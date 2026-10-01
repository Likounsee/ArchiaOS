#include "elf_loader.h"
#include "boot_debug.h"

struct Elf64Header
{
    UINT8 e_ident[16];
    UINT16 e_type;
    UINT16 e_machine;
    UINT32 e_version;
    UINT64 e_entry;
    UINT64 e_phoff;
    UINT64 e_shoff;
    UINT32 e_flags;
    UINT16 e_ehsize;
    UINT16 e_phentsize;
    UINT16 e_phnum;
    UINT16 e_shentsize;
    UINT16 e_shnum;
    UINT16 e_shstrndx;
};

struct Elf64ProgramHeader
{
    UINT32 p_type;
    UINT32 p_flags;
    UINT64 p_offset;
    UINT64 p_vaddr;
    UINT64 p_paddr;
    UINT64 p_filesz;
    UINT64 p_memsz;
    UINT64 p_align;
};

static constexpr UINT16 ET_EXEC = 2;
static constexpr UINT16 EM_X86_64 = 62;
static constexpr UINT8 ELFCLASS64 = 2;
static constexpr UINT8 ELFDATA2LSB = 1;
static constexpr UINT32 PT_LOAD = 1;
static constexpr UINT32 PF_X = 1;
static constexpr UINT64 PAGE = 4096;
using EFI_FREE_PAGES = EFI_STATUS(EFIAPI*)(EFI_PHYSICAL_ADDRESS, UINTN);

static bool add_overflow(UINT64 a, UINT64 b, UINT64* out)
{
    if (b > ~a)
        return true;
    *out = a + b;
    return false;
}

static bool range_overlap(UINT64 a, UINT64 aSize, UINT64 b, UINT64 bSize)
{
    UINT64 aEnd, bEnd;
    if (add_overflow(a, aSize, &aEnd) ||
        add_overflow(b, bSize, &bEnd))
        return true;
    return a < bEnd && b < aEnd;
}

static EFI_STATUS read_all(
    EFI_SYSTEM_TABLE* st,
    EFI_FILE_PROTOCOL* file,
    UINT8** outBuffer,
    UINTN* outSize)
{
    auto setPosition =
        reinterpret_cast<EFI_FILE_SET_POSITION>(file->SetPosition);
    auto getPosition =
        reinterpret_cast<EFI_FILE_GET_POSITION>(file->GetPosition);

    EFI_STATUS status = setPosition(file, ~0ULL);
    if (status != EFI_SUCCESS)
        return status;

    UINT64 fileSize64 = 0;
    status = getPosition(file, &fileSize64);
    if (status != EFI_SUCCESS || fileSize64 < sizeof(Elf64Header) ||
        fileSize64 > 0xFFFFFFFFULL)
        return EFI_INVALID_PARAMETER;

    auto allocatePool =
        reinterpret_cast<EFI_ALLOCATE_POOL>(st->BootServices->AllocatePool);

    void* buffer = nullptr;
    status = allocatePool(
        EfiLoaderData,
        static_cast<UINTN>(fileSize64),
        &buffer);
    if (status != EFI_SUCCESS)
        return status;

    status = setPosition(file, 0);
    if (status != EFI_SUCCESS)
    {
        auto freePool = reinterpret_cast<EFI_FREE_POOL>(st->BootServices->FreePool);
        freePool(buffer);
        return status;
    }

    UINTN size = static_cast<UINTN>(fileSize64);
    status = file->Read(file, &size, buffer);
    if (status != EFI_SUCCESS || size != fileSize64)
    {
        auto freePool = reinterpret_cast<EFI_FREE_POOL>(st->BootServices->FreePool);
        freePool(buffer);
        return status != EFI_SUCCESS ? status : EFI_INVALID_PARAMETER;
    }

    *outBuffer = reinterpret_cast<UINT8*>(buffer);
    *outSize = size;
    return EFI_SUCCESS;
}

