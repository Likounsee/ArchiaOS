#include "process.hpp"
#include "../cpu/idt.hpp"
#include "../memory/paging.hpp"
#include "../memory/pmm.hpp"
#include "ipc.hpp"

extern "C" [[noreturn]] void ring3_enter(uint64_t rip, uint64_t rsp);

static Process* current_process = nullptr;
static uint32_t next_pid = 1;
static constexpr unsigned int PROCESS_MAX = 64;
static Process* process_table[PROCESS_MAX] = {};
static unsigned int process_table_count = 0;
/* Initial process state is kept kernel-owned until the first user transition. */
static volatile unsigned long long syscall_count = 0;
static volatile bool ipc_user_ok = false;

struct Elf64Header
{
    uint8_t ident[16];
    uint16_t type;
    uint16_t machine;
    uint32_t version;
    uint64_t entry;
    uint64_t phoff;
    uint64_t shoff;
    uint32_t flags;
    uint16_t ehsize;
    uint16_t phentsize;
    uint16_t phnum;
    uint16_t shentsize;
    uint16_t shnum;
    uint16_t shstrndx;
};

struct Elf64ProgramHeader
{
    uint32_t type;
    uint32_t flags;
    uint64_t offset;
    uint64_t vaddr;
    uint64_t paddr;
    uint64_t filesz;
    uint64_t memsz;
    uint64_t align;
};

static bool range_ok(uint64_t offset, uint64_t size, uint64_t limit)
{
    return offset <= limit && size <= limit - offset;
}

static bool process_pid_in_use(uint32_t pid)
{
    if (!pid)
        return true;
    for (unsigned int i = 0; i < PROCESS_MAX; ++i)
        if (process_table[i] && process_table[i]->pid == pid)
            return true;
    return false;
}

static bool allocate_process_pid(uint32_t* pid)
{
    if (!pid)
        return false;

    /* At most PROCESS_MAX PIDs can be resident, so this bound guarantees a free candidate. */
    for (uint64_t attempts = 0; attempts <= PROCESS_MAX; ++attempts)
    {
        const uint32_t candidate = next_pid ? next_pid : 1U;
        next_pid = candidate + 1U;
        if (!next_pid)
            next_pid = 1U;

        if (!process_pid_in_use(candidate))
        {
            *pid = candidate;
            return true;
        }
    }
    return false;
}

static bool map_stack(AddressSpace* space, uint64_t* top)
{
    if (!space || !top)
        return false;

    const uint64_t stack_top = NOVOS_USER_VIRTUAL_TOP;
    const uint64_t stack_base = stack_top - 3ULL * NOVOS_PAGE_SIZE;

    /* Keep one unmapped guard page below the user stack. */
    for (uint64_t va = stack_base; va < stack_top; va += NOVOS_PAGE_SIZE)
    {
        const uint64_t physical = pmm_alloc_page_above(0x01000000ULL);
        if (!physical)
            return false;
        if (!address_space_map(space, va, physical, true, false))
        {
            pmm_free_page(physical);
            return false;
        }

        auto* page = reinterpret_cast<uint8_t*>(paging_physical_to_virtual(physical));
        for (unsigned int i = 0; i < 4096; ++i)
            page[i] = 0;
    }

    *top = stack_top;
    return true;
}

