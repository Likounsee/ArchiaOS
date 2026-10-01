#include "paging.hpp"
#include "../cpu/idt.hpp"

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

    if (!paging_initialize())
        fail("PAGING TEST FAIL: INIT\n");

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
        0xFFFFFFFFFULL
    };

    for (u64 address : testAddresses)
    {
        const u64 translated = paging_translate(address);

        if (translated != address)
            fail("PAGING TEST FAIL: TRANSLATION\n");
    }

    test_str("PAGING IDENTITY MAP PASS (64 GiB)\n");

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
        NOVOS_PMM_MAX_PHYSICAL_ADDRESS - NOVOS_PAGE_SIZE
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

    if (paging_physical_to_virtual(NOVOS_PMM_MAX_PHYSICAL_ADDRESS) != 0 ||
        paging_virtual_to_physical(NOVOS_HHDM_BASE +
                                   NOVOS_PMM_MAX_PHYSICAL_ADDRESS) != 0)
    {
        fail("PAGING TEST FAIL: HHDM RANGE\n");
    }

    test_str("PAGING HHDM 64GiB PASS\n");

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

    if (!paging_map_identity(0x0000000000200000ULL))
        fail("PAGING TEST FAIL: MAP API\n");

    /*
     * Exercise fine-grained permissions with a PMM-owned page.
     */
    const u64 testPage = pmm_alloc_page();
    if (testPage == 0)
        fail("PAGING TEST FAIL: 4K ALLOCATION\n");

    const PagingFlags readOnlyNoExecute{
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

    const u64 entry = paging_get_4k_entry(testPage);

    if ((entry & 0x000FFFFFFFFFF000ULL) != testPage ||
        (entry & NOVOS_PAGE_PRESENT) == 0 ||
        (entry & NOVOS_PAGE_WRITE) != 0 ||
        (entry & NOVOS_PAGE_USER) != 0 ||
        (entry & NOVOS_PAGE_NO_EXECUTE) == 0)
    {
        fail("PAGING TEST FAIL: PERMISSIONS\n");
    }

    if (paging_translate(testPage) != testPage)
        fail("PAGING TEST FAIL: 4K TRANSLATION\n");

    test_str("PAGING 4K RO/NX BITS PASS\n");

    /*
     * Prove NX is enforced by the CPU, not merely encoded in the PTE.
     * The page is writable/executable through the bootstrap identity map
     * while we install a single RET instruction, then remapped RO+NX.
     * Executing it must raise #PF with the recovery RIP below.
     */
    const unsigned long long nxRecovery =
        reinterpret_cast<unsigned long long>(&&nx_page_fault_recovered);

    exception_expect_page_fault(nxRecovery);

    asm volatile(
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

    exception_expect_page_fault(roRecovery);

    *reinterpret_cast<volatile unsigned char*>(testPage) = 0x5A;

ro_write_fault_recovered:

    if (exception_page_fault_test_active())
        fail("PAGING TEST FAIL: RO WRITE DID NOT FIRE\n");

    test_str("PAGING RO WRITE FAULT PASS\n");

    if (!paging_unmap_4k(testPage))
        fail("PAGING TEST FAIL: 4K UNMAP\n");

    if (paging_get_4k_entry(testPage) != 0)
        fail("PAGING TEST FAIL: 4K UNMAP STATE\n");

    /*
     * Trigger one deliberate non-present-page fault. The exception path
     * redirects only this explicitly armed test to the recovery label.
     */
    const unsigned long long recovery =
        reinterpret_cast<unsigned long long>(&&page_fault_recovered);

    exception_expect_page_fault(recovery);

    asm volatile(
        "movq (%%rax), %%rax"
        :
        : "a"(testPage)
        : "memory");

page_fault_recovered:

    if (exception_page_fault_test_active())
        fail("PAGING TEST FAIL: PAGE FAULT DID NOT FIRE\n");

    test_str("PAGING PAGE FAULT RECOVERY PASS\n");

    test_str("PAGING CR3 ACTIVATION PASS\n");
    test_str("PAGING TEST PASS\n");
}
