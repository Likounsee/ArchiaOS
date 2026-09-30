#include "pmm.hpp"

static inline void debug_char(char c)
{
    asm volatile (
        "outb %0, %1"
        :
        : "a"(c),
          "Nd"(static_cast<unsigned short>(0xE9))
    );
}

static void debug_str(const char* s)
{
    for (int i = 0; s[i] != '\0'; ++i)
        debug_char(s[i]);
}

static void debug_hex64(u64 value)
{
    const char* digits = "0123456789ABCDEF";
    debug_char('0');
    debug_char('x');

    for (int i = 0; i < 16; ++i)
        debug_char(digits[(value >> ((15 - i) * 4)) & 0xF]);
}

extern "C" void pmm_run_tests(BootInfo* bootInfo)
{
    debug_str("PMM TEST START\n");

    pmm_initialize(bootInfo);

    u64 free_before = pmm_free_page_count();

    if (free_before == 0)
    {
        debug_str("PMM TEST FAIL: NO FREE PAGES\n");
        for (;;) asm volatile ("cli; hlt");
    }

    u64 page_a = pmm_alloc_page();
    u64 page_b = pmm_alloc_page();

    if (page_a == 0 || page_b == 0 || page_a == page_b)
    {
        debug_str("PMM TEST FAIL: ALLOCATION\n");
        for (;;) asm volatile ("cli; hlt");
    }

    if ((page_a & 0xFFFULL) != 0 ||
        (page_b & 0xFFFULL) != 0)
    {
        debug_str("PMM TEST FAIL: ALIGNMENT\n");
        for (;;) asm volatile ("cli; hlt");
    }

    u64 free_after_alloc = pmm_free_page_count();

    if (free_after_alloc + 2 != free_before)
    {
        debug_str("PMM TEST FAIL: COUNT AFTER ALLOC\n");
        for (;;) asm volatile ("cli; hlt");
    }

    pmm_free_page(page_a);
    pmm_free_page(page_b);

    if (pmm_free_page_count() != free_before)
    {
        debug_str("PMM TEST FAIL: FREE\n");
        for (;;) asm volatile ("cli; hlt");
    }

    pmm_free_page(page_a);

    if (pmm_free_page_count() != free_before)
    {
        debug_str("PMM TEST FAIL: DOUBLE FREE\n");
        for (;;) asm volatile ("cli; hlt");
    }

    debug_str("PMM TEST PASS\n");
    debug_str("PMM FREE PAGES: ");
    debug_hex64(pmm_free_page_count());
    debug_str("\n");
}
