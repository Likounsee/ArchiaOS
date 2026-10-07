#include "paging.hpp"
#include "../cpu/features.hpp"

static constexpr u64 NOVOS_IDENTITY_MAP_SIZE = NOVOS_PMM_MAX_PHYSICAL_ADDRESS;
static constexpr u64 NOVOS_2M_PAGE_SIZE = 0x200000ULL;

static u64* pml4 = nullptr;
static u64 pml4_physical = 0;
static u64* pdpt = nullptr;
static bool paging_active = false;
static u64 mapped_physical_limit = NOVOS_PMM_MAX_PHYSICAL_ADDRESS;

static inline void invalidate_page_aliases(u64 virtualAddress)
{
    asm volatile("invlpg (%0)" : : "r"(virtualAddress) : "memory");

    if (virtualAddress < NOVOS_IDENTITY_MAP_SIZE)
    {
        const u64 alias = NOVOS_HHDM_BASE + virtualAddress;
        asm volatile("invlpg (%0)" : : "r"(alias) : "memory");
    }
    else if (virtualAddress >= NOVOS_HHDM_BASE &&
             virtualAddress < NOVOS_HHDM_BASE + mapped_physical_limit)
    {
        const u64 alias = virtualAddress - NOVOS_HHDM_BASE;
        asm volatile("invlpg (%0)" : : "r"(alias) : "memory");
    }
}

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

static inline u64 page_entry_flags(PagingFlags flags)
{
    u64 value = NOVOS_PAGE_PRESENT;

    if (flags.write_through)
        value |= 1ULL << 3;
    if (flags.cache_disable)
        value |= 1ULL << 4;

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

static bool split_1g_pdpte(u64* pdpte)
{
    if ((*pdpte & NOVOS_PAGE_PRESENT) == 0)
        return false;

    if ((*pdpte & NOVOS_PAGE_HUGE) == 0)
        return true;

    const u64 oldEntry = *pdpte;
    const u64 oldBase = oldEntry & 0x000FFFFFC0000000ULL;
    const u64 permissionFlags =
        oldEntry & (NOVOS_PAGE_PRESENT |
                    NOVOS_PAGE_WRITE |
                    NOVOS_PAGE_USER |
                    (1ULL << 3) |
                    (1ULL << 4) |
                    NOVOS_PAGE_NO_EXECUTE);

    const u64 pdPhysical = pmm_alloc_page();
    if (pdPhysical == 0)
        return false;

    auto* pdTable = table_pointer(pdPhysical);
    zero_page(pdPhysical);

    for (u64 i = 0; i < 512; ++i)
    {
        pdTable[i] =
            (oldBase + i * NOVOS_2M_PAGE_SIZE) |
            permissionFlags |
            NOVOS_PAGE_HUGE;
    }

    *pdpte = pdPhysical | permissionFlags;
    return true;
}

static bool split_2m_pde(u64* pde)
{
    if ((*pde & NOVOS_PAGE_PRESENT) == 0)
        return false;

    if ((*pde & NOVOS_PAGE_HUGE) == 0)
        return true;

    const u64 oldEntry = *pde;
    const u64 oldBase = oldEntry & 0x000FFFFFFFE00000ULL;
    const u64 permissionFlags =
        oldEntry & (NOVOS_PAGE_PRESENT |
                    NOVOS_PAGE_WRITE |
                    NOVOS_PAGE_USER |
                    (1ULL << 3) |
                    (1ULL << 4) |
                    NOVOS_PAGE_NO_EXECUTE);
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
                permissionFlags;
    }

    *pde = ptPhysical | permissionFlags;
    return true;
}

static u64 allocate_table_page()
{
    const u64 physical = pmm_alloc_page_above(0x01000000ULL);
    if (!physical) return 0;
    zero_page(physical);
    return physical;
}

