#include "paging.hpp"

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

    if (!paging_activate())
        fail("PAGING TEST FAIL: ACTIVATION\n");

    if (!paging_is_enabled())
        fail("PAGING TEST FAIL: PAGING DISABLED\n");

    if (paging_translate(0x0000000012345678ULL) !=
        0x0000000012345678ULL)
        fail("PAGING TEST FAIL: POST-ACTIVATE TRANSLATION\n");

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

    test_str("PAGING 4K RO/NX PASS\n");

    test_str("PAGING CR3 ACTIVATION PASS\n");
    test_str("PAGING TEST PASS\n");
}