extern "C" bool process_create_elf(
    Process* process, const uint8_t* image, uint64_t image_size)
{
    if (!process || !image || image_size < sizeof(Elf64Header) ||
        process->state != PROCESS_UNUSED ||
        process->pid != 0 || process->address_space.pml4_physical != 0)
        return false;

    const auto* header = reinterpret_cast<const Elf64Header*>(image);
    if (header->ident[0] != 0x7F || header->ident[1] != 'E' ||
        header->ident[2] != 'L' || header->ident[3] != 'F' ||
        header->ident[4] != 2 || header->ident[5] != 1 ||
        header->type != 2 || header->machine != 62 ||
        header->version != 1 || header->ehsize != sizeof(Elf64Header) ||
        header->phentsize != sizeof(Elf64ProgramHeader) ||
        header->phnum == 0)
        return false;

    if (!range_ok(header->phoff,
                  static_cast<uint64_t>(header->phnum) * header->phentsize,
                  image_size))
        return false;

    AddressSpace space{};
    if (!address_space_create(&space))
        return false;

    const auto* phdrs = reinterpret_cast<const Elf64ProgramHeader*>(
        image + header->phoff);

    bool loaded = false;
    for (unsigned int i = 0; i < header->phnum; ++i)
    {
        const auto& ph = phdrs[i];
        if (ph.type != 1)
            continue;
        if (ph.memsz == 0 || ph.memsz < ph.filesz ||
            !range_ok(ph.offset, ph.filesz, image_size) ||
            (ph.flags & 6U) == 6U ||
            ph.vaddr < NOVOS_USER_VIRTUAL_BASE ||
            ph.vaddr >= NOVOS_USER_VIRTUAL_TOP ||
            ph.memsz > NOVOS_USER_VIRTUAL_TOP - ph.vaddr ||
            (ph.align != 0 && (ph.align & (ph.align - 1)) != 0) ||
            (ph.align > 1 &&
             ((ph.vaddr ^ ph.offset) & (ph.align - 1)) != 0))
        {
            address_space_destroy(&space);
            return false;
        }

        const uint64_t base = ph.vaddr & ~0xFFFULL;
        const uint64_t end = (ph.vaddr + ph.memsz + 0xFFFULL) & ~0xFFFULL;
        for (uint64_t va = base; va < end; va += NOVOS_PAGE_SIZE)
        {
            const uint64_t physical = pmm_alloc_page_above(0x01000000ULL);
            if (!physical)
            {
                address_space_destroy(&space);
                return false;
            }
            if (!address_space_map(
                    &space, va, physical,
                    (ph.flags & 2U) != 0, (ph.flags & 1U) != 0))
            {
                pmm_free_page(physical);
                address_space_destroy(&space);
                return false;
            }

            auto* page = reinterpret_cast<uint8_t*>(
                paging_physical_to_virtual(physical));
            for (unsigned int j = 0; j < 4096; ++j)
                page[j] = 0;

            const uint64_t page_end = va + NOVOS_PAGE_SIZE;
            const uint64_t copy_begin = ph.vaddr > va ? ph.vaddr : va;
            const uint64_t file_end = ph.vaddr + ph.filesz;
            const uint64_t copy_end = file_end < page_end ? file_end : page_end;
            if (copy_begin < copy_end)
            {
                const uint64_t source_offset =
                    ph.offset + (copy_begin - ph.vaddr);
                auto* destination = page + (copy_begin - va);
                for (uint64_t n = 0; n < copy_end - copy_begin; ++n)
                    destination[n] = image[source_offset + n];
            }
        }
        loaded = true;
    }

    if (!loaded)
    {
        address_space_destroy(&space);
        return false;
    }

    uint64_t stack_top = 0;
    if (!map_stack(&space, &stack_top))
    {
        address_space_destroy(&space);
        return false;
    }

    if (header->entry < NOVOS_USER_VIRTUAL_BASE ||
        header->entry >= NOVOS_USER_VIRTUAL_TOP ||
        !address_space_is_user_executable(&space, header->entry))
    {
        address_space_destroy(&space);
        return false;
    }

    uint32_t pid = 0;
    if (!allocate_process_pid(&pid))
    {
        address_space_destroy(&space);
        return false;
    }

    process->pid = pid;
    process->state = PROCESS_READY;
    process->address_space = space;
    process->entry = header->entry;
    process->user_stack_top = stack_top;
    return true;
}

extern "C" bool process_register(Process* process)
{
    if (!process || !process->pid ||
        !process->address_space.pml4_physical ||
        (process->state != PROCESS_READY && process->state != PROCESS_RUNNING))
        return false;
    for (unsigned int i = 0; i < PROCESS_MAX; ++i)
    {
        if (process_table[i] == process)
            return true;
        if (process_table[i] && process_table[i]->pid == process->pid)
            return false;
        if (!process_table[i])
        {
            process_table[i] = process;
            ++process_table_count;
            return true;
        }
    }
    return false;
}

extern "C" Process* process_find(uint32_t pid)
{
    if (!pid)
        return nullptr;
    for (unsigned int i = 0; i < PROCESS_MAX; ++i)
        if (process_table[i] && process_table[i]->pid == pid)
            return process_table[i];
    return nullptr;
}

