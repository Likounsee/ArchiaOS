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

    u64 free_before = pmm_free_page_count();

    if (bootInfo->kernel_image_base != 0)
    {
        const u64 reserved_page =
            bootInfo->kernel_image_base & ~(NOVOS_PAGE_SIZE - 1ULL);
        pmm_free_page(reserved_page);
        if (pmm_free_page_count() != free_before)
        {
            debug_str("PMM TEST FAIL: RESERVED FRAME FREED\n");
            for (;;) asm volatile ("cli; hlt");
        }
    }

    if (free_before == 0)
    {
        debug_str("PMM TEST FAIL: NO FREE PAGES\n");
        for (;;) asm volatile ("cli; hlt");
    }

    const u64 free_before_overflow = pmm_free_page_count();
    if (pmm_alloc_page_above(UINT64_MAX) != 0 ||
        pmm_free_page_count() != free_before_overflow)
    {
        debug_str("PMM TEST FAIL: ABOVE OVERFLOW GUARD\n");
        for (;;) asm volatile ("cli; hlt");
    }

    u64 above_page = pmm_alloc_page_above(0x00200000ULL);
    if (above_page == 0 ||
        (above_page & (NOVOS_PAGE_SIZE - 1ULL)) != 0 ||
        above_page < 0x00200000ULL)
    {
        debug_str("PMM TEST FAIL: ABOVE ALLOCATION\n");
        for (;;) asm volatile ("cli; hlt");
    }
    pmm_free_page(above_page);

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

    const u64 free_before_double_free = pmm_free_page_count();
    pmm_free_page(page_a);
    pmm_free_page(page_b);

    if (pmm_free_page_count() != free_before_double_free)
    {
        debug_str("PMM TEST FAIL: DOUBLE FREE\n");
        for (;;) asm volatile ("cli; hlt");
    }

    const u64 free_before_contiguous = pmm_free_page_count();
    const u64 contiguous = pmm_alloc_contiguous(4);

    if (contiguous == 0 ||
        (contiguous & 0xFFFULL) != 0 ||
        pmm_free_page_count() + 4 != free_before_contiguous)
    {
        debug_str("PMM TEST FAIL: CONTIGUOUS ALLOCATION\n");
        for (;;) asm volatile ("cli; hlt");
    }

    /*
     * Verify every page in the returned run is independently addressable
     * and aligned. The allocator's contiguous-run invariant is then
     * exercised again by freeing the complete range.
     */
    for (u64 i = 0; i < 4; ++i)
    {
        const u64 page = contiguous + i * NOVOS_PAGE_SIZE;

        if ((page & (NOVOS_PAGE_SIZE - 1ULL)) != 0 ||
            page == 0 ||
            page >= NOVOS_PMM_MAX_PHYSICAL_ADDRESS)
        {
            debug_str("PMM TEST FAIL: CONTIGUOUS RANGE\n");
            for (;;) asm volatile ("cli; hlt");
        }
    }

    debug_str("PMM CONTIGUOUS PASS\n");

    for (u64 i = 0; i < 4; ++i)
        pmm_free_page(contiguous + i * NOVOS_PAGE_SIZE);

    if (pmm_free_page_count() != free_before_contiguous)
    {
        debug_str("PMM TEST FAIL: CONTIGUOUS FREE\n");
        for (;;) asm volatile ("cli; hlt");
    }

    debug_str("PMM TEST PASS\n");
    debug_str("PMM FREE PAGES: ");
    debug_hex64(pmm_free_page_count());
    debug_str("\n");
}