static u64* find_4k_entry(u64 virtualAddress, bool user, bool split)
{
    if (pml4 == nullptr ||
        ((virtualAddress >> 48) != 0 && (virtualAddress >> 48) != 0xFFFFULL))
        return nullptr;

    u64& pml4e = pml4[(virtualAddress >> 39) & 0x1FF];
    if ((pml4e & NOVOS_PAGE_PRESENT) == 0)
    {
        if (!split) return nullptr;
        const u64 table = allocate_table_page();
        if (!table) return nullptr;
        pml4e = table | NOVOS_PAGE_PRESENT | NOVOS_PAGE_WRITE |
                (user ? NOVOS_PAGE_USER : 0);
    }
    else if (user)
        pml4e |= NOVOS_PAGE_USER;

    auto* table3 = table_pointer(pml4e & ~0xFFFULL);
    u64& pdpte = table3[(virtualAddress >> 30) & 0x1FF];
    if ((pdpte & NOVOS_PAGE_PRESENT) == 0)
    {
        if (!split) return nullptr;
        const u64 table = allocate_table_page();
        if (!table) return nullptr;
        pdpte = table | NOVOS_PAGE_PRESENT | NOVOS_PAGE_WRITE |
                (user ? NOVOS_PAGE_USER : 0);
    }
    else if (user)
        pdpte |= NOVOS_PAGE_USER;

    if (split && (pdpte & NOVOS_PAGE_HUGE) != 0)
        if (!split_1g_pdpte(&pdpte)) return nullptr;

    auto* table2 = table_pointer(pdpte & ~0xFFFULL);
    u64& pde = table2[(virtualAddress >> 21) & 0x1FF];
    if ((pde & NOVOS_PAGE_PRESENT) == 0)
    {
        if (!split) return nullptr;
        const u64 table = allocate_table_page();
        if (!table) return nullptr;
        pde = table | NOVOS_PAGE_PRESENT | NOVOS_PAGE_WRITE |
              (user ? NOVOS_PAGE_USER : 0);
    }
    else if (user)
        pde |= NOVOS_PAGE_USER;

    if (split && !split_2m_pde(&pde))
        return nullptr;

    auto* pt = table_pointer(pde & ~0xFFFULL);
    return &pt[(virtualAddress >> 12) & 0x1FF];
}

static inline u64 read_cr3()
{
    u64 value = 0;
    asm volatile("mov %%cr3, %0" : "=r"(value));
    return value & ~0xFFFULL;
}