extern "C" bool process_unregister(Process* process)
{
    if (!process || process == current_process)
        return false;
    for (unsigned int i = 0; i < PROCESS_MAX; ++i)
    {
        if (process_table[i] == process)
        {
            process_table[i] = nullptr;
            --process_table_count;
            return true;
        }
    }
    return false;
}

extern "C" bool process_destroy(Process* process)
{
    if (!process || process == current_process || !process->pid)
        return false;

    unsigned int table_slot = PROCESS_MAX;
    for (unsigned int i = 0; i < PROCESS_MAX; ++i)
    {
        if (process_table[i] == process)
        {
            table_slot = i;
            break;
        }
    }
    if (table_slot == PROCESS_MAX)
        return false;

    if (!address_space_destroy(&process->address_space))
        return false;

    ipc_destroy_owner(process->pid);

    process_table[table_slot] = nullptr;
    --process_table_count;
    process->state = PROCESS_EXITED;
    process->pid = 0;
    process->entry = 0;
    process->user_stack_top = 0;
    return true;
}

extern "C" bool process_activate(Process* process)
{
    if (!process || !process->pid || !process->address_space.pml4_physical ||
        process_find(process->pid) != process)
        return false;
    if (!address_space_activate(&process->address_space))
        return false;
    current_process = process;
    process->state = PROCESS_RUNNING;
    return true;
}

extern "C" unsigned int process_registered_count()
{
    return process_table_count;
}

extern "C" uint32_t process_current_pid()
{
    return current_process ? current_process->pid : 0;
}

extern "C" bool process_ipc_user_ok()
{
    return __atomic_load_n(&ipc_user_ok, __ATOMIC_ACQUIRE);
}

extern "C" unsigned long long process_syscall_count()
{
    return __atomic_load_n(&syscall_count, __ATOMIC_ACQUIRE);
}

extern "C" bool process_handle_syscall(ExceptionFrame* frame)
{
    if (!frame || !current_process ||
        current_process->state != PROCESS_RUNNING ||
        (frame->cs & 3ULL) != 3ULL)
        return false;

    if (frame->rip > UINT64_MAX - 2ULL ||
        !address_space_is_user_executable(
            &current_process->address_space, frame->rip) ||
        !address_space_is_user_executable(
            &current_process->address_space, frame->rip + 1ULL))
        return false;

    const u64 instruction_physical = paging_translate(frame->rip);
    const u64 instruction_virtual =
        paging_physical_to_virtual(instruction_physical);
    if (!instruction_physical || !instruction_virtual)
        return false;

    const auto* instruction =
        reinterpret_cast<const uint8_t*>(instruction_virtual);
    if (instruction[0] != 0xCD || instruction[1] != 0x80)
        return false;

    if ((frame->rax == 4 || frame->rax == 5 || frame->rax == 6) &&
        frame->rbx >= 32ULL)
        return false;

    __atomic_fetch_add(&syscall_count, 1ULL, __ATOMIC_RELAXED);

    switch (frame->rax)
    {
        case 1:
            frame->rax = current_process->pid;
            break;
        case 2:
            frame->rax = 0;
            break;
        case 3:
            frame->rax = static_cast<uint64_t>(ipc_create(current_process->pid));
            break;
        case 4:
            frame->rax = ipc_send(
                static_cast<int>(frame->rbx),
                current_process->pid,
                0,
                frame->rcx) ? 0 : static_cast<uint64_t>(-1);
            break;
        case 5:
        {
            IpcMessage message{};
            if (ipc_receive(
                    static_cast<int>(frame->rbx),
                    current_process->pid,
                    &message))
            {
                frame->rax = message.value;
                if (message.value == 0x12345678ULL)
                    ipc_user_ok = true;
            }
            else
                frame->rax = static_cast<uint64_t>(-1);
            break;
        }
        case 6:
            frame->rax = ipc_destroy(
                static_cast<int>(frame->rbx),
                current_process->pid) ? 0 : static_cast<uint64_t>(-1);
            break;
        default:
            frame->rax = static_cast<uint64_t>(-1);
            break;
    }
    frame->rip += 2;
    return true;
}

