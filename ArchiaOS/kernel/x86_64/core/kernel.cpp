#include "../../../common/boot_info.h"
#include "../cpu/gdt.hpp"
#include "../cpu/tss.hpp"
#include "../cpu/idt.hpp"
#include "../cpu/irq.hpp"
#include "../cpu/acpi.hpp"
#include "../memory/pmm.hpp"
#include "../memory/paging.hpp"
#include "../memory/vmm.hpp"
#include "../memory/heap.hpp"
#include "../memory/address_space.hpp"
#include "../process/process.hpp"
#include "../fs/vfs.hpp"
#include "../fs/openfs_adapter.hpp"
#include "../drivers/pci.hpp"
#include "../drivers/input.hpp"
#include "../drivers/graphics.hpp"
#include "../drivers/gui.hpp"
#include "../drivers/device.hpp"
#include "../drivers/net.hpp"
#include "../drivers/block.hpp"
#include "../drivers/storage.hpp"
#include "../fs/gpt.hpp"
#include "../cpu/features.hpp"
#include "../cpu/security.hpp"
#include "../cpu/smp.hpp"
#include "../cpu/scheduler.hpp"
#include "../cpu/ioapic.hpp"
#include "../cpu/lapic.hpp"

using UINT32 = unsigned int;
using UINT64 = unsigned long long;

static inline void debug_char(char c)
{
    asm volatile(
        "outb %0,%1"
        :
        : "a"(c), "Nd"(static_cast<unsigned short>(0xE9))
        : "memory");
}

static void debug_str(const char* s)
{
    for (int i = 0; s[i]; ++i)
        debug_char(s[i]);
}

static void halt()
{
    for (;;)
        asm volatile("cli; hlt");
}

extern "C" void pmm_run_tests(BootInfo* bootInfo);

