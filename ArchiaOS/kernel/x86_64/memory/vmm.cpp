#include "vmm.hpp"
#include "paging.hpp"
#include "pmm.hpp"

static constexpr unsigned int VMM_MAX_REGIONS = 4096;
static VmmRegion regions[VMM_MAX_REGIONS];
static u64 allocated_pages = 0;
static bool initialized = false;

static bool valid_range(u64 base, u64 pages)
{
    if (pages == 0 || (base & (PAGE_SIZE - 1ULL)) != 0)
        return false;
    if (base < KERNEL_HEAP_BASE ||
        base >= KERNEL_HEAP_BASE + KERNEL_HEAP_SIZE)
        return false;
    if (pages > (KERNEL_HEAP_SIZE / PAGE_SIZE))
        return false;
    const u64 bytes = pages * PAGE_SIZE;
    return base <= KERNEL_HEAP_BASE + KERNEL_HEAP_SIZE - bytes;
}

static bool overlaps(u64 a, u64 pages, const VmmRegion& b)
{
    if (!b.used ||
        pages == 0 ||
        b.pages == 0 ||
        pages > UINT64_MAX / PAGE_SIZE ||
        b.pages > UINT64_MAX / PAGE_SIZE)
        return false;

    const u64 bytes = pages * PAGE_SIZE;
    const u64 other_bytes = b.pages * PAGE_SIZE;

    /* Treat corrupted metadata as non-overlapping only after proving both
       half-open ranges can be represented without wrapping. */
    if (a > UINT64_MAX - bytes ||
        b.base > UINT64_MAX - other_bytes)
        return false;

    return a < b.base + other_bytes &&
           b.base < a + bytes;
}

extern "C" bool vmm_initialize()
{
    for (auto& region : regions)
        region = {};
    allocated_pages = 0;
    initialized = true;
    return true;
}

extern "C" u64 vmm_alloc_pages(
    u64 pages, bool user, bool writable, bool executable)
{
    if (!initialized || pages == 0 || user || (writable && executable))
        return 0;
    if (pages > (KERNEL_HEAP_SIZE / PAGE_SIZE))
        return 0;

    u64 cursor = KERNEL_HEAP_BASE;
    for (unsigned int slot = 0; slot < VMM_MAX_REGIONS; ++slot)
    {
        bool conflict = false;
        u64 next = cursor;
        for (unsigned int i = 0; i < VMM_MAX_REGIONS; ++i)
        {
            if (!regions[i].used) continue;
            if (overlaps(cursor, pages, regions[i]))
            {
                conflict = true;
                const u64 end = regions[i].base + regions[i].pages * PAGE_SIZE;
                if (end > next) next = end;
            }
        }
        if (!conflict)
        {
            if (!valid_range(cursor, pages))
                return 0;

            for (u64 page = 0; page < pages; ++page)
            {
                const u64 va = cursor + page * PAGE_SIZE;
                if (paging_translate(va) != 0 || paging_get_4k_entry(va) != 0)
                    return 0;
            }

            unsigned int free_slot = VMM_MAX_REGIONS;
            for (unsigned int i = 0; i < VMM_MAX_REGIONS; ++i)
            {
                if (!regions[i].used)
                {
                    free_slot = i;
                    break;
                }
            }
            if (free_slot == VMM_MAX_REGIONS)
                return 0;

            u64 mapped = 0;
            for (; mapped < pages; ++mapped)
            {
                const u64 physical = pmm_alloc_page();
                if (physical == 0)
                {
                    for (u64 j = 0; j < mapped; ++j)
                    {
                        const u64 va = cursor + j * PAGE_SIZE;
                        const u64 pa = paging_translate(va);
                        paging_unmap_4k(va);
                        if (pa)
                            pmm_free_page(pa & ~0xFFFULL);
                    }
                    return 0;
                }
                if (!paging_map_4k(cursor + mapped * PAGE_SIZE,
                    physical, PagingFlags{writable, user, executable, false, false}))
                {
                    pmm_free_page(physical);
                    for (u64 j = 0; j < mapped; ++j)
                    {
                        const u64 va = cursor + j * PAGE_SIZE;
                        const u64 pa = paging_translate(va);
                        paging_unmap_4k(va);
                        if (pa)
                            pmm_free_page(pa & ~0xFFFULL);
                    }
                    return 0;
                }
            }
            regions[free_slot] = {
                cursor, pages, user, writable, executable, true, true
            };
            allocated_pages += pages;
            return cursor;
        }
        cursor = next;
    }
    return 0;
}