extern "C" bool process_run_ring3_test()
{
    static uint8_t image[0x11E] = {};
    for (unsigned int i = 0; i < sizeof(image); ++i)
        image[i] = 0;

    image[0] = 0x7F; image[1] = 'E'; image[2] = 'L'; image[3] = 'F';
    image[4] = 2; image[5] = 1; image[6] = 1;

    auto put16 = [](uint8_t* p, uint16_t v)
    {
        p[0] = static_cast<uint8_t>(v);
        p[1] = static_cast<uint8_t>(v >> 8);
    };
    auto put32 = [](uint8_t* p, uint32_t v)
    {
        for (unsigned int i = 0; i < 4; ++i)
            p[i] = static_cast<uint8_t>(v >> (i * 8));
    };
    auto put64 = [](uint8_t* p, uint64_t v)
    {
        for (unsigned int i = 0; i < 8; ++i)
            p[i] = static_cast<uint8_t>(v >> (i * 8));
    };

    put16(image + 16, 2);
    put16(image + 18, 62);
    put32(image + 20, 1);
    put64(image + 24, 0x400100ULL);
    put64(image + 32, 64);
    put16(image + 52, 64);
    put16(image + 54, 56);
    put16(image + 56, 1);

    put32(image + 64, 1);
    put32(image + 68, 5);
    put64(image + 72, 0x100);
    put64(image + 80, 0x400100);
    put64(image + 88, 0);
    put64(image + 96, 30);
    put64(image + 104, 30);
    put64(image + 112, 0x1000);

    const uint8_t user_code[] = {
        0xB8, 0x03, 0x00, 0x00, 0x00, 0xCD, 0x80,
        0x89, 0xC3,
        0xB9, 0x78, 0x56, 0x34, 0x12,
        0xB8, 0x04, 0x00, 0x00, 0x00, 0xCD, 0x80,
        0xB8, 0x05, 0x00, 0x00, 0x00, 0xCD, 0x80,
        0xEB, 0xFE
    };
    for (unsigned int i = 0; i < sizeof(user_code); ++i)
        image[0x100 + i] = user_code[i];

    static Process invalid_wx{};
    put32(image + 68, 7);
    if (process_create_elf(&invalid_wx, image, sizeof(image)))
        return false;
    put32(image + 68, 5);

    static Process non_executable_entry{};
    put32(image + 68, 4);
    if (process_create_elf(&non_executable_entry, image, sizeof(image)))
        return false;
    put32(image + 68, 5);

    static Process invalid_alignment{};
    put64(image + 72, 0x400101);
    put64(image + 96, 1);
    put64(image + 104, 1);
    put64(image + 112, 0x200);
    if (process_create_elf(&invalid_alignment, image, sizeof(image)))
        return false;
    put64(image + 72, 0x100);
    put64(image + 96, 30);
    put64(image + 104, 30);
    put64(image + 112, 0x1000);

    static Process wrap_process{};
    next_pid = UINT32_MAX;
    if (!process_create_elf(&wrap_process, image, sizeof(image)) ||
        wrap_process.pid != UINT32_MAX ||
        !process_register(&wrap_process) ||
        !process_destroy(&wrap_process))
        return false;

    static Process process{};
    if (!process_create_elf(&process, image, sizeof(image)))
        return false;

    Process busy_state{};
    busy_state.state = PROCESS_READY;
    if (process_create_elf(&busy_state, image, sizeof(image)))
        return false;

    if (process_create_elf(&process, image, sizeof(image)))
        return false;
    if (!process_register(&process) || process_find(process.pid) != &process ||
        process_registered_count() != 1)
        return false;

    static Process cleanup_process{};
    if (!process_create_elf(&cleanup_process, image, sizeof(image)) ||
        !process_register(&cleanup_process))
        return false;
    const int cleanup_endpoint = ipc_create(cleanup_process.pid);
    IpcMessage cleanup_message{};
    if (cleanup_endpoint < 0 ||
        !process_destroy(&cleanup_process) ||
        cleanup_process.state != PROCESS_EXITED ||
        cleanup_process.pid != 0 ||
        process_register(&cleanup_process) ||
        ipc_receive(cleanup_endpoint, cleanup_process.pid, &cleanup_message))
        return false;

    Process duplicate = process;
    if (process_register(&duplicate) || process_destroy(&duplicate))
        return false;

    Process invalid_state = process;
    invalid_state.state = PROCESS_EXITED;
    if (process_register(&invalid_state))
        return false;

    if (!process_unregister(&process) || process_registered_count() != 0)
        return false;
    if (process_find(process.pid) != &process ||
        process_activate(&process))
        return false;
    if (!process_register(&process) || process_registered_count() != 1)
        return false;

    if (!process_activate(&process))
        return false;

    if (process_unregister(&process) || process_destroy(&process) ||
        process_find(process.pid) != &process ||
        process_current_pid() != process.pid ||
        process_registered_count() != 1)
        return false;

    ExceptionFrame invalid_syscall_frame{};
    invalid_syscall_frame.cs = 0x1B;
    invalid_syscall_frame.rip = process.user_stack_top - NOVOS_PAGE_SIZE;
    if (process_handle_syscall(&invalid_syscall_frame))
        return false;

    process.state = PROCESS_READY;
    if (process_handle_syscall(&invalid_syscall_frame))
        return false;
    process.state = PROCESS_RUNNING;

    ExceptionFrame overflow_syscall_frame{};
    overflow_syscall_frame.cs = 0x1B;
    overflow_syscall_frame.rip = UINT64_MAX - 1ULL;
    if (process_handle_syscall(&overflow_syscall_frame))
        return false;

    ExceptionFrame non_syscall_instruction{};
    non_syscall_instruction.cs = 0x1B;
    non_syscall_instruction.rip = process.entry;
    non_syscall_instruction.rax = 1;
    if (process_handle_syscall(&non_syscall_instruction))
        return false;

    ExceptionFrame valid_syscall_frame{};
    valid_syscall_frame.cs = 0x1B;
    valid_syscall_frame.rip = process.entry + 5ULL;
    valid_syscall_frame.rax = 1;
    const unsigned long long syscall_before =
        process_syscall_count();
    if (!process_handle_syscall(&valid_syscall_frame) ||
        valid_syscall_frame.rax != process.pid ||
        valid_syscall_frame.rip != process.entry + 7ULL ||
        process_syscall_count() != syscall_before + 1ULL)
        return false;

    ExceptionFrame create_endpoint_frame{};
    create_endpoint_frame.cs = 0x1B;
    create_endpoint_frame.rip = process.entry + 7ULL;
    create_endpoint_frame.rax = 3;
    if (!process_handle_syscall(&create_endpoint_frame) ||
        create_endpoint_frame.rax >= 32ULL)
        return false;

    const uint64_t endpoint = create_endpoint_frame.rax;
    ExceptionFrame destroy_endpoint_frame{};
    destroy_endpoint_frame.cs = 0x1B;
    destroy_endpoint_frame.rip = process.entry + 7ULL;
    destroy_endpoint_frame.rax = 6;
    destroy_endpoint_frame.rbx = endpoint;
    if (!process_handle_syscall(&destroy_endpoint_frame) ||
        destroy_endpoint_frame.rax != 0)
        return false;

    ExceptionFrame destroyed_endpoint_frame{};
    destroyed_endpoint_frame.cs = 0x1B;
    destroyed_endpoint_frame.rip = process.entry + 7ULL;
    destroyed_endpoint_frame.rax = 6;
    destroyed_endpoint_frame.rbx = endpoint;
    if (!process_handle_syscall(&destroyed_endpoint_frame) ||
        destroyed_endpoint_frame.rax != static_cast<uint64_t>(-1))
        return false;

    ExceptionFrame invalid_endpoint_frame{};
    invalid_endpoint_frame.cs = 0x1B;
    invalid_endpoint_frame.rip = process.entry + 7ULL;
    invalid_endpoint_frame.rax = 6;
    invalid_endpoint_frame.rbx = 32ULL;
    const unsigned long long rejected_syscalls = process_syscall_count();
    if (process_handle_syscall(&invalid_endpoint_frame) ||
        process_syscall_count() != rejected_syscalls)
        return false;

    asm volatile("sti" : : : "memory");
    ring3_enter(process.entry, process.user_stack_top);
}
