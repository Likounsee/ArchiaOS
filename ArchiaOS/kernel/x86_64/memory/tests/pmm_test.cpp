#include "pmm.hpp"
#include "../paging.hpp"
#include "../cpu/features.hpp"

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
    const CpuInfo* cpu = cpu_get_info();
    if (cpu == nullptr || pmm_max_physical_address() == 0)
    {
        debug_str("PMM TEST FAIL: CPU PHYSICAL LIMIT\n");
        for (;;) asm volatile ("cli; hlt");
    }
    if (cpu->physical_address_bits < 63)
    {
        const u64 cpu_limit = 1ULL << cpu->physical_address_bits;
        if (pmm_max_physical_address() > cpu_limit ||
            pmm_alloc_page_above(cpu_limit) != 0)
        {
            debug_str("PMM TEST FAIL: CPU PHYSICAL LIMIT\n");
            for (;;) asm volatile ("cli; hlt");
        }
    }

    u64 boot_pml4 = 0;
    asm volatile("mov %%cr3, %0" : "=r"(boot_pml4));
    boot_pml4 &= ~(PAGE_SIZE - 1ULL);
    if (boot_pml4 == 0)
    {
        debug_str("PMM TEST FAIL: BOOT CR3\n");
        for (;;) asm volatile ("cli; hlt");
    }
    const u64 free_before_boot_pml4 = pmm_free_page_count();
    pmm_free_page(boot_pml4);
    if (pmm_free_page_count() != free_before_boot_pml4)
    {
        debug_str("PMM TEST FAIL: BOOT PML4 FREED\n");
        for (;;) asm volatile ("cli; hlt");
    }

    if (bootInfo->kernel_image_base != 0)
    {
        const u64 reserved_page =
            bootInfo->kernel_image_base & ~(PAGE_SIZE - 1ULL);
        pmm_free_page(reserved_page);
        if (pmm_free_page_count() != free_before)
        {
            debug_str("PMM TEST FAIL: RESERVED FRAME FREED\n");
            for (;;) asm volatile ("cli; hlt");
        }
    }

    struct TestEfiMemoryDescriptor
    {
        u32 type;
        u32 pad;
        u64 physical_start;
        u64 virtual_start;
        u64 number_of_pages;
        u64 attribute;
    };

    bool checked_firmware_reserved = false;
    const u64 memory_map_virtual =
        paging_physical_to_virtual(bootInfo->memory_map_address);
    if (!memory_map_virtual)
    {
        debug_str("PMM TEST FAIL: MEMORY MAP HHDM\n");
        for (;;) asm volatile ("cli; hlt");
    }
    const u64 entry_count =
        bootInfo->memory_map_size / bootInfo->memory_descriptor_size;
    for (u64 i = 0; i < entry_count; ++i)
    {
        const auto* descriptor =
            reinterpret_cast<const TestEfiMemoryDescriptor*>(
                memory_map_virtual +
                i * bootInfo->memory_descriptor_size);

        const bool reclaimable =
            descriptor->type == 1 ||
            descriptor->type == 2 ||
            descriptor->type == 3 ||
            descriptor->type == 4 ||
            descriptor->type == 7;
        if (reclaimable || descriptor->number_of_pages == 0 ||
            descriptor->physical_start == 0)
            continue;

        const u64 reserved_firmware_page =
            descriptor->physical_start & ~(PAGE_SIZE - 1ULL);
        const u64 before_firmware_free = pmm_free_page_count();
        pmm_free_page(reserved_firmware_page);
        if (pmm_free_page_count() != before_firmware_free)
        {
            debug_str("PMM TEST FAIL: FIRMWARE FRAME FREED\n");
            for (;;) asm volatile ("cli; hlt");
        }
        checked_firmware_reserved = true;
        break;
    }

    if (!checked_firmware_reserved)
    {
        debug_str("PMM TEST FAIL: NO FIRMWARE RESERVED RANGE\n");
        for (;;) asm volatile ("cli; hlt");
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
        (above_page & (PAGE_SIZE - 1ULL)) != 0 ||
        above_page < 0x00200000ULL)
    {
        debug_str("PMM TEST FAIL: ABOVE ALLOCATION\n");
        for (;;) asm volatile ("cli; hlt");
    }
    pmm_free_page(above_page);

    const u64 free_before_unaligned_below = pmm_free_page_count();
    const u64 unaligned_limit = 0x00200001ULL;
    const u64 below_page = pmm_alloc_page_below(unaligned_limit);
    if (below_page == 0 || below_page >= unaligned_limit ||
        (below_page & (PAGE_SIZE - 1ULL)) != 0)
    {
        debug_str("PMM TEST FAIL: UNALIGNED BELOW LIMIT\n");
        for (;;) asm volatile ("cli; hlt");
    }
    pmm_free_page(below_page);
    if (pmm_free_page_count() != free_before_unaligned_below)
    {
        debug_str("PMM TEST FAIL: UNALIGNED BELOW COUNT\n");
        for (;;) asm volatile ("cli; hlt");
    }

    const u64 free_before_tiny_below = pmm_free_page_count();
    if (pmm_alloc_page_below(PAGE_SIZE + 1ULL) != 0 ||
        pmm_free_page_count() != free_before_tiny_below)
    {
        debug_str("PMM TEST FAIL: TINY BELOW LIMIT\n");
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

    const u64 free_before_double_free = pmm_free_page_count();
    pmm_free_page(page_a);
    pmm_free_page(page_b);

    if (pmm_free_page_count() != free_before_double_free)
    {
        debug_str("PMM TEST FAIL: DOUBLE FREE\n");
        for (;;) asm volatile ("cli; hlt");
    }

    /*
     * Invalid frees must be ignored without corrupting the bitmap or the
     * free-page counter. Include zero, an unaligned address, and the exact
     * exclusive physical limit.
     */
    const u64 free_before_invalid_free = pmm_free_page_count();
    pmm_free_page(0);
    pmm_free_page(page_a + 1ULL);
    pmm_free_page(pmm_max_physical_address());
    if (pmm_free_page_count() != free_before_invalid_free)
    {
        debug_str("PMM TEST FAIL: INVALID FREE\n");
        for (;;) asm volatile ("cli; hlt");
    }

    /*
     * Requests that cannot be satisfied must fail without consuming pages.
     */
    const u64 free_before_invalid_contiguous = pmm_free_page_count();
    if (pmm_alloc_contiguous(0) != 0 ||
        pmm_alloc_contiguous(pmm_max_physical_address() / PAGE_SIZE) != 0 ||
        pmm_free_page_count() != free_before_invalid_contiguous)
    {
        debug_str("PMM TEST FAIL: INVALID CONTIGUOUS REQUEST\n");
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
        const u64 page = contiguous + i * PAGE_SIZE;

        if ((page & (PAGE_SIZE - 1ULL)) != 0 ||
            page == 0 ||
            page >= PMM_MAX_PHYSICAL_ADDRESS)
        {
            debug_str("PMM TEST FAIL: CONTIGUOUS RANGE\n");
            for (;;) asm volatile ("cli; hlt");
        }
    }

    debug_str("PMM CONTIGUOUS PASS\n");

    for (u64 i = 0; i < 4; ++i)
        pmm_free_page(contiguous + i * PAGE_SIZE);

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
