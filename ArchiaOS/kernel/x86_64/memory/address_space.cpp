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
    if (!physical)
        return 0;

    if (paging_physical_to_virtual(physical) == 0)
    {
        pmm_free_page(physical);
        return 0;
    }

    zero_table(physical);
    return physical;
}

static u64* table(u64 physical)
{
    return reinterpret_cast<u64*>(paging_physical_to_virtual(physical));
}

extern "C" bool address_space_create(AddressSpace* space)
{
    if (!space || space->pml4_physical != 0 || space->active)
        return false;
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
        physical_address == 0 ||
        physical_address >= paging_max_physical_address() ||
        (writable && executable) ||
        virtual_address < NOVOS_USER_VIRTUAL_BASE ||
        virtual_address >= NOVOS_USER_VIRTUAL_TOP)
        return false;

    auto* pml4 = table(space->pml4_physical);
    if (!pml4) return false;

    const unsigned int i4 = (virtual_address >> 39) & 0x1FF;
    const unsigned int i3 = (virtual_address >> 30) & 0x1FF;
    const unsigned int i2 = (virtual_address >> 21) & 0x1FF;
    const unsigned int i1 = (virtual_address >> 12) & 0x1FF;

    bool created_pdpt = false;
    bool created_pd = false;
    bool created_pt = false;
    u64 pdpt_physical = 0;
    u64 pd_physical = 0;
    u64 pt_physical = 0;
    u64* pdpt = nullptr;
    u64* pd = nullptr;
    u64* pt = nullptr;

    auto rollback_tables = [&]() {
        if (created_pt)
        {
            pd[i2] = 0;
            pmm_free_page(pt_physical);
        }
        if (created_pd)
        {
            pdpt[i3] = 0;
            pmm_free_page(pd_physical);
        }
        if (created_pdpt)
        {
            pml4[i4] = 0;
            pmm_free_page(pdpt_physical);
        }
    };

    if (pml4[i4] & NOVOS_PAGE_HUGE)
        return false;

    if (!(pml4[i4] & NOVOS_PAGE_PRESENT))
    {
        const u64 page = new_table();
        if (!page)
            return false;
        pml4[i4] = page | NOVOS_PAGE_PRESENT | NOVOS_PAGE_WRITE | NOVOS_PAGE_USER;
        created_pdpt = true;
    }

    const u64 pml4_flags = pml4[i4];
    if (!(pml4_flags & NOVOS_PAGE_USER) ||
        (executable && (pml4_flags & NOVOS_PAGE_NO_EXECUTE)) ||
        (writable && !(pml4_flags & NOVOS_PAGE_WRITE)))
    {
        rollback_tables();
        return false;
    }

    pdpt_physical = pml4_flags & ~0xFFFULL;
    pdpt = table(pdpt_physical);
    if (!pdpt)
    {
        rollback_tables();
        return false;
    }

    if (pdpt[i3] & NOVOS_PAGE_HUGE)
    {
        rollback_tables();
        return false;
    }

    if (!(pdpt[i3] & NOVOS_PAGE_PRESENT))
    {
        const u64 page = new_table();
        if (!page)
        {
            rollback_tables();
            return false;
        }
        pdpt[i3] = page | NOVOS_PAGE_PRESENT | NOVOS_PAGE_WRITE | NOVOS_PAGE_USER;
        created_pd = true;
    }

    const u64 pdpt_flags = pdpt[i3];
    if (!(pdpt_flags & NOVOS_PAGE_USER) ||
        (executable && (pdpt_flags & NOVOS_PAGE_NO_EXECUTE)) ||
        (writable && !(pdpt_flags & NOVOS_PAGE_WRITE)))
    {
        rollback_tables();
        return false;
    }

    pd_physical = pdpt_flags & ~0xFFFULL;
    pd = table(pd_physical);
    if (!pd)
    {
        rollback_tables();
        return false;
    }

    if (pd[i2] & NOVOS_PAGE_HUGE)
    {
        rollback_tables();
        return false;
    }

    if (!(pd[i2] & NOVOS_PAGE_PRESENT))
    {
        const u64 page = new_table();
        if (!page)
        {
            rollback_tables();
            return false;
        }
        pd[i2] = page | NOVOS_PAGE_PRESENT | NOVOS_PAGE_WRITE | NOVOS_PAGE_USER;
        created_pt = true;
    }

    const u64 pd_flags = pd[i2];
    if (!(pd_flags & NOVOS_PAGE_USER) ||
        (executable && (pd_flags & NOVOS_PAGE_NO_EXECUTE)) ||
        (writable && !(pd_flags & NOVOS_PAGE_WRITE)))
    {
        rollback_tables();
        return false;
    }

    pt_physical = pd_flags & ~0xFFFULL;
    pt = table(pt_physical);
    if (!pt)
    {
        rollback_tables();
        return false;
    }

    if (pt[i1] & NOVOS_PAGE_PRESENT)
    {
        rollback_tables();
        return false;
    }

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
    current_cr3 &= ~0xFFFULL;
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
    if (!space || !space->pml4_physical ||
        virtual_address < NOVOS_USER_VIRTUAL_BASE ||
        virtual_address >= NOVOS_USER_VIRTUAL_TOP)
        return false;
    auto* pml4 = table(space->pml4_physical);
    if (!pml4) return false;
    const u64 e4 = pml4[(virtual_address >> 39) & 0x1FF];
    if (!(e4 & NOVOS_PAGE_PRESENT) || !(e4 & NOVOS_PAGE_USER)) return false;
    auto* pdpt = table(e4 & ~0xFFFULL);
    if (!pdpt) return false;
    const u64 e3 = pdpt[(virtual_address >> 30) & 0x1FF];
    if (!(e3 & NOVOS_PAGE_PRESENT) || !(e3 & NOVOS_PAGE_USER)) return false;
    if (e3 & NOVOS_PAGE_HUGE) return true;
    auto* pd = table(e3 & ~0xFFFULL);
    if (!pd) return false;
    const u64 e2 = pd[(virtual_address >> 21) & 0x1FF];
    if (!(e2 & NOVOS_PAGE_PRESENT) || !(e2 & NOVOS_PAGE_USER)) return false;
    if (e2 & NOVOS_PAGE_HUGE) return true;
    auto* pt = table(e2 & ~0xFFFULL);
    if (!pt) return false;
    return (pt[(virtual_address >> 12) & 0x1FF] &
            (NOVOS_PAGE_PRESENT | NOVOS_PAGE_USER)) ==
           (NOVOS_PAGE_PRESENT | NOVOS_PAGE_USER);
}

