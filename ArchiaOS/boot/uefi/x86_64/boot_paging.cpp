#include "boot_paging.h"

static constexpr UINT64 PAGE_SIZE = 4096ULL;
static constexpr UINT64 HUGE_PAGE_SIZE = 0x200000ULL;
static constexpr UINT64 KERNEL_BASE_MIN = 0xFFFFFFFF80000000ULL;
static constexpr UINT64 MAX_PHYSICAL = 0x8000000000ULL;
static constexpr UINT64 PRESENT = 1ULL << 0;
static constexpr UINT64 WRITE = 1ULL << 1;
static constexpr UINT64 HUGE = 1ULL << 7;

static EFI_STATUS alloc_page(
    EFI_SYSTEM_TABLE* st,
    EFI_PHYSICAL_ADDRESS* out)
{
    auto allocatePages =
        reinterpret_cast<EFI_ALLOCATE_PAGES>(
            st->BootServices->AllocatePages);
    auto setMem =
        reinterpret_cast<EFI_SET_MEM>(
            st->BootServices->SetMem);

    if (!allocatePages || !setMem || !out)
        return EFI_INVALID_PARAMETER;

    EFI_PHYSICAL_ADDRESS address = 0;
    EFI_STATUS status = allocatePages(
        EFI_ALLOCATE_ANY_PAGES,
        EfiLoaderData,
        1,
        &address);
    if (status != EFI_SUCCESS)
        return status;

    setMem(reinterpret_cast<void*>(address), PAGE_SIZE, 0);
    *out = address;
    return EFI_SUCCESS;
}

static UINT64 max_mapped_physical(
    const FinalMemoryMap* map)
{
    if (!map || !map->buffer ||
        map->descriptorSize < sizeof(EFI_MEMORY_DESCRIPTOR))
        return 0;

    UINT64 highest = 0;

    for (UINTN offset = 0; offset < map->size; offset += map->descriptorSize)
    {
        const auto* d =
            reinterpret_cast<const EFI_MEMORY_DESCRIPTOR*>(
                reinterpret_cast<const UINT8*>(map->buffer) + offset);

        if (d->Type != EfiLoaderCode &&
            d->Type != EfiLoaderData &&
            d->Type != EfiBootServicesCode &&
            d->Type != EfiBootServicesData &&
            d->Type != EfiConventionalMemory)
            continue;

        if (d->PhysicalStart >= MAX_PHYSICAL)
            continue;

        UINT64 pages = d->NumberOfPages;
        const UINT64 available =
            (MAX_PHYSICAL - d->PhysicalStart) / PAGE_SIZE;
        if (pages > available)
            pages = available;

        const UINT64 end =
            d->PhysicalStart + pages * PAGE_SIZE;
        if (end > highest)
            highest = end;
    }

    return highest;
}

static bool add_overflow(UINT64 a, UINT64 b, UINT64* out)
{
    if (b > ~a)
        return true;
    *out = a + b;
    return false;
}