extern "C" void kernel_main(BootInfo* bootInfo)
{
    debug_str("KERNEL STARTED\n");
    debug_str("Architecture: x86_64\n");

    if (!bootInfo)
    {
        debug_str("[KERNEL] BootInfo NULL\n");
        halt();
    }

    if (bootInfo->magic != BOOT_INFO_MAGIC)
    {
        debug_str("[KERNEL] BootInfo BAD MAGIC\n");
        halt();
    }

    if (bootInfo->version == 0 ||
        bootInfo->version > BOOT_INFO_VERSION ||
        bootInfo->size < sizeof(BootInfo))
    {
        debug_str("[KERNEL] BootInfo BAD VERSION/SIZE\n");
        halt();
    }

    if (!bootInfo->kernel_image_base ||
        !bootInfo->kernel_image_size ||
        !bootInfo->boot_info_address ||
        bootInfo->boot_info_size < sizeof(BootInfo))
    {
        debug_str("[KERNEL] BootInfo ABI addresses BAD\n");
        halt();
    }

    if (!bootInfo->memory_map_address ||
        !bootInfo->memory_map_size ||
        bootInfo->memory_descriptor_size < 40 ||
        bootInfo->memory_descriptor_size > 4096 ||
        bootInfo->memory_map_size <
            bootInfo->memory_descriptor_size ||
        bootInfo->memory_map_size %
            bootInfo->memory_descriptor_size != 0 ||
        bootInfo->memory_descriptor_count !=
            bootInfo->memory_map_size / bootInfo->memory_descriptor_size)
    {
        debug_str("[KERNEL] Memory map BAD\n");
        halt();
    }

    constexpr UINT64 bootstrapPhysicalLimit = PMM_MAX_PHYSICAL_ADDRESS;
    const auto physical_range_ok = [](UINT64 base, UINT64 size) -> bool
    {
        return base != 0 && size != 0 &&
               base < bootstrapPhysicalLimit &&
               size <= bootstrapPhysicalLimit - base;
    };

    if (!physical_range_ok(bootInfo->kernel_image_base,
                           bootInfo->kernel_image_size) ||
        !physical_range_ok(bootInfo->boot_info_address,
                           bootInfo->boot_info_size) ||
        !physical_range_ok(bootInfo->memory_map_address,
                           bootInfo->memory_map_size))
    {
        debug_str("[KERNEL] BOOT PHYSICAL RANGE EXCEEDS BOOTSTRAP LIMIT\\n");
        halt();
    }

    if (!physical_range_ok(bootInfo->framebuffer_base,
                           bootInfo->framebuffer_size))
    {
        debug_str("[KERNEL] FRAMEBUFFER PHYSICAL RANGE EXCEEDS BOOTSTRAP LIMIT\\n");
        halt();
    }

    if (!bootInfo->framebuffer_base ||
        !bootInfo->framebuffer_width ||
        !bootInfo->framebuffer_height)
    {
        debug_str("[KERNEL] Framebuffer BAD\n");
        halt();
    }

    const UINT64 minimumPitch =
        static_cast<UINT64>(bootInfo->framebuffer_width) * 4ULL;

    if (static_cast<UINT64>(bootInfo->framebuffer_pitch) < minimumPitch)
    {
        debug_str("[KERNEL] Framebuffer PITCH BAD\n");
        halt();
    }

    const UINT64 requiredFramebufferBytes =
        static_cast<UINT64>(bootInfo->framebuffer_pitch) *
        static_cast<UINT64>(bootInfo->framebuffer_height);

    if (requiredFramebufferBytes > bootInfo->framebuffer_size)
    {
        debug_str("[KERNEL] Framebuffer SIZE BAD\n");
        halt();
    }

    debug_str("BootInfo: OK\n");
    debug_str("Memory Map: OK\n");
    debug_str("Framebuffer: OK\n");

    debug_str("CPU: detecting CPUID/features/topology\n");
    cpu_initialize();
    cpu_print_report();

    debug_str("CPU: initializing GDT\n");
    gdt_initialize();
    debug_str("CPU: GDT OK\n");

    debug_str("CPU: initializing TSS\n");
    tss_initialize();
    debug_str("CPU: TSS OK\n");

    debug_str("CPU: initializing IDT\n");
    idt_initialize();
    debug_str("CPU: IDT 256 VECTORS OK\n");

    debug_str("PMM: initializing\n");
    if (!pmm_initialize(bootInfo))
    {
        debug_str("[KERNEL] PMM INIT FAILED\n");
        halt();
    }
    debug_str("PMM: running tests\n");
    pmm_run_tests(bootInfo);
    debug_str("PMM: TESTS OK\n");

    /*
     * Enable CR0.WP/NXE/UMIP before creating 4 KiB mappings that use
     * execute-disable. IA32_EFER.NXE must be enabled before a present
     * paging entry is allowed to carry XD=1.
     */
    debug_str("CPU: activating hardware security protections\n");
    if (!cpu_security_initialize())
    {
        debug_str("[KERNEL] CPU SECURITY ACTIVATION FAILED\n");
        halt();
    }
    cpu_security_print_report();

    debug_str("MM: initializing paging\n");
    if (!paging_initialize())
    {
        debug_str("[KERNEL] PAGING INIT FAILED\n");
        halt();
    }
    paging_run_tests();
    debug_str("MM: paging ACTIVE\n");

    debug_str("MM: initializing virtual memory manager\n");
    if (!vmm_initialize())
    {
        debug_str("[KERNEL] VMM INIT FAILED\n");
        halt();
    }
    vmm_run_tests();
    debug_str("MM: VMM OK\n");

    debug_str("MM: initializing kernel heap\n");
    if (!heap_initialize())
    {
        debug_str("[KERNEL] HEAP INIT FAILED\n");
        halt();
    }
    heap_run_tests();
    debug_str("MM: KERNEL HEAP OK\n");

    debug_str("MM: testing independent address space\n");
    address_space_run_tests();
    debug_str("MM: ADDRESS SPACE OK\n");
    if (!block_memory_test()) {
        debug_str("[KERNEL] BLOCK DEVICE TEST FAILED\n");
        halt();
    }
    debug_str("STORAGE: BLOCK DEVICE OK\n");
    const uint32_t openfs_test_stage = openfs_kernel_test();
    if (openfs_test_stage != 0U)
    {
        debug_str("[KERNEL] OPENFS TEST FAILED STAGE ");
        debug_char(static_cast<char>('0' + (openfs_test_stage % 10U)));
        debug_str("\n");
        halt();
    }
    debug_str("OPENFS: FORMAT/MOUNT/LOOKUP/READ/WRITE/FSCK OK\n");
    if (!gpt_test()) {
        debug_str("[KERNEL] GPT TEST FAILED\n");
        halt();
    }
    debug_str("STORAGE: GPT OK\n");
    if (!storage_test()) {
        debug_str("[KERNEL] STORAGE CONTROLLER TEST FAILED\n");
        halt();
    }
    debug_str("STORAGE: CONTROLLER DISCOVERY OK\n");
    debug_str("STORAGE: BEFORE REAL COUNT\n");
    if (storage_real_block_device_count() > 0)
        debug_str("STORAGE: REAL BLOCK DEVICE OK\n");
    else
        debug_str("STORAGE: NO REAL BLOCK DEVICE\n");
    debug_str("STORAGE: BEFORE VFS\n");
    if (!vfs_test())
    {
        debug_str("[KERNEL] VFS TEST FAILED\n");
        halt();
    }
    debug_str("VFS: OPENFS BACKEND OK\n");
    if (!pci_test())
    {
        debug_str("[KERNEL] PCI ENUMERATION FAILED\n");
        halt();
    }
    debug_str("PCI: ENUMERATION OK\n");
    if (!input_test())
    {
        debug_str("[KERNEL] INPUT QUEUE TEST FAILED\n");
        halt();
    }
    debug_str("INPUT: EVENT QUEUE OK\n");
    if (!graphics_test())
    {
        debug_str("[KERNEL] GRAPHICS TEST FAILED\n");
        halt();
    }
    debug_str("GRAPHICS: SOFTWARE COMPOSITOR OK\n");
    if (!gui_test())
    {
        debug_str("[KERNEL] GUI TEST FAILED\n");
        halt();
    }
    debug_str("GUI: WINDOW MANAGER OK\n");
    if (!device_manager_test())
    {
        debug_str("[KERNEL] DEVICE MANAGER TEST FAILED\n");
        halt();
    }
    debug_str("DRIVER: DEVICE MANAGER OK\n");
    if (!net_test())
    {
        debug_str("[KERNEL] NETWORK TEST FAILED\n");
        halt();
    }
    debug_str("NET: LOOPBACK OK\n");

    debug_str("CPU: testing invalid opcode handler\n");
    exception_expect_invalid_opcode(
        reinterpret_cast<unsigned long long>(&idt_test_invalid_opcode));
    idt_test_invalid_opcode();
    if (exception_invalid_opcode_test_active())
    {
        debug_str("[KERNEL] INVALID OPCODE TEST DID NOT TRAP\n");
        halt();
    }
    debug_str("CPU: INVALID OPCODE HANDLER OK\n");

    debug_str("IRQ: initializing LAPIC\n");
    if (!irq_initialize())
    {
        debug_str("[KERNEL] IRQ/LAPIC INIT FAILED\n");
        halt();
    }
    debug_str("IRQ: LAPIC OK\n");

    debug_str("IRQ: testing timer\n");
    if (!irq_test_timer())
    {
        debug_str("[KERNEL] IRQ TIMER TEST FAILED\n");
        halt();
    }
    debug_str("IRQ: TIMER TEST OK\n");
    if (irq_dispatch_count_get() < 3)
    {
        debug_str("[KERNEL] IRQ DISPATCH CHAIN VALIDATION FAILED\n");
        halt();
    }
    debug_str("IRQ: DISPATCH CHAIN OK\n");

    debug_str("SCHEDULER: initializing\n");
    if (!scheduler_initialize(1) ||
        !scheduler_set_local_cpu_index(0) ||
        !scheduler_run_test())
    {
        debug_str("[KERNEL] SCHEDULER TEST FAILED\n");
        halt();
    }
    debug_str("SCHEDULER: ROUND-ROBIN TEST OK\n");

    debug_str("ACPI: parsing RSDP/MADT\n");
    if (!acpi_initialize(bootInfo->acpi_rsdp_address))
    {
        debug_str("[KERNEL] ACPI INIT FAILED\n");
        halt();
    }

    const AcpiInfo* acpi = acpi_get_info();
    if (!acpi ||
        acpi->rsdp_address == 0 ||
        acpi->root_table_address == 0 ||
        acpi->madt_address == 0 ||
        acpi->local_apic_address == 0 ||
        acpi->processor_count == 0)
    {
        debug_str("[KERNEL] ACPI DATA INVALID\n");
        halt();
    }

    debug_str("ACPI: RSDP/MADT OK\n");
    debug_str("ACPI: CPU/IOAPIC tables parsed\n");

    debug_str("IOAPIC: initializing redirection table\n");
    if (!ioapic_initialize(acpi))
    {
        debug_str("[KERNEL] IOAPIC INIT FAILED\n");
        halt();
    }

    const unsigned int bspApicId = lapic_current_id();
    if (!ioapic_route_isa_irq(acpi, 1, 0x21, bspApicId))
    {
        debug_str("[KERNEL] IOAPIC KEYBOARD ROUTE FAILED\n");
        halt();
    }

    unsigned long long keyboardRoute = 0;
    if (!ioapic_read_redirection(1, &keyboardRoute) ||
        (keyboardRoute & 0xFFULL) != 0x21ULL ||
        (keyboardRoute & (1ULL << 16)) != 0)
    {
        debug_str("[KERNEL] IOAPIC KEYBOARD ROUTE TEST FAILED\n");
        halt();
    }
    debug_str("IOAPIC: KEYBOARD IRQ ROUTE OK\n");
    debug_str("IRQ: KEYBOARD HANDLER READY\n");

    debug_str("SMP: starting application processors\n");
    if (!smp_initialize(acpi))
    {
        debug_str("[KERNEL] SMP INIT FAILED\\n");
        halt();
    }
    debug_str("SMP: APPLICATION PROCESSORS ONLINE\n");

    debug_str("SCHEDULER: starting per-CPU preemptive context-switch test\n");

    const unsigned int scheduler_cpu_count =
        acpi->processor_count < SCHEDULER_MAX_CPUS
            ? acpi->processor_count
            : SCHEDULER_MAX_CPUS;

    if (!scheduler_initialize(scheduler_cpu_count))
    {
        debug_str("[KERNEL] SCHEDULER PREEMPTIVE INIT FAILED\n");
        halt();
    }

    unsigned int scheduler_apic_ids[SCHEDULER_MAX_CPUS] = {};
    unsigned int scheduler_cpu = 0;
    scheduler_apic_ids[scheduler_cpu++] = bspApicId;
    for (unsigned int index = 0;
         index < scheduler_cpu_count && scheduler_cpu < scheduler_cpu_count;
         ++index)
    {
        const unsigned int apic_id = acpi->processor_apic_ids[index];
        if (apic_id != bspApicId)
            scheduler_apic_ids[scheduler_cpu++] = apic_id;
    }
    if (scheduler_cpu != scheduler_cpu_count)
    {
        debug_str("[KERNEL] SCHEDULER APIC TOPOLOGY INCOMPLETE\\n");
        halt();
    }

    if (!scheduler_set_cpu_apic_ids(scheduler_apic_ids, scheduler_cpu_count))
    {
        debug_str("[KERNEL] SCHEDULER APIC MAP FAILED\\n");
        halt();
    }

    if (!scheduler_kernel_thread_test())
    {
        debug_str("[KERNEL] KERNEL THREAD CREATION FAILED\n");
        halt();
    }
    debug_str("SCHEDULER: REAL KERNEL THREADS READY\n");

    if (!scheduler_set_ready_for_kernel())
    {
        debug_str("[KERNEL] SCHEDULER READY FAILED\n");
        halt();
    }

    /* Release APs into their per-CPU scheduler loops. */
    scheduler_cpu_start(0);
    lapic_timer_start();
    irq_enable();

    bool preemptive_test_ok = false;
    for (unsigned long long timeout = 0;
         timeout < 200000000ULL;
         ++timeout)
    {
        bool all_cpus_started = true;
        for (unsigned int cpu = 0; cpu < scheduler_cpu_count; ++cpu)
        {
            if (scheduler_task1_counter_get(cpu) == 0 ||
                scheduler_switch_count(cpu) < 2)
            {
                all_cpus_started = false;
                break;
            }
        }

        if (all_cpus_started &&
            scheduler_kernel_thread_counter(0, 0) != 0 &&
            scheduler_kernel_thread_counter(0, 1) != 0 &&
            scheduler_kernel_thread_exited(0) &&
            scheduler_task_count(0) == 4)
        {
            preemptive_test_ok = true;
            break;
        }

        asm volatile("pause");
    }

    if (!preemptive_test_ok)
    {
        debug_str("[KERNEL] SCHEDULER PER-CPU TEST FAILED\n");
        halt();
    }
    debug_str("SCHEDULER: PREEMPTIVE_OK\n");

    if (bootInfo->framebuffer_base)
    {
        /* Framebuffer is MMIO/video memory: use UC page mappings. */
        const UINT64 first = bootInfo->framebuffer_base & ~(PAGE_SIZE - 1ULL);
        const UINT64 end = bootInfo->framebuffer_base +
            static_cast<UINT64>(bootInfo->framebuffer_pitch) *
            static_cast<UINT64>(bootInfo->framebuffer_height);
        for (UINT64 page = first; page < end; page += PAGE_SIZE)
        {
            if (!paging_map_4k(page, page, PagingFlags{true, false, true, true, true}))
            {
                debug_str("[KERNEL] FRAMEBUFFER MMIO MAP FAILED\\n");
                halt();
            }
        }

        GraphicsSurface surface{
            reinterpret_cast<volatile UINT32*>(bootInfo->framebuffer_base),
            bootInfo->framebuffer_width,
            bootInfo->framebuffer_height,
            bootInfo->framebuffer_pitch / 4
        };
        if (!graphics_initialize(&surface))
        {
            debug_str("[KERNEL] GRAPHICS FRAMEBUFFER INIT FAILED\\n");
            halt();
        }
        gui_render(&surface);
    }

    debug_str("PROCESS: ELF LOADER READY\n");
    if (!process_run_ring3_test())
    {
        irq_disable();
        lapic_stop_timer();
        debug_str("[KERNEL] RING3 PROCESS FAILED\n");
        halt();
    }

    for (;;)
        asm volatile("hlt");
}
