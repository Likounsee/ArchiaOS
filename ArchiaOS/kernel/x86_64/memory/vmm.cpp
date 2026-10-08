#include "vmm.hpp"
#include "paging.hpp"
#include "pmm.hpp"

static constexpr unsigned int VMM_MAX_REGIONS = 4096;
static VmmRegion regions[VMM_MAX_REGIONS];
static u64 allocated_pages = 0;
static bool initialized = false;

static bool valid_range(u64 base, u64 pages)
{
    if (pages == 0 || (base & (NOVOS_PAGE_SIZE - 1ULL)) != 0)
        return false;
    if (base < NOVOS_KERNEL_HEAP_BASE ||
        base >= NOVOS_KERNEL_HEAP_BASE + NOVOS_KERNEL_HEAP_SIZE)
        return false;
    if (pages > (NOVOS_KERNEL_HEAP_SIZE / NOVOS_PAGE_SIZE))
        return false;
    const u64 bytes = pages * NOVOS_PAGE_SIZE;
    return base <= NOVOS_KERNEL_HEAP_BASE + NOVOS_KERNEL_HEAP_SIZE - bytes;
}

static bool overlaps(u64 a, u64 pages, const VmmRegion& b)
{
    if (!b.used) return false;
    if (pages > (UINT64_MAX / NOVOS_PAGE_SIZE))
        return false;
    const u64 bytes = pages * NOVOS_PAGE_SIZE;
    return a < b.base + b.pages * NOVOS_PAGE_SIZE &&
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
    if (!initialized || pages == 0 || (writable && executable))
        return 0;
    if (pages > (NOVOS_KERNEL_HEAP_SIZE / NOVOS_PAGE_SIZE))
        return 0;

    u64 cursor = NOVOS_KERNEL_HEAP_BASE;
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
                const u64 end = regions[i].base + regions[i].pages * NOVOS_PAGE_SIZE;
                if (end > next) next = end;
            }
        }
        if (!conflict)
        {
            if (!valid_range(cursor, pages))
                return 0;

            for (u64 page = 0; page < pages; ++page)
            {
                const u64 va = cursor + page * NOVOS_PAGE_SIZE;
                if (paging_translate(va) != 0 || paging_get_4k_entry(va) != 0)
                    return 0;
            }

            for (unsigned int i = 0; i < VMM_MAX_REGIONS; ++i)
            {
                if (regions[i].used) continue;
                u64 mapped = 0;
                for (; mapped < pages; ++mapped)
                {
                    const u64 physical = pmm_alloc_page();
                    if (physical == 0)
                    {
                        for (u64 j = 0; j < mapped; ++j)
                        {
                            const u64 va = cursor + j * NOVOS_PAGE_SIZE;
                            const u64 pa = paging_translate(va);
                            paging_unmap_4k(va);
                            if (pa)
                                pmm_free_page(pa & ~0xFFFULL);
                        }
                        return 0;
                    }
                    if (!paging_map_4k(cursor + mapped * NOVOS_PAGE_SIZE,
                        physical, PagingFlags{writable, user, executable, false, false}))
                    {
                        pmm_free_page(physical);
                        for (u64 j = 0; j < mapped; ++j)
                        {
                            const u64 va = cursor + j * NOVOS_PAGE_SIZE;
                            const u64 pa = paging_translate(va);
                            paging_unmap_4k(va);
                            if (pa) pmm_free_page(pa & ~0xFFFULL);
                        }
                        return 0;
                    }
                }
                regions[i] = {cursor, pages, user, writable, executable, true};
                allocated_pages += pages;
                return cursor;
            }
            return 0;
        }
        cursor = next;
    }
    return 0;
}

extern "C" bool vmm_free_pages(u64 base, u64 pages)
{
    for (unsigned int i = 0; i < VMM_MAX_REGIONS; ++i)
    {
        if (!regions[i].used || regions[i].base != base || regions[i].pages != pages)
            continue;
        for (u64 page = 0; page < pages; ++page)
        {
            const u64 va = base + page * NOVOS_PAGE_SIZE;
            if (paging_translate(va) == 0 ||
                paging_get_4k_entry(va) == 0)
                return false;
        }

        for (u64 page = 0; page < pages; ++page)
        {
            const u64 va = base + page * NOVOS_PAGE_SIZE;
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
    if (!initialized || !valid_range(base, pages))
        return false;
    for (const auto& region : regions)
        if (overlaps(base, pages, region))
            return false;
    for (auto& region : regions)
    {
        if (!region.used)
        {
            region = {base, pages, user, true, false, true};
            return true;
        }
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
    if (vmm_alloc_pages(1, false, true, true) != 0)
        for (;;) asm volatile("cli; hlt");
    if (vmm_alloc_pages(UINT64_MAX, false, true, false) != 0 ||
        vmm_allocated_pages() != before)
        for (;;) asm volatile("cli; hlt");
    const u64 occupiedPhysical = pmm_alloc_page();
    if (occupiedPhysical == 0 ||
        !paging_map_4k(NOVOS_KERNEL_HEAP_BASE, occupiedPhysical,
                       PagingFlags{true, false, false, false, false}) ||
        vmm_alloc_pages(1, false, true, false) != 0)
        for (;;) asm volatile("cli; hlt");
    if (!paging_unmap_4k(NOVOS_KERNEL_HEAP_BASE))
        for (;;) asm volatile("cli; hlt");
    pmm_free_page(occupiedPhysical);

    const u64 base = vmm_alloc_pages(2, false, true, false);
    if (base == 0 || !vmm_is_mapped(base) || !vmm_is_mapped(base + NOVOS_PAGE_SIZE))
        for (;;) asm volatile("cli; hlt");
    auto* bytes = reinterpret_cast<volatile unsigned char*>(base);
    bytes[0] = 0xA5;
    bytes[NOVOS_PAGE_SIZE] = 0x5A;
    if (bytes[0] != 0xA5 || bytes[NOVOS_PAGE_SIZE] != 0x5A)
        for (;;) asm volatile("cli; hlt");
    if (!vmm_free_pages(base, 2) || vmm_allocated_pages() != before)
        for (;;) asm volatile("cli; hlt");

    const u64 reserved = NOVOS_KERNEL_HEAP_BASE + 8 * NOVOS_PAGE_SIZE;
    if (!vmm_reserve(reserved, 2, false) ||
        vmm_reserve(reserved + NOVOS_PAGE_SIZE, 1, false))
        for (;;) asm volatile("cli; hlt");

    const u64 allocated_around_reservation =
        vmm_alloc_pages(2, false, true, false);
    if (!allocated_around_reservation ||
        allocated_around_reservation == reserved ||
        !vmm_free_pages(allocated_around_reservation, 2) ||
        vmm_allocated_pages() != before)
        for (;;) asm volatile("cli; hlt");
}
