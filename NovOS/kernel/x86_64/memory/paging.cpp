#include "paging.hpp"

static u64* pml4 = nullptr;
static u64* pdpt = nullptr;
static u64* pd[4] = { nullptr, nullptr, nullptr, nullptr };

static inline void zero_page(u64 address)
{
    auto* page = reinterpret_cast<volatile u64*>(address);
    for (u64 i = 0; i < 512; ++i)
        page[i] = 0;
}

static inline u64 table_entry(u64 address)
{
    return address | NOVOS_PAGE_PRESENT | NOVOS_PAGE_WRITE;
}

static bool setup_identity_2m()
{
    u64 pml4Physical = pmm_alloc_page();
    u64 pdptPhysical = pmm_alloc_page();

    if (pml4Physical == 0 || pdptPhysical == 0)
        return false;

    pml4 = reinterpret_cast<u64*>(pml4Physical);
    pdpt = reinterpret_cast<u64*>(pdptPhysical);

    zero_page(pml4Physical);
    zero_page(pdptPhysical);

    pml4[0] = table_entry(pdptPhysical);

    /*
     * Identity-map the first 4 GiB using 2 MiB pages.
     *
     * This covers the low physical-memory region used by firmware,
     * ACPI tables and MMIO during early kernel initialization.
     * Each PDPT entry covers 1 GiB and points to one page directory.
     */
    for (u64 pdptIndex = 0; pdptIndex < 4; ++pdptIndex)
    {
        u64 pdPhysical = pmm_alloc_page();

        if (pdPhysical == 0)
            return false;

        pd[pdptIndex] = reinterpret_cast<u64*>(pdPhysical);
        zero_page(pdPhysical);

        pdpt[pdptIndex] = table_entry(pdPhysical);

        for (u64 i = 0; i < 512; ++i)
        {
            u64 physicalAddress =
                (pdptIndex * 0x40000000ULL) + (i * 0x200000ULL);

            pd[pdptIndex][i] =
                physicalAddress |
                NOVOS_PAGE_PRESENT |
                NOVOS_PAGE_WRITE |
                (1ULL << 7);
        }
    }

    return true;
}

extern "C" bool paging_initialize()
{
    return setup_identity_2m();
}

extern "C" u64 paging_pml4_physical()
{
    return reinterpret_cast<u64>(pml4);
}

extern "C" u64 paging_translate(u64 virtualAddress)
{
    if (pml4 == nullptr)
        return 0;

    u64 pml4e = pml4[(virtualAddress >> 39) & 0x1FF];
    if ((pml4e & NOVOS_PAGE_PRESENT) == 0)
        return 0;

    auto* table3 = reinterpret_cast<u64*>(pml4e & ~0xFFFULL);
    u64 pdpte = table3[(virtualAddress >> 30) & 0x1FF];
    if ((pdpte & NOVOS_PAGE_PRESENT) == 0)
        return 0;

    if (pdpte & (1ULL << 7))
        return (pdpte & ~0x3FFFFFFFULL) |
               (virtualAddress & 0x3FFFFFFFULL);

    auto* table2 = reinterpret_cast<u64*>(pdpte & ~0xFFFULL);
    u64 pde = table2[(virtualAddress >> 21) & 0x1FF];
    if ((pde & NOVOS_PAGE_PRESENT) == 0)
        return 0;

    if (pde & (1ULL << 7))
        return (pde & ~0x1FFFFFULL) |
               (virtualAddress & 0x1FFFFFULL);

    return 0;
}

extern "C" bool paging_is_enabled()
{
    u64 cr0 = 0;
    asm volatile ("mov %%cr0, %0" : "=r"(cr0));
    return (cr0 & (1ULL << 31)) != 0;
}