extern "C" bool vmm_free_pages(u64 base, u64 pages)
{
    for (unsigned int i = 0; i < VMM_MAX_REGIONS; ++i)
    {
        if (!regions[i].used || !regions[i].mapped ||
            regions[i].base != base || regions[i].pages != pages)
            continue;
        for (u64 page = 0; page < pages; ++page)
        {
            const u64 va = base + page * PAGE_SIZE;
            if (paging_translate(va) == 0 ||
                paging_get_4k_entry(va) == 0)
                return false;
        }

        for (u64 page = 0; page < pages; ++page)
        {
            const u64 va = base + page * PAGE_SIZE;
            const u64 pa = paging_translate(va);
            if (!paging_unmap_4k(va))
                return false;
            if (pa)
                pmm_free_page(pa & ~0xFFFULL);
        }
        regions[i].used = false;
        allocated_pages -= pages;
        return true;
    }
    return false;
}

extern "C" bool vmm_reserve(u64 base, u64 pages, bool user)
{
    if (!initialized || user || !valid_range(base, pages))
        return false;
    for (const auto& region : regions)
        if (overlaps(base, pages, region))
            return false;

    /*
     * A reservation is metadata-only, but it must never claim virtual
     * addresses that are already backed by page tables. Otherwise a later
     * allocator operation could observe a free VMM slot while the paging
     * hierarchy already owns those pages.
     */
    for (u64 page = 0; page < pages; ++page)
    {
        const u64 va = base + page * PAGE_SIZE;
        if (paging_translate(va) != 0 || paging_get_4k_entry(va) != 0)
            return false;
    }

    for (auto& region : regions)
    {
        if (!region.used)
        {
            region = {base, pages, user, false, false, false, true};
            return true;
        }
    }
    return false;
}

extern "C" bool vmm_release(u64 base, u64 pages)
{
    for (auto& region : regions)
    {
        if (!region.used || region.mapped ||
            region.base != base || region.pages != pages)
            continue;

        region = {};
        return true;
    }
    return false;
}

extern "C" bool vmm_is_mapped(u64 virtual_address)
{
    return paging_translate(virtual_address) != 0;
}

extern "C" u64 vmm_allocated_pages()
{
    return allocated_pages;
}

extern "C" void vmm_run_tests()
{
    const u64 before = vmm_allocated_pages();
    if (vmm_alloc_pages(1, true, true, false) != 0 ||
        vmm_reserve(KERNEL_HEAP_BASE, 1, true) ||
        vmm_allocated_pages() != before)
        for (;;) asm volatile("cli; hlt");
    if (vmm_alloc_pages(1, false, true, true) != 0)
        for (;;) asm volatile("cli; hlt");
    if (vmm_alloc_pages(UINT64_MAX, false, true, false) != 0 ||
        vmm_allocated_pages() != before)
        for (;;) asm volatile("cli; hlt");
    const u64 occupiedPhysical = pmm_alloc_page();
    if (occupiedPhysical == 0 ||
        !paging_map_4k(KERNEL_HEAP_BASE, occupiedPhysical,
                       PagingFlags{true, false, false, false, false}) ||
        vmm_alloc_pages(1, false, true, false) != 0 ||
        vmm_reserve(KERNEL_HEAP_BASE, 1, false))
        for (;;) asm volatile("cli; hlt");
    if (!paging_unmap_4k(KERNEL_HEAP_BASE))
        for (;;) asm volatile("cli; hlt");
    pmm_free_page(occupiedPhysical);

    const u64 reserved = KERNEL_HEAP_BASE;
    if (!vmm_reserve(reserved, 2, false))
        for (;;) asm volatile("cli; hlt");
    const u64 allocation_around_reservation =
        vmm_alloc_pages(2, false, true, false);
    if (allocation_around_reservation == 0 ||
        allocation_around_reservation == reserved ||
        vmm_is_mapped(reserved) ||
        !vmm_free_pages(allocation_around_reservation, 2) ||
        vmm_free_pages(reserved, 2) ||
        !vmm_release(reserved, 2) ||
        vmm_release(reserved, 2))
        for (;;) asm volatile("cli; hlt");

    const u64 base = vmm_alloc_pages(2, false, true, false);
    if (base == 0 || !vmm_is_mapped(base) || !vmm_is_mapped(base + PAGE_SIZE))
        for (;;) asm volatile("cli; hlt");
    auto* bytes = reinterpret_cast<volatile unsigned char*>(base);
    bytes[0] = 0xA5;
    bytes[PAGE_SIZE] = 0x5A;
    if (bytes[0] != 0xA5 || bytes[PAGE_SIZE] != 0x5A)
        for (;;) asm volatile("cli; hlt");
    if (!vmm_free_pages(base, 2) || vmm_allocated_pages() != before)
        for (;;) asm volatile("cli; hlt");

}
