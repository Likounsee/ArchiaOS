#include "address_space.hpp"
#include "paging.hpp"
#include "pmm.hpp"

static inline void zero_table(u64 physical)
{
    auto* table = reinterpret_cast<u64*>(paging_physical_to_virtual(physical));
    for (unsigned int i = 0; i < 512; ++i) table[i] = 0;
}

static u64 new_table()
{
    const u64 physical = pmm_alloc_page_above(0x01000000ULL);
    if (physical) zero_table(physical);
    return physical;
}

static u64* table(u64 physical)
{
    return reinterpret_cast<u64*>(paging_physical_to_virtual(physical));
}

extern "C" bool address_space_create(AddressSpace* space)
{
    if (!space) return false;
    const u64 pml4 = new_table();
    if (!pml4) return false;

    auto* dst = table(pml4);
    auto* src = table(paging_pml4_physical());
    if (!dst || !src) { pmm_free_page(pml4); return false; }

    /* Upper-half kernel/HHDM mappings are shared read-only page-table roots. */
    for (unsigned int i = 256; i < 512; ++i)
        dst[i] = src[i];

    space->pml4_physical = pml4;
    space->active = false;
    return true;
}

extern "C" bool address_space_map(
    AddressSpace* space, u64 virtual_address, u64 physical_address,
    bool writable, bool executable)
{
    if (!space || !space->pml4_physical ||
        (virtual_address & 0xFFFULL) ||
        (physical_address & 0xFFFULL) ||
        physical_address >= NOVOS_PMM_MAX_PHYSICAL_ADDRESS ||
        virtual_address < NOVOS_USER_VIRTUAL_BASE ||
        virtual_address >= NOVOS_USER_VIRTUAL_TOP)
        return false;

    auto* pml4 = table(space->pml4_physical);
    if (!pml4) return false;

    const unsigned int i4 = (virtual_address >> 39) & 0x1FF;
    const unsigned int i3 = (virtual_address >> 30) & 0x1FF;
    const unsigned int i2 = (virtual_address >> 21) & 0x1FF;
    const unsigned int i1 = (virtual_address >> 12) & 0x1FF;

    auto ensure = [](u64& entry) -> bool {
        if (entry & NOVOS_PAGE_PRESENT) return true;
        const u64 page = new_table();
        if (!page) return false;
        entry = page | NOVOS_PAGE_PRESENT | NOVOS_PAGE_WRITE | NOVOS_PAGE_USER;
        return true;
    };

    if (!ensure(pml4[i4])) return false;
    auto* pdpt = table(pml4[i4] & ~0xFFFULL);
    if (!ensure(pdpt[i3])) return false;
    if (pdpt[i3] & NOVOS_PAGE_HUGE) return false;

    auto* pd = table(pdpt[i3] & ~0xFFFULL);
    if (!ensure(pd[i2])) return false;
    if (pd[i2] & NOVOS_PAGE_HUGE) return false;

    auto* pt = table(pd[i2] & ~0xFFFULL);
    if (!pt) return false;

    u64 flags = NOVOS_PAGE_PRESENT | NOVOS_PAGE_USER;
    if (writable) flags |= NOVOS_PAGE_WRITE;
    if (!executable) flags |= NOVOS_PAGE_NO_EXECUTE;
    pt[i1] = physical_address | flags;

    return true;
}

static void destroy_table_level(u64 physical, unsigned int level)
{
    auto* entries = table(physical);
    if (!entries)
        return;

    for (unsigned int i = 0; i < 512; ++i)
    {
        const u64 entry = entries[i];
        if (!(entry & NOVOS_PAGE_PRESENT))
            continue;

        if (level == 1)
        {
            pmm_free_page(entry & ~0xFFFULL);
            entries[i] = 0;
            continue;
        }

        if (entry & NOVOS_PAGE_HUGE)
        {
            entries[i] = 0;
            continue;
        }

        const u64 child = entry & ~0xFFFULL;
        destroy_table_level(child, level - 1);
        pmm_free_page(child);
        entries[i] = 0;
    }
}

extern "C" bool address_space_destroy(AddressSpace* space)
{
    if (!space || !space->pml4_physical)
        return false;

    u64 current_cr3 = 0;
    asm volatile("mov %%cr3, %0" : "=r"(current_cr3));
    if (current_cr3 == space->pml4_physical)
        return false;

    auto* pml4 = table(space->pml4_physical);
    if (!pml4)
        return false;

    for (unsigned int i = 0; i < 256; ++i)
    {
        const u64 entry = pml4[i];
        if (!(entry & NOVOS_PAGE_PRESENT))
            continue;
        if (entry & NOVOS_PAGE_HUGE)
            continue;

        const u64 child = entry & ~0xFFFULL;
        destroy_table_level(child, 3);
        pmm_free_page(child);
        pml4[i] = 0;
    }

    const u64 old = space->pml4_physical;
    space->pml4_physical = 0;
    space->active = false;
    pmm_free_page(old);
    return true;
}

extern "C" bool address_space_activate(AddressSpace* space)
{
    if (!space || !space->pml4_physical) return false;
    asm volatile("mov %0, %%cr3" : : "r"(space->pml4_physical) : "memory");
    space->active = true;
    return true;
}

extern "C" bool address_space_is_user_mapped(
    const AddressSpace* space, u64 virtual_address)
{
    if (!space || !space->pml4_physical) return false;
    auto* pml4 = table(space->pml4_physical);
    if (!pml4) return false;
    const u64 e4 = pml4[(virtual_address >> 39) & 0x1FF];
    if (!(e4 & NOVOS_PAGE_PRESENT) || !(e4 & NOVOS_PAGE_USER)) return false;
    auto* pdpt = table(e4 & ~0xFFFULL);
    const u64 e3 = pdpt[(virtual_address >> 30) & 0x1FF];
    if (!(e3 & NOVOS_PAGE_PRESENT) || !(e3 & NOVOS_PAGE_USER)) return false;
    if (e3 & NOVOS_PAGE_HUGE) return true;
    auto* pd = table(e3 & ~0xFFFULL);
    const u64 e2 = pd[(virtual_address >> 21) & 0x1FF];
    if (!(e2 & NOVOS_PAGE_PRESENT) || !(e2 & NOVOS_PAGE_USER)) return false;
    if (e2 & NOVOS_PAGE_HUGE) return true;
    auto* pt = table(e2 & ~0xFFFULL);
    return (pt[(virtual_address >> 12) & 0x1FF] &
            (NOVOS_PAGE_PRESENT | NOVOS_PAGE_USER)) ==
           (NOVOS_PAGE_PRESENT | NOVOS_PAGE_USER);
}

extern "C" void address_space_run_tests()
{
    AddressSpace space{};
    if (!address_space_create(&space))
        for (;;) asm volatile("cli; hlt");

    if (address_space_map(
            &space, NOVOS_USER_VIRTUAL_BASE, NOVOS_PMM_MAX_PHYSICAL_ADDRESS,
            true, false))
        for (;;) asm volatile("cli; hlt");

    const u64 physical = pmm_alloc_page();
    if (!physical ||
        !address_space_map(&space, NOVOS_USER_VIRTUAL_BASE, physical, true, false) ||
        !address_space_is_user_mapped(&space, NOVOS_USER_VIRTUAL_BASE))
        for (;;) asm volatile("cli; hlt");

    pmm_free_page(physical);
}
