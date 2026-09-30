#include "paging.hpp"
#include "../cpu/features.hpp"

static constexpr u64 NOVOS_PAGE_TABLE_COUNT = 64;
static constexpr u64 NOVOS_IDENTITY_MAP_SIZE = 0x1000000000ULL; /* 64 GiB */
static constexpr u64 NOVOS_2M_PAGE_SIZE = 0x200000ULL;
static constexpr u64 NOVOS_PDPT_COVERAGE = 0x40000000ULL; /* 1 GiB */

static u64* pml4 = nullptr;
static u64 pml4_physical = 0;
static u64* pdpt = nullptr;
static u64* pd[NOVOS_PAGE_TABLE_COUNT] = {};
static bool paging_active = false;

static inline u64* table_pointer(u64 physicalAddress)
{
    /*
     * Before CR3 activation only the bootstrap identity map is guaranteed.
     * Once active, page-table pages must be reached through the HHDM so that
     * page-table management no longer depends on the identity map.
     */
    if (paging_active)
        return reinterpret_cast<u64*>(NOVOS_HHDM_BASE + physicalAddress);

    return reinterpret_cast<u64*>(physicalAddress);
}

static inline void zero_page(u64 address)
{
    auto* page = reinterpret_cast<volatile u64*>(table_pointer(address));

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
    {
        const CpuInfo* cpu = cpu_get_info();
        if (cpu == nullptr || !cpu->features.nx)
            return 0;

        value |= NOVOS_PAGE_NO_EXECUTE;
    }

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

    auto* pt = table_pointer(ptPhysical);
    zero_page(ptPhysical);

    /*
     * Preserve the existing bootstrap mapping while replacing one 2 MiB
     * leaf with a 4 KiB page table. The default entries remain supervisor,
     * writable and executable; individual pages can then be hardened.
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
        table_pointer(pml4e & ~0xFFFULL);

    const u64 pdpte =
        table3[(virtualAddress >> 30) & 0x1FF];

    if ((pdpte & NOVOS_PAGE_PRESENT) == 0)
        return nullptr;

    auto* table2 =
        table_pointer(pdpte & ~0xFFFULL);

    u64* pde =
        &table2[(virtualAddress >> 21) & 0x1FF];

    if (!split_2m_pde(pde))
        return nullptr;

    auto* pt =
        table_pointer((*pde) & ~0xFFFULL);

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

    pml4_physical = pml4Physical;
    pml4 = reinterpret_cast<u64*>(pml4Physical);
    pdpt = reinterpret_cast<u64*>(pdptPhysical);

    zero_page(pml4Physical);
    zero_page(pdptPhysical);

    /*
     * Early address space:
     *   0x0000000000000000..0x0000000FFFFFFFFF = physical identity map
     *   0xFFFF800000000000..0xFFFF80FFFFFFFFFF = HHDM/direct map
     *
     * Both virtual ranges intentionally reference the same page tables.
     * The HHDM therefore adds no second copy of the 64 GiB mapping and gives
     * the kernel a stable canonical virtual address for every managed frame.
     */
    pml4[0] = table_entry(pdptPhysical);
    pml4[256] = table_entry(pdptPhysical);

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
    return pml4_physical;
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
        table_pointer(pml4e & ~0xFFFULL);

    const u64 pdpte =
        table3[(virtualAddress >> 30) & 0x1FF];

    if ((pdpte & NOVOS_PAGE_PRESENT) == 0)
        return 0;

    auto* table2 =
        table_pointer(pdpte & ~0xFFFULL);

    const u64 pde =
        table2[(virtualAddress >> 21) & 0x1FF];

    if ((pde & NOVOS_PAGE_PRESENT) == 0)
        return 0;

    if (pde & NOVOS_PAGE_HUGE)
        return (pde & ~0x1FFFFFULL) |
               (virtualAddress & 0x1FFFFFULL);

    auto* table1 =
        table_pointer(pde & ~0xFFFULL);

    const u64 pte =
        table1[(virtualAddress >> 12) & 0x1FF];

    if ((pte & NOVOS_PAGE_PRESENT) == 0)
        return 0;

    return (pte & 0x000FFFFFFFFFF000ULL) |
           (virtualAddress & 0xFFFULL);
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

    const u64 flagsValue = page_entry_flags(flags);
    if (flagsValue == 0)
        return false;

    *entry = physicalAddress | flagsValue;

    asm volatile("invlpg (%0)" : : "r"(virtualAddress) : "memory");
    return true;
}

extern "C" bool paging_unmap_4k(u64 virtualAddress)
{
    if ((virtualAddress & (NOVOS_PAGE_SIZE - 1ULL)) != 0)
        return false;

    u64* entry = find_4k_entry(virtualAddress);
    if (entry == nullptr)
        return false;

    *entry = 0;
    asm volatile("invlpg (%0)" : : "r"(virtualAddress) : "memory");
    return true;
}

extern "C" u64 paging_get_4k_entry(u64 virtualAddress)
{
    u64* entry = find_4k_entry(virtualAddress);
    return entry ? *entry : 0;
}

extern "C" u64 paging_physical_to_virtual(u64 physicalAddress)
{
    if (physicalAddress >= NOVOS_PMM_MAX_PHYSICAL_ADDRESS)
        return 0;

    const u64 hhdmEnd = NOVOS_HHDM_BASE + NOVOS_PMM_MAX_PHYSICAL_ADDRESS;
    if (hhdmEnd < NOVOS_HHDM_BASE)
        return 0;

    return NOVOS_HHDM_BASE + physicalAddress;
}

extern "C" u64 paging_virtual_to_physical(u64 virtualAddress)
{
    if (virtualAddress < NOVOS_HHDM_BASE ||
        virtualAddress >=
            NOVOS_HHDM_BASE + NOVOS_PMM_MAX_PHYSICAL_ADDRESS)
    {
        return 0;
    }

    const u64 translated = paging_translate(virtualAddress);
    return translated;
}

extern "C" bool paging_activate()
{
    if (pml4 == nullptr)
        return false;

    const u64 pml4Physical = pml4_physical;

    asm volatile(
        "mov %0, %%cr3"
        :
        : "r"(pml4Physical)
        : "memory");

    /* From this point on, page-table pages are accessed through the HHDM. */
    paging_active = true;
    pml4 = table_pointer(pml4_physical);

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
