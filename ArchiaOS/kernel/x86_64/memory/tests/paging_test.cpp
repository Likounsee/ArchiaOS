#include "paging.hpp"
#include "idt.hpp"
#include "features.hpp"

static inline void test_char(char c)
{
    asm volatile (
        "outb %0, %1"
        :
        : "a"(c),
          "Nd"(static_cast<unsigned short>(0xE9)));
}

static void test_str(const char* s)
{
    for (int i = 0; s[i] != '\0'; ++i)
        test_char(s[i]);
}

static void fail(const char* message)
{
    test_str(message);

    for (;;)
        asm volatile("cli; hlt");
}

extern "C" void paging_run_tests()
{
    test_str("PAGING TEST START\n");

    test_str("PAGING TABLES CREATED\n");

    if (paging_pml4_physical() == 0)
        fail("PAGING TEST FAIL: PML4\n");

    const u64 testAddresses[] = {
        0x00000000ULL,
        0x00100000ULL,
        0x3FFFFFFFULL,
        0x40000000ULL,
        0x100000000ULL,
        0x7FFFFFFFFULL,
        0x7FFFFFFFFFULL
    };

    for (u64 address : testAddresses)
    {
        const u64 translated = paging_translate(address);

        if (translated != address)
            fail("PAGING TEST FAIL: TRANSLATION\n");
    }

    test_str("PAGING IDENTITY MAP PASS\n");

    /*
     * The high-half direct map must resolve the same physical frames as
     * the identity map. This is the address-space primitive used later by
     * the PMM/page-table code to access arbitrary physical memory without
     * consuming low virtual addresses.
     */
    const u64 hhdmSamples[] = {
        0x0000000000000000ULL,
        0x0000000000200000ULL,
        0x0000000040000000ULL,
        0x0000000100000000ULL,
        paging_max_physical_address() - PAGE_SIZE
    };

    for (u64 physical : hhdmSamples)
    {
        const u64 virtualAddress =
            paging_physical_to_virtual(physical);

        if (virtualAddress == 0 ||
            paging_translate(virtualAddress) != physical ||
            paging_virtual_to_physical(virtualAddress) != physical)
        {
            fail("PAGING TEST FAIL: HHDM TRANSLATION\n");
        }
    }

    if (paging_physical_to_virtual(paging_max_physical_address()) != 0 ||
        paging_virtual_to_physical(HHDM_BASE +
                                   paging_max_physical_address()) != 0)
    {
        fail("PAGING TEST FAIL: HHDM RANGE\n");
    }

    test_str("PAGING HHDM PASS\n");

    if (!paging_activate())
        fail("PAGING TEST FAIL: ACTIVATION\n");

    if (!paging_is_enabled())
        fail("PAGING TEST FAIL: PAGING DISABLED\n");

    if (paging_translate(0x0000000012345678ULL) !=
        0x0000000012345678ULL)
        fail("PAGING TEST FAIL: POST-ACTIVATE TRANSLATION\n");

    /*
     * Validate a real load/store through the HHDM after CR3 activation.
     * Translation-only tests cannot prove that the CPU can actually access
     * the direct-map virtual address.
     */
    const u64 hhdmTestPage = pmm_alloc_page();
    if (hhdmTestPage == 0)
        fail("PAGING TEST FAIL: HHDM ALLOCATION\n");

    const u64 hhdmTestVirtual = paging_physical_to_virtual(hhdmTestPage);
    volatile u64* hhdmMemory =
        reinterpret_cast<volatile u64*>(hhdmTestVirtual);
    constexpr u64 hhdmPattern = 0xA55A5AA55AA55AA5ULL;

    hhdmMemory[0] = hhdmPattern;

    if (hhdmMemory[0] != hhdmPattern)
        fail("PAGING TEST FAIL: HHDM MEMORY ACCESS\n");

    pmm_free_page(hhdmTestPage);
    test_str("PAGING HHDM MEMORY ACCESS PASS\n");

    /*
     * On CPUs with 1 GiB-page support, bootstrap mappings start as 1 GiB
     * PDPTE leaves. Mapping a single 4 KiB page inside one of those leaves
     * must split it safely before installing the fine-grained PTE.
     */
    const CpuInfo* cpu = cpu_get_info();
    if (cpu != nullptr && cpu->features.one_gib_pages)
    {
        const u64 splitVirtual = 0x0000000041234000ULL;
        if (paging_get_4k_entry(splitVirtual) != 0)
            fail("PAGING TEST FAIL: 1G LEAF AS 4K\n");
        const u64 splitPhysical = pmm_alloc_page();

        if (splitPhysical == 0)
            fail("PAGING TEST FAIL: 1G SPLIT ALLOCATION\n");

        if (!paging_map_4k(
                splitVirtual,
                splitPhysical,
                PagingFlags{false, false, false, false, false}) ||
            paging_translate(splitVirtual) != splitPhysical)
        {
            fail("PAGING TEST FAIL: 1G SPLIT\n");
        }

        if (!paging_unmap_4k(splitVirtual))
            fail("PAGING TEST FAIL: 1G SPLIT UNMAP\n");

        pmm_free_page(splitPhysical);
        test_str("PAGING 1G SPLIT PASS\n");
    }
    else
    {
        test_str("PAGING 1G SPLIT SKIPPED\n");
    }

    if (!paging_map_identity(0x0000000000200000ULL))
        fail("PAGING TEST FAIL: MAP API\n");

    const u64 kernelUserProbe = pmm_alloc_page();
    if (kernelUserProbe == 0 ||
        paging_map_4k(
            KERNEL_HEAP_BASE,
            kernelUserProbe,
            PagingFlags{false, true, false, false, false}))
    {
        fail("PAGING TEST FAIL: KERNEL USER RANGE\n");
    }
    pmm_free_page(kernelUserProbe);

    /*
     * An existing supervisor mapping must never be promoted to USER merely
     * because a caller repeats paging_map_4k with user=true.
     */
    const u64 identityPhysical = paging_translate(0x0000000000201000ULL);
    if (identityPhysical == 0 ||
        paging_map_4k(
            0x0000000000201000ULL,
            identityPhysical,
            PagingFlags{false, true, false, false, false}))
    {
        fail("PAGING TEST FAIL: EXISTING MAP PROMOTION\n");
    }

    const u64 identityPml4Virtual = paging_physical_to_virtual(
        paging_pml4_physical());
    auto* identityPml4Table =
        reinterpret_cast<volatile u64*>(identityPml4Virtual);
    if ((identityPml4Table[0] & PAGE_USER) != 0)
        fail("PAGING TEST FAIL: IDENTITY USER PROMOTION\n");

    if (paging_translate(0x0000800000000000ULL) != 0 ||
        paging_translate(0xFFFF000000000000ULL) != 0)
        fail("PAGING TEST FAIL: NONCANONICAL TRANSLATE\n");
    if (paging_get_4k_entry(0x0000800000000000ULL) != 0 ||
        paging_get_4k_entry(0xFFFF000000000000ULL) != 0)
        fail("PAGING TEST FAIL: NONCANONICAL ENTRY\n");
    test_str("PAGING CANONICAL ADDRESS PASS\n");

    /*
     * Exercise fine-grained permissions with a PMM-owned page.
     */
    const u64 testPage = pmm_alloc_page();
    if (testPage == 0)
        fail("PAGING TEST FAIL: 4K ALLOCATION\n");

    const PagingFlags readOnlyNoExecute{
        false,
        false,
        false,
        false,
        false
    };

    /*
     * Install the instruction while the bootstrap identity mapping is still
     * writable. The protection test below must not fault while preparing its
     * own test page.
     */
    *reinterpret_cast<volatile unsigned char*>(testPage) = 0xC3;

    if (!paging_map_4k(testPage, testPage, readOnlyNoExecute))
        fail("PAGING TEST FAIL: 4K MAP\n");

    const u64 conflictingPage = pmm_alloc_page();
    if (conflictingPage == 0 ||
        paging_map_4k(
            testPage,
            conflictingPage,
            readOnlyNoExecute) ||
        paging_translate(testPage) != testPage)
    {
        fail("PAGING TEST FAIL: CONFLICTING REMAP\n");
    }
    pmm_free_page(conflictingPage);

    const u64 entry = paging_get_4k_entry(testPage);

    if ((entry & 0x000FFFFFFFFFF000ULL) != testPage ||
        (entry & PAGE_PRESENT) == 0 ||
        (entry & PAGE_WRITE) != 0 ||
        (entry & PAGE_USER) != 0 ||
        (entry & PAGE_NO_EXECUTE) == 0)
    {
        fail("PAGING TEST FAIL: PERMISSIONS\n");
    }

    if (paging_translate(testPage) != testPage)
        fail("PAGING TEST FAIL: 4K TRANSLATION\n");

    /*
     * A supervisor-only identity mapping must not be promoted to USER just
     * because a fine-grained user mapping is requested inside its huge leaf.
     */
    const u64 identityUserProbe = pmm_alloc_page();
    if (identityUserProbe == 0 ||
        paging_map_4k(
            USER_VIRTUAL_BASE,
            identityUserProbe,
            PagingFlags{false, true, false, false, false}))
    {
        fail("PAGING TEST FAIL: HUGE USER PROMOTION\n");
    }

    const u64 identityHierarchyVirtual = paging_physical_to_virtual(
        paging_pml4_physical());
    auto* identityHierarchy =
        reinterpret_cast<volatile u64*>(identityHierarchyVirtual);
    if ((identityHierarchy[0] & PAGE_USER) != 0)
        fail("PAGING TEST FAIL: HUGE USER HIERARCHY\n");
    pmm_free_page(identityUserProbe);

    const u64 userPage = pmm_alloc_page();
    if (userPage == 0)
        fail("PAGING TEST FAIL: USER PAGE ALLOCATION\n");

    const PagingFlags userReadOnly{
        false,
        true,
        false,
        false,
        false
    };

    const u64 userVirtual = USER_VIRTUAL_BASE + 0x0000080000000000ULL;
    const unsigned int userPml4Index =
        static_cast<unsigned int>((userVirtual >> 39) & 0x1FFULL);

    if (!paging_map_4k(userVirtual,
                       userPage,
                       userReadOnly))
        fail("PAGING TEST FAIL: USER MAP\n");

    /*
     * NX is legal on an upper-level entry. Address extraction must discard
     * that permission bit instead of mistaking it for part of the physical
     * page-table address. Translation remains valid even though execution
     * through this subtree is disabled.
     */
    const u64 userPml4Virtual = paging_physical_to_virtual(
        paging_pml4_physical());
    auto* userPml4Table =
        reinterpret_cast<volatile u64*>(userPml4Virtual);
    const u64 savedUserPml4e = userPml4Table[userPml4Index];
    userPml4Table[userPml4Index] = savedUserPml4e | PAGE_NO_EXECUTE;
    if (paging_translate(userVirtual) != userPage)
        fail("PAGING TEST FAIL: NX PML4 ADDRESS MASK\n");
    userPml4Table[userPml4Index] = savedUserPml4e;
    test_str("PAGING NX TABLE ADDRESS MASK PASS\n");

    /*
     * The kernel heap is supervisor-only. A user mapping request into that
     * window must be rejected even when its page-table hierarchy already
     * exists and would otherwise be promotable to USER.
     */
    const u64 kernelHeapPage = pmm_alloc_page();
    if (kernelHeapPage == 0)
        fail("PAGING TEST FAIL: KERNEL HEAP ALLOCATION\n");
    if (paging_map_4k(KERNEL_HEAP_BASE,
                      kernelHeapPage,
                      PagingFlags{true, true, false, false, false}))
        fail("PAGING TEST FAIL: KERNEL HEAP USER PROMOTION\n");
    pmm_free_page(kernelHeapPage);

    const u64 pml4Virtual = paging_physical_to_virtual(
        paging_pml4_physical());
    auto* pml4Table =
        reinterpret_cast<volatile u64*>(pml4Virtual);

    if ((pml4Table[userPml4Index] & PAGE_USER) == 0 ||
        (pml4Table[256] & PAGE_USER) != 0)
        fail("PAGING TEST FAIL: USER/HHDM ISOLATION\n");

    if (!paging_unmap_4k(userVirtual))
        fail("PAGING TEST FAIL: USER UNMAP\n");

    pmm_free_page(userPage);
    test_str("PAGING USER/HHDM ISOLATION PASS\n");

    test_str("PAGING 4K RO/NX BITS PASS\n");

    const u64 cachePage = pmm_alloc_page();
    if (cachePage == 0)
        fail("PAGING TEST FAIL: CACHE ALLOCATION\n");

    const PagingFlags uncachedFlags{
        false,
        false,
        false,
        true,
        true
    };

    if (!paging_map_4k(cachePage, cachePage, uncachedFlags))
        fail("PAGING TEST FAIL: CACHE MAP\n");

    const u64 cacheEntry = paging_get_4k_entry(cachePage);
    if ((cacheEntry & (1ULL << 3)) == 0 ||
        (cacheEntry & (1ULL << 4)) == 0)
    {
        fail("PAGING TEST FAIL: CACHE FLAGS\n");
    }

    if (!paging_unmap_4k(cachePage))
        fail("PAGING TEST FAIL: CACHE UNMAP\n");

    pmm_free_page(cachePage);
    test_str("PAGING CACHE FLAGS PASS\n");

    /*
     * Prove NX is enforced by the CPU, not merely encoded in the PTE.
     * The page is writable/executable through the bootstrap identity map
     * while we install a single RET instruction, then remapped RO+NX.
     * Executing it must raise #PF with the recovery RIP below.
     */
    const unsigned long long nxRecovery =
        reinterpret_cast<unsigned long long>(&&nx_page_fault_recovered);

    exception_expect_page_fault(nxRecovery, testPage, 0x1FULL, 0x11ULL);

    asm volatile(
        "mov %%cr3, %%rcx\n"
        "mov %%rcx, %%cr3\n"
        "jmp *%%rax"
        :
        : "a"(testPage)
        : "memory");

nx_page_fault_recovered:

    if (exception_page_fault_test_active())
        fail("PAGING TEST FAIL: NX DID NOT FIRE\n");

    test_str("PAGING NX EXECUTION FAULT PASS\n");

    /*
     * Prove supervisor write-protection is enforced by CR0.WP + the PTE,
     * not merely encoded in the page-table entry. The same mapped page is
     * intentionally written while it is read-only; the page-fault handler
     * must redirect execution to the recovery label below.
     */
    const unsigned long long roRecovery =
        reinterpret_cast<unsigned long long>(&&ro_write_fault_recovered);

    exception_expect_page_fault(roRecovery, testPage, 0x1FULL, 0x03ULL);

    *reinterpret_cast<volatile unsigned char*>(testPage) = 0x5A;

ro_write_fault_recovered:

    if (exception_page_fault_test_active())
        fail("PAGING TEST FAIL: RO WRITE DID NOT FIRE\n");

    test_str("PAGING RO WRITE FAULT PASS\n");

    if (!paging_unmap_4k(testPage))
        fail("PAGING TEST FAIL: 4K UNMAP\n");

    if (paging_unmap_4k(testPage))
        fail("PAGING TEST FAIL: REPEAT UNMAP\n");

    if (paging_get_4k_entry(testPage) != 0)
        fail("PAGING TEST FAIL: 4K UNMAP STATE\n");

    /*
     * Trigger one deliberate non-present-page fault. The exception path
     * redirects only this explicitly armed test to the recovery label.
     */
    const unsigned long long recovery =
        reinterpret_cast<unsigned long long>(&&page_fault_recovered);

    exception_expect_page_fault(recovery, testPage, 0x1FULL, 0x00ULL);

    asm volatile(
        "movq (%%rax), %%rax"
        :
        : "a"(testPage)
        : "memory");

page_fault_recovered:

    if (exception_page_fault_test_active())
        fail("PAGING TEST FAIL: PAGE FAULT DID NOT FIRE\n");

    test_str("PAGING PAGE FAULT RECOVERY PASS\n");

    pmm_free_page(testPage);

    test_str("PAGING CR3 ACTIVATION PASS\n");
    test_str("PAGING TEST PASS\n");
}