extern "C" bool address_space_is_user_executable(
    const AddressSpace* space, u64 virtual_address)
{
    if (!space || !space->pml4_physical ||
        virtual_address < NOVOS_USER_VIRTUAL_BASE ||
        virtual_address >= NOVOS_USER_VIRTUAL_TOP)
        return false;

    auto* pml4 = table(space->pml4_physical);
    if (!pml4) return false;

    const u64 e4 = pml4[(virtual_address >> 39) & 0x1FF];
    if (!(e4 & NOVOS_PAGE_PRESENT) || !(e4 & NOVOS_PAGE_USER) ||
        (e4 & NOVOS_PAGE_NO_EXECUTE))
        return false;

    auto* pdpt = table(e4 & ~0xFFFULL);
    if (!pdpt) return false;
    const u64 e3 = pdpt[(virtual_address >> 30) & 0x1FF];
    if (!(e3 & NOVOS_PAGE_PRESENT) || !(e3 & NOVOS_PAGE_USER) ||
        (e3 & NOVOS_PAGE_NO_EXECUTE))
        return false;
    if (e3 & NOVOS_PAGE_HUGE) return true;

    auto* pd = table(e3 & ~0xFFFULL);
    if (!pd) return false;
    const u64 e2 = pd[(virtual_address >> 21) & 0x1FF];
    if (!(e2 & NOVOS_PAGE_PRESENT) || !(e2 & NOVOS_PAGE_USER) ||
        (e2 & NOVOS_PAGE_NO_EXECUTE))
        return false;
    if (e2 & NOVOS_PAGE_HUGE) return true;

    auto* pt = table(e2 & ~0xFFFULL);
    if (!pt) return false;
    const u64 e1 = pt[(virtual_address >> 12) & 0x1FF];
    return (e1 & (NOVOS_PAGE_PRESENT | NOVOS_PAGE_USER)) ==
               (NOVOS_PAGE_PRESENT | NOVOS_PAGE_USER) &&
           (e1 & NOVOS_PAGE_NO_EXECUTE) == 0;
}