EFI_STATUS prepare_boot_paging(
    EFI_SYSTEM_TABLE* st,
    const FinalMemoryMap* memoryMap,
    const LoadedKernel* kernel,
    UINT64* outPml4Physical)
{
    if (!st || !st->BootServices || !memoryMap || !kernel ||
        !outPml4Physical || kernel->base == 0 || kernel->size == 0)
        return EFI_INVALID_PARAMETER;

    if (kernel->virtual_base < KERNEL_BASE_MIN ||
        (kernel->virtual_base >> 48) != 0xFFFFULL)
        return EFI_INVALID_PARAMETER;

    UINT64 kernelEnd;
    if (add_overflow(kernel->virtual_base, kernel->size, &kernelEnd))
        return EFI_INVALID_PARAMETER;

    UINT64 physicalEnd;
    if (add_overflow(kernel->base, kernel->size, &physicalEnd) ||
        physicalEnd > MAX_PHYSICAL)
        return EFI_INVALID_PARAMETER;

    UINT64 highest = max_mapped_physical(memoryMap);
    if (highest < physicalEnd)
        highest = physicalEnd;

    highest = (highest + HUGE_PAGE_SIZE - 1) & ~(HUGE_PAGE_SIZE - 1);
    if (highest == 0 || highest > MAX_PHYSICAL)
        return EFI_INVALID_PARAMETER;

    const UINT64 pdCount =
        (highest + 0x3FFFFFFFULL) / 0x40000000ULL;

    EFI_PHYSICAL_ADDRESS pml4Physical = 0;
    EFI_PHYSICAL_ADDRESS identityPdptPhysical = 0;
    EFI_PHYSICAL_ADDRESS kernelPdptPhysical = 0;

    EFI_STATUS status = alloc_page(st, &pml4Physical);
    if (status != EFI_SUCCESS)
        return status;

    status = alloc_page(st, &identityPdptPhysical);
    if (status != EFI_SUCCESS)
        return status;

    status = alloc_page(st, &kernelPdptPhysical);
    if (status != EFI_SUCCESS)
        return status;

    auto* pml4 = reinterpret_cast<UINT64*>(pml4Physical);
    auto* identityPdpt = reinterpret_cast<UINT64*>(identityPdptPhysical);
    auto* kernelPdpt = reinterpret_cast<UINT64*>(kernelPdptPhysical);

    pml4[0] = identityPdptPhysical | PRESENT | WRITE;
    pml4[256] = identityPdptPhysical | PRESENT | WRITE;
    pml4[511] = kernelPdptPhysical | PRESENT | WRITE;

    /*
     * Identity + HHDM mappings cover all RAM visible to the bootstrap PMM
     * limit. 2 MiB leaves work on CPUs without 1 GiB page support.
     */
    for (UINT64 pdptIndex = 0; pdptIndex < pdCount; ++pdptIndex)
    {
        EFI_PHYSICAL_ADDRESS pdPhysical = 0;
        status = alloc_page(st, &pdPhysical);
        if (status != EFI_SUCCESS)
            return status;

        auto* pd = reinterpret_cast<UINT64*>(pdPhysical);
        identityPdpt[pdptIndex] = pdPhysical | PRESENT | WRITE;

        const UINT64 physicalBase =
            pdptIndex * 0x40000000ULL;

        for (UINT64 pdIndex = 0; pdIndex < 512; ++pdIndex)
        {
            const UINT64 physical =
                physicalBase + pdIndex * HUGE_PAGE_SIZE;

            if (physical >= highest)
                break;

            pd[pdIndex] =
                physical | PRESENT | WRITE | HUGE;
        }
    }

    /*
     * Map the linked higher-half kernel VMA to its independently allocated
     * physical image. Because the ELF is linked at its final VMA, absolute
     * kernel addresses remain valid and no runtime relocation is required.
     */
    UINT64 mapped = 0;
    while (mapped < kernel->size)
    {
        const UINT64 virtualAddress =
            kernel->virtual_base + mapped;

        const UINT64 pml4Index =
            (virtualAddress >> 39) & 0x1FFULL;
        const UINT64 pdptIndex =
            (virtualAddress >> 30) & 0x1FFULL;
        const UINT64 pdIndex =
            (virtualAddress >> 21) & 0x1FFULL;
        const UINT64 ptIndex =
            (virtualAddress >> 12) & 0x1FFULL;

        if (pml4Index != 511)
            return EFI_INVALID_PARAMETER;

        UINT64 pdpt = kernelPdpt[pdptIndex];
        EFI_PHYSICAL_ADDRESS pdPhysical = 0;

        if ((pdpt & PRESENT) == 0)
        {
            status = alloc_page(st, &pdPhysical);
            if (status != EFI_SUCCESS)
                return status;

            kernelPdpt[pdptIndex] =
                pdPhysical | PRESENT | WRITE;
        }
        else
        {
            pdPhysical =
                static_cast<EFI_PHYSICAL_ADDRESS>(
                    pdpt & ~0xFFFULL);
        }

        auto* pd =
            reinterpret_cast<UINT64*>(pdPhysical);

        UINT64 pde = pd[pdIndex];
        EFI_PHYSICAL_ADDRESS ptPhysical = 0;

        if ((pde & PRESENT) == 0)
        {
            status = alloc_page(st, &ptPhysical);
            if (status != EFI_SUCCESS)
                return status;

            pd[pdIndex] =
                ptPhysical | PRESENT | WRITE;
        }
        else
        {
            ptPhysical =
                static_cast<EFI_PHYSICAL_ADDRESS>(
                    pde & ~0xFFFULL);
        }

        auto* pt =
            reinterpret_cast<UINT64*>(ptPhysical);

        pt[ptIndex] =
            (kernel->base + mapped) |
            PRESENT |
            WRITE;

        mapped += PAGE_SIZE;
    }

    *outPml4Physical = pml4Physical;
    return EFI_SUCCESS;
}
