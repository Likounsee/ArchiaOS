#include "address_space.hpp"
#include "paging.hpp"
#include "pmm.hpp"

extern "C" bool address_space_create(AddressSpace* space)
{
    if (!space) return false;
    const u64 pml4 = pmm_alloc_page();
    if (!pml4) return false;
    auto* table = reinterpret_cast<u64*>(paging_physical_to_virtual(pml4));
    if (!table) { pmm_free_page(pml4); return false; }
    for (unsigned int i = 0; i < 512; ++i) table[i] = 0;

    const u64 kernel = paging_pml4_physical();
    auto* current = reinterpret_cast<u64*>(paging_physical_to_virtual(kernel));
    if (!current) { pmm_free_page(pml4); return false; }
    for (unsigned int i = 256; i < 512; ++i)
        table[i] = current[i];

    space->pml4_physical = pml4;
    space->active = false;
    return true;
}

extern "C" bool address_space_map(
    AddressSpace* space, u64 virtual_address, u64 physical_address,
    bool writable, bool executable)
{
    if (!space || (virtual_address & 0xFFFULL) || (physical_address & 0xFFFULL) ||
        virtual_address < NOVOS_USER_VIRTUAL_BASE ||
        virtual_address >= NOVOS_USER_VIRTUAL_TOP)
        return false;

    /* Temporarily switch CR3 and use the paging walker against this address space. */
    const u64 old = paging_pml4_physical();
    (void)old;
    /* The current paging API intentionally owns the active hierarchy; mapping
       arbitrary CR3s will be added in the next address-space hardening pass. */
    return false;
}

extern "C" bool address_space_activate(AddressSpace* space)
{
    if (!space || !space->pml4_physical) return false;
    asm volatile("mov %0, %%cr3" : : "r"(space->pml4_physical) : "memory");
    space->active = true;
    return true;
}

extern "C" bool address_space_is_user_mapped(const AddressSpace*, u64)
{
    return false;
}

extern "C" void address_space_run_tests()
{
    AddressSpace space{};
    if (!address_space_create(&space))
        for (;;) asm volatile("cli; hlt");
}