extern "C" void address_space_run_tests()
{
    AddressSpace space{};
    if (!address_space_create(&space))
        for (;;) asm volatile("cli; hlt");

    AddressSpace occupied = space;
    if (address_space_create(&occupied))
        for (;;) asm volatile("cli; hlt");

    if (address_space_map(
            &space, NOVOS_USER_VIRTUAL_BASE, 0,
            true, false) ||
        address_space_map(
            &space, NOVOS_USER_VIRTUAL_BASE, NOVOS_PMM_MAX_PHYSICAL_ADDRESS,
            true, false))
        for (;;) asm volatile("cli; hlt");

    const u64 paging_limit = paging_max_physical_address();
    if (paging_limit == 0 ||
        address_space_map(
            &space, NOVOS_USER_VIRTUAL_BASE + NOVOS_PAGE_SIZE,
            paging_limit, true, false))
        for (;;) asm volatile("cli; hlt");

    const u64 physical = pmm_alloc_page();
    if (!physical ||
        !address_space_map(&space, NOVOS_USER_VIRTUAL_BASE, physical, true, false) ||
        !address_space_is_user_mapped(&space, NOVOS_USER_VIRTUAL_BASE))
        for (;;) asm volatile("cli; hlt");

    if (address_space_map(
            &space, NOVOS_USER_VIRTUAL_BASE, physical, true, false))
        for (;;) asm volatile("cli; hlt");

    if (address_space_map(
            &space, NOVOS_USER_VIRTUAL_BASE + NOVOS_PAGE_SIZE,
            physical, true, true) ||
        address_space_is_user_mapped(
            &space, NOVOS_USER_VIRTUAL_BASE + NOVOS_PAGE_SIZE))
        for (;;) asm volatile("cli; hlt");

    if (address_space_is_user_mapped(&space, NOVOS_USER_VIRTUAL_TOP))
        for (;;) asm volatile("cli; hlt");

    const u64 executable_physical = pmm_alloc_page();
    if (!executable_physical ||
        !address_space_map(
            &space, NOVOS_USER_VIRTUAL_BASE + 2 * NOVOS_PAGE_SIZE,
            executable_physical, false, true) ||
        !address_space_is_user_executable(
            &space, NOVOS_USER_VIRTUAL_BASE + 2 * NOVOS_PAGE_SIZE) ||
        address_space_is_user_executable(
            &space, NOVOS_USER_VIRTUAL_BASE))
        for (;;) asm volatile("cli; hlt");

    auto* pml4 = table(space.pml4_physical);
    const unsigned int user_pml4_index =
        (NOVOS_USER_VIRTUAL_BASE >> 39) & 0x1FF;
    if (!pml4)
        for (;;) asm volatile("cli; hlt");
    const u64 saved_pml4_entry = pml4[user_pml4_index];
    pml4[user_pml4_index] = saved_pml4_entry | NOVOS_PAGE_NO_EXECUTE;
    if (address_space_is_user_executable(
            &space, NOVOS_USER_VIRTUAL_BASE + 2 * NOVOS_PAGE_SIZE))
        for (;;) asm volatile("cli; hlt");
    pml4[user_pml4_index] = saved_pml4_entry;

    pml4[user_pml4_index] = saved_pml4_entry & ~NOVOS_PAGE_WRITE;
    const u64 blocked_write_physical = pmm_alloc_page();
    if (!blocked_write_physical ||
        address_space_map(
            &space, NOVOS_USER_VIRTUAL_BASE + 4 * NOVOS_PAGE_SIZE,
            blocked_write_physical, true, false))
        for (;;) asm volatile("cli; hlt");
    pmm_free_page(blocked_write_physical);
    pml4[user_pml4_index] = saved_pml4_entry;

    pml4[user_pml4_index] = saved_pml4_entry | NOVOS_PAGE_NO_EXECUTE;
    const u64 blocked_exec_physical = pmm_alloc_page();
    if (!blocked_exec_physical ||
        address_space_map(
            &space, NOVOS_USER_VIRTUAL_BASE + 3 * NOVOS_PAGE_SIZE,
            blocked_exec_physical, false, true))
        for (;;) asm volatile("cli; hlt");
    pmm_free_page(blocked_exec_physical);
    pml4[user_pml4_index] = saved_pml4_entry;

    if (!address_space_destroy(&space) || space.pml4_physical != 0)
        for (;;) asm volatile("cli; hlt");
}
