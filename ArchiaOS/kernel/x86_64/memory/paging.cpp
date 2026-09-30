#include "paging.hpp"

static constexpr u64 NOVOS_PAGE_TABLE_COUNT = 64;
static constexpr u64 NOVOS_IDENTITY_MAP_SIZE = 0x10000000000ULL; /* 64 GiB */
static constexpr u64 NOVOS_2M_PAGE_SIZE = 0x200000ULL;
static constexpr u64 NOVOS_PDPT_COVERAGE = 0x40000000ULL; /* 1 GiB */

static u64* pml4 = nullptr;
static u64* pdpt = nullptr;
static u64* pd[NOVOS_PAGE_TABLE_COUNT] = {};

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


static inline u64 page_entry_flags(PagingFlags flags)
{
    u64 value = NOVOS_PAGE_PRESENT;

    if (flags.writable)
        value |= NOVOS_PAGE_WRITE;

    if (flags.user)
        value |= NOVOS_PAGE_USER;

    if (!flags.executable)
        value |= NOVOS_PAGE_NO_EXECUTE;

    return value;
}

static bool split_2m_pde(u64* pde)
{
    if ((*pde & NOVOS_PAGE_PRESENT) == 0)
        return false;

    if ((*pde & NOVOS_PAGE_HUGE) == 0)
        return true;

    const u64 oldBase = *pde & ~0x1FFFFFULL;
    const u64 ptPhysical = pmm_alloc_page();

    if (ptPhysical == 0)
        return false;

    auto* pt = reinterpret_cast<u64*>(ptPhysical);
    zero_page(ptPhysical);

    /*
     * Preserve the existing bootstrap identity mapping while replacing
     * one 2 MiB leaf with a 4 KiB page table. The default entries remain
     * supervisor RW executable; individual pages can then be hardened.
     */
    for (u64 i = 0; i < 512; ++i)
    {
        pt[i] = (oldBase + i * NOVOS_PAGE_SIZE) |
                NOVOS_PAGE_PRESENT |
                NOVOS_PAGE_WRITE;
    }

    *pde = ptPhysical | NOVOS_PAGE_PRESENT | NOVOS_PAGE_WRITE;
    return true;
}

static u64* find_4k_entry(u64 virtualAddress)
{
    if (pml4 == nullptr)
        return nullptr;

    const u64 pml4e =
        pml4[(virtualAddress >> 39) & 0x1FF];

    if ((pml4e & NOVOS_PAGE_PRESENT) == 0)
        return nullptr;

    auto* table3 =
        reinterpret_cast<u64*>(pml4e & ~0xFFFULL);

    const u64 pdpte =
        table3[(virtualAddress >> 30) & 0x1FF];

    if ((pdpte & NOVOS_PAGE_PRESENT) == 0)
        return nullptr;

    auto* table2 =
        reinterpret_cast<u64*>(pdpte & ~0xFFFULL);

    u64* pde =
        &table2[(virtualAddress >> 21) & 0x1FF];

    if (!split_2m_pde(pde))
        return nullptr;

    auto* pt =
        reinterpret_cast<u64*>((*pde) & ~0xFFFULL);

    return &pt[(virtualAddress >> 12) & 0x1FF];
}