EFI_STATUS load_kernel_elf(
    EFI_SYSTEM_TABLE* st,
    EFI_FILE_PROTOCOL* file,
    LoadedKernel* outKernel)
{
    if (!st || !file || !outKernel)
        return EFI_INVALID_PARAMETER;

    UINT8* image = nullptr;
    UINTN imageSize = 0;
    EFI_STATUS status = read_all(st, file, &image, &imageSize);
    if (status != EFI_SUCCESS)
        return status;

    auto freePool =
        reinterpret_cast<EFI_FREE_POOL>(st->BootServices->FreePool);

    const Elf64Header* eh = reinterpret_cast<const Elf64Header*>(image);

    if (eh->e_ident[0] != 0x7f || eh->e_ident[1] != 'E' ||
        eh->e_ident[2] != 'L' || eh->e_ident[3] != 'F' ||
        eh->e_ident[4] != ELFCLASS64 || eh->e_ident[5] != ELFDATA2LSB ||
        eh->e_ident[6] != 1 ||
        eh->e_type != ET_EXEC || eh->e_machine != EM_X86_64 ||
        eh->e_version != 1 ||
        eh->e_ehsize != sizeof(Elf64Header) ||
        eh->e_phentsize != sizeof(Elf64ProgramHeader) ||
        eh->e_phnum == 0 || eh->e_phoff == 0)
    {
        freePool(image);
        return EFI_INVALID_PARAMETER;
    }

    UINT64 phEnd;
    UINT64 phBytes = static_cast<UINT64>(eh->e_phnum) * eh->e_phentsize;
    if (eh->e_phoff > imageSize ||
        add_overflow(eh->e_phoff, phBytes, &phEnd) ||
        phEnd > imageSize)
    {
        freePool(image);
        return EFI_INVALID_PARAMETER;
    }

    const auto* ph = reinterpret_cast<const Elf64ProgramHeader*>(
        image + eh->e_phoff);

    UINT64 lowest = ~0ULL;
    UINT64 highest = 0;
    UINT16 loadCount = 0;
    bool entryExecutable = false;

    for (UINT16 i = 0; i < eh->e_phnum; ++i)
    {
        if (ph[i].p_type != PT_LOAD)
            continue;

        ++loadCount;

        UINT64 fileEnd, memEnd;
        if (ph[i].p_memsz < ph[i].p_filesz ||
            add_overflow(ph[i].p_offset, ph[i].p_filesz, &fileEnd) ||
            fileEnd > imageSize ||
            add_overflow(ph[i].p_vaddr, ph[i].p_memsz, &memEnd) ||
            ph[i].p_paddr != ph[i].p_vaddr ||
            ph[i].p_vaddr == 0 ||
            (ph[i].p_vaddr & (PAGE - 1)) != 0 ||
            ph[i].p_memsz == 0)
        {
            freePool(image);
            return EFI_INVALID_PARAMETER;
        }

        if (ph[i].p_align > 1 &&
             ((ph[i].p_align & (ph[i].p_align - 1)) != 0 ||
              (ph[i].p_vaddr % ph[i].p_align) !=
              (ph[i].p_offset % ph[i].p_align)))
        {
            freePool(image);
            return EFI_INVALID_PARAMETER;
        }

        if (ph[i].p_vaddr < lowest)
            lowest = ph[i].p_vaddr;
        if (memEnd > highest)
            highest = memEnd;

        if ((ph[i].p_flags & PF_X) &&
            eh->e_entry >= ph[i].p_vaddr &&
            eh->e_entry < memEnd)
        {
            /* Entry must point into bytes actually present in the file,
               not merely into a zero-filled executable BSS tail. */
            UINT64 fileBackedEnd = ph[i].p_vaddr + ph[i].p_filesz;
            if (eh->e_entry < fileBackedEnd)
                entryExecutable = true;
        }

        /* The current bootstrap loader cannot represent W+X permissions;
           reject such a kernel instead of silently loading an executable
           writable segment. */
        if ((ph[i].p_flags & (PF_X | 2U)) == (PF_X | 2U))
        {
            freePool(image);
            return EFI_INVALID_PARAMETER;
        }

        for (UINT16 j = 0; j < i; ++j)
        {
            if (ph[j].p_type != PT_LOAD)
                continue;
            if (range_overlap(
                    ph[i].p_vaddr, ph[i].p_memsz,
                    ph[j].p_vaddr, ph[j].p_memsz))
            {
                freePool(image);
                return EFI_INVALID_PARAMETER;
            }
        }
    }

    if (loadCount == 0 || !entryExecutable || eh->e_entry < lowest ||
        eh->e_entry >= highest)
    {
        freePool(image);
        return EFI_INVALID_PARAMETER;
    }

    auto allocatePages =
        reinterpret_cast<EFI_ALLOCATE_PAGES>(
            st->BootServices->AllocatePages);
    auto freePages =
        reinterpret_cast<EFI_FREE_PAGES>(
            st->BootServices->FreePages);

    auto rollback_segments = [&](UINT16 count)
    {
        for (UINT16 j = 0; j < count; ++j)
        {
            if (ph[j].p_type != PT_LOAD)
                continue;

            UINT64 segmentEnd = ph[j].p_vaddr + ph[j].p_memsz;
            UINT64 last = (segmentEnd + PAGE - 1) & ~(PAGE - 1);
            UINT64 pages = (last - ph[j].p_vaddr) / PAGE;
            if (pages != 0)
                freePages(ph[j].p_vaddr, static_cast<UINTN>(pages));
        }
    };

    for (UINT16 i = 0; i < eh->e_phnum; ++i)
    {
        if (ph[i].p_type != PT_LOAD)
            continue;

        UINT64 segmentEnd = ph[i].p_vaddr + ph[i].p_memsz;
        UINT64 last;
        if (add_overflow(segmentEnd, PAGE - 1, &last))
        {
            freePool(image);
            return EFI_INVALID_PARAMETER;
        }
        last &= ~(PAGE - 1);
        UINT64 pages = (last - ph[i].p_vaddr) / PAGE;

        if (last < segmentEnd || pages == 0 || pages > 0xFFFFFFFFULL)
        {
            freePool(image);
            return EFI_INVALID_PARAMETER;
        }

        EFI_PHYSICAL_ADDRESS address = ph[i].p_vaddr;
        const EFI_MEMORY_TYPE memoryType =
            (ph[i].p_flags & PF_X) ? EfiLoaderCode : EfiLoaderData;

        status = allocatePages(
            EFI_ALLOCATE_ADDRESS,
            memoryType,
            static_cast<UINTN>(pages),
            &address);

        if (status != EFI_SUCCESS || address != ph[i].p_vaddr)
        {
            if (status == EFI_SUCCESS && address != ph[i].p_vaddr)
                freePages(address, static_cast<UINTN>(pages));
            rollback_segments(i);
            freePool(image);
            return status != EFI_SUCCESS ? status : EFI_INVALID_PARAMETER;
        }

        UINT8* destination = reinterpret_cast<UINT8*>(address);
        for (UINT64 j = 0; j < ph[i].p_filesz; ++j)
            destination[j] = image[ph[i].p_offset + j];

        for (UINT64 j = ph[i].p_filesz; j < ph[i].p_memsz; ++j)
            destination[j] = 0;
    }

    const UINT64 loadedEntry = eh->e_entry;
    const UINT64 loadedBase = lowest;
    const UINT64 loadedSize = highest - lowest;

    freePool(image);

    outKernel->entry = loadedEntry;
    outKernel->base = loadedBase;
    outKernel->size = loadedSize;

    boot_debug("elf: kernel loaded\r\n");
    return EFI_SUCCESS;
}