static bool setup_identity_2m()
{
    if (pml4 != nullptr)
        return true;

    /*
     * UEFI has already built and activated the transition page tables:
     * identity + HHDM + higher-half kernel mapping. Adopting that CR3 keeps
     * the kernel executable while ownership moves from the bootloader to
     * the kernel paging code.
     */
    const u64 bootstrapPml4Physical = read_cr3();
    if (bootstrapPml4Physical == 0)
        return false;

    auto* bootstrapPml4 =
        reinterpret_cast<u64*>(bootstrapPml4Physical);

    const u64 identityPdptEntry = bootstrapPml4[0];
    const u64 hhdmPdptEntry = bootstrapPml4[256];
    const u64 kernelPdptEntry = bootstrapPml4[511];

    if ((identityPdptEntry & NOVOS_PAGE_PRESENT) == 0 ||
        (hhdmPdptEntry & NOVOS_PAGE_PRESENT) == 0 ||
        (kernelPdptEntry & NOVOS_PAGE_PRESENT) == 0)
        return false;

    /*
     * The bootstrap HHDM and identity map intentionally share the same
     * lower-level hierarchy. This is the direct-map invariant used by the
     * PMM/page-table code before the hierarchy is later expanded.
     */
    if ((identityPdptEntry & ~0xFFFULL) !=
        (hhdmPdptEntry & ~0xFFFULL))
        return false;

    const CpuInfo* cpu = cpu_get_info();
    if (cpu && cpu->physical_address_bits >= 32 &&
        cpu->physical_address_bits < 63)
    {
        const u64 maxByCpu = 1ULL << cpu->physical_address_bits;
        if (maxByCpu < mapped_physical_limit)
            mapped_physical_limit = maxByCpu;
    }

    if (mapped_physical_limit > NOVOS_PMM_MAX_PHYSICAL_ADDRESS)
        mapped_physical_limit = NOVOS_PMM_MAX_PHYSICAL_ADDRESS;

    if (mapped_physical_limit < NOVOS_2M_PAGE_SIZE)
        return false;

    const u64 identityPdptPhysical =
        identityPdptEntry & ~0xFFFULL;

    pml4_physical = bootstrapPml4Physical;
    pml4 = bootstrapPml4;
    pdpt = reinterpret_cast<u64*>(identityPdptPhysical);

    /*
     * The boot hierarchy uses 2 MiB leaves on CPUs without 1 GiB pages and
     * may use 1 GiB leaves when available. Leave those leaves intact; the
     * existing split_1g_pdpte/split_2m_pde paths will convert them when a
     * fine-grained mapping is requested.
     */
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

extern "C" u64 paging_max_physical_address()
{
    return mapped_physical_limit;
}

extern "C" u64 paging_translate(u64 virtualAddress)
{
    if (pml4 == nullptr ||
        ((virtualAddress >> 48) != 0 &&
         (virtualAddress >> 48) != 0xFFFFULL))
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

    if (pdpte & NOVOS_PAGE_HUGE)
        return (pdpte & 0x000FFFFFC0000000ULL) |
               (virtualAddress & 0x3FFFFFFFULL);

    auto* table2 =
        table_pointer(pdpte & ~0xFFFULL);

    const u64 pde =
        table2[(virtualAddress >> 21) & 0x1FF];

    if ((pde & NOVOS_PAGE_PRESENT) == 0)
        return 0;

    if (pde & NOVOS_PAGE_HUGE)
        return (pde & 0x000FFFFFFFE00000ULL) |
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
        physicalAddress >= mapped_physical_limit ||
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
        ((virtualAddress >> 48) != 0 && (virtualAddress >> 48) != 0xFFFFULL) ||
        (physicalAddress & (NOVOS_PAGE_SIZE - 1ULL)) != 0 ||
        physicalAddress >= mapped_physical_limit)
        return false;

    u64* entry = find_4k_entry(virtualAddress, flags.user, true);
    if (entry == nullptr)
        return false;

    const u64 flagsValue = page_entry_flags(flags);
    if (flagsValue == 0)
        return false;

    *entry = physicalAddress | flagsValue;

    invalidate_page_aliases(virtualAddress);
    return true;
}

extern "C" bool paging_unmap_4k(u64 virtualAddress)
{
    if ((virtualAddress & (NOVOS_PAGE_SIZE - 1ULL)) != 0 ||
        ((virtualAddress >> 48) != 0 && (virtualAddress >> 48) != 0xFFFFULL))
        return false;

    u64* entry = find_4k_entry(virtualAddress, false, false);
    if (entry == nullptr || (*entry & NOVOS_PAGE_PRESENT) == 0)
        return false;

    *entry = 0;
    invalidate_page_aliases(virtualAddress);
    return true;
}

extern "C" u64 paging_get_4k_entry(u64 virtualAddress)
{
    u64* entry = find_4k_entry(virtualAddress, false, false);
    return entry ? *entry : 0;
}

extern "C" u64 paging_physical_to_virtual(u64 physicalAddress)
{
    if (physicalAddress >= mapped_physical_limit)
        return 0;

    const u64 hhdmEnd = NOVOS_HHDM_BASE + mapped_physical_limit;
    if (hhdmEnd < NOVOS_HHDM_BASE)
        return 0;

    return NOVOS_HHDM_BASE + physicalAddress;
}

extern "C" u64 paging_virtual_to_physical(u64 virtualAddress)
{
    if (virtualAddress < NOVOS_HHDM_BASE ||
        virtualAddress >=
            NOVOS_HHDM_BASE + mapped_physical_limit)
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
    u64 cr3 = 0;

    asm volatile(
        "mov %%cr0, %0\n"
        "mov %%cr3, %1"
        : "=r"(cr0), "=r"(cr3));

    return (cr0 & (1ULL << 31)) != 0 &&
           pml4_physical != 0 &&
           (cr3 & ~0xFFFULL) == pml4_physical;
}