static bool setup_identity_2m()
{
    if (pml4 != nullptr)
        return true;

    const u64 pml4Physical = pmm_alloc_page();
    const u64 pdptPhysical = pmm_alloc_page();

    if (pml4Physical == 0 || pdptPhysical == 0)
        return false;

    pml4 = reinterpret_cast<u64*>(pml4Physical);
    pdpt = reinterpret_cast<u64*>(pdptPhysical);

    zero_page(pml4Physical);
    zero_page(pdptPhysical);

    /*
     * Keep the early address space deliberately simple: virtual == physical
     * for the entire physical range managed by the PMM (64 GiB).
     *
     * This covers the kernel image, its stack, BootInfo, ACPI tables,
     * framebuffer, LAPIC/IOAPIC MMIO and future early allocations without
     * depending on any firmware page tables after CR3 is replaced.
     */
    pml4[0] = table_entry(pdptPhysical);

    for (u64 pdptIndex = 0; pdptIndex < NOVOS_PAGE_TABLE_COUNT; ++pdptIndex)
    {
        const u64 pdPhysical = pmm_alloc_page();

        if (pdPhysical == 0)
            return false;

        pd[pdptIndex] = reinterpret_cast<u64*>(pdPhysical);
        zero_page(pdPhysical);
        pdpt[pdptIndex] = table_entry(pdPhysical);

        for (u64 i = 0; i < 512; ++i)
        {
            const u64 physicalAddress =
                pdptIndex * NOVOS_PDPT_COVERAGE +
                i * NOVOS_2M_PAGE_SIZE;

            pd[pdptIndex][i] =
                physicalAddress |
                NOVOS_PAGE_PRESENT |
                NOVOS_PAGE_WRITE |
                NOVOS_PAGE_HUGE;
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

    const u64 pml4e =
        pml4[(virtualAddress >> 39) & 0x1FF];

    if ((pml4e & NOVOS_PAGE_PRESENT) == 0)
        return 0;

    auto* table3 =
        reinterpret_cast<u64*>(pml4e & ~0xFFFULL);

    const u64 pdpte =
        table3[(virtualAddress >> 30) & 0x1FF];

    if ((pdpte & NOVOS_PAGE_PRESENT) == 0)
        return 0;

    auto* table2 =
        reinterpret_cast<u64*>(pdpte & ~0xFFFULL);

    const u64 pde =
        table2[(virtualAddress >> 21) & 0x1FF];

    if ((pde & NOVOS_PAGE_PRESENT) == 0)
        return 0;

    if (pde & NOVOS_PAGE_HUGE)
        return (pde & ~0x1FFFFFULL) |
               (virtualAddress & 0x1FFFFFULL);

    return 0;
}

extern "C" bool paging_map_identity(u64 physicalAddress)
{
    if ((physicalAddress & (NOVOS_2M_PAGE_SIZE - 1ULL)) != 0 ||
        physicalAddress >= NOVOS_IDENTITY_MAP_SIZE ||
        pml4 == nullptr)
        return false;

    return paging_translate(physicalAddress) == physicalAddress;
}

extern "C" bool paging_map_4k(
    u64 virtualAddress,
    u64 physicalAddress,
    PagingFlags flags)
{
    if ((virtualAddress & (NOVOS_PAGE_SIZE - 1ULL)) != 0 ||
        (physicalAddress & (NOVOS_PAGE_SIZE - 1ULL)) != 0 ||
        physicalAddress >= NOVOS_PMM_MAX_PHYSICAL_ADDRESS)
        return false;

    u64* entry = find_4k_entry(virtualAddress);
    if (entry == nullptr)
        return false;

    *entry = physicalAddress | page_entry_flags(flags);

    asm volatile("invlpg (%0)" : : "r"(virtualAddress) : "memory");
    return true;
}

extern "C" u64 paging_get_4k_entry(u64 virtualAddress)
{
    u64* entry = find_4k_entry(virtualAddress);
    return entry ? *entry : 0;
}

extern "C" bool paging_activate()
{
    if (pml4 == nullptr)
        return false;

    const u64 pml4Physical =
        reinterpret_cast<u64>(pml4);

    asm volatile(
        "mov %0, %%cr3"
        :
        : "r"(pml4Physical)
        : "memory");

    asm volatile("mov %%cr3, %%rax" ::: "rax", "memory");

    return paging_is_enabled();
}

extern "C" bool paging_is_enabled()
{
    u64 cr0 = 0;

    asm volatile(
        "mov %%cr0, %0"
        : "=r"(cr0));

    return (cr0 & (1ULL << 31)) != 0;
}
