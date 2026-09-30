#include "pmm.hpp"

struct EfiMemoryDescriptor
{
    u32 type;
    u32 reserved;
    u64 physical_start;
    u64 virtual_start;
    u64 number_of_pages;
    u64 attribute;
};

static_assert(
    sizeof(EfiMemoryDescriptor) == 40,
    "EFI memory descriptor layout must be 40 bytes");

static u64 pmm_bitmap[NOVOS_PMM_BITMAP_WORDS];
static u64 pmm_free_pages = 0;

static inline void bitmap_set(u64 frame)
{
    pmm_bitmap[frame >> 6] |= 1ULL << (frame & 63ULL);
}

static inline void bitmap_clear(u64 frame)
{
    pmm_bitmap[frame >> 6] &= ~(1ULL << (frame & 63ULL));
}

static inline bool bitmap_test(u64 frame)
{
    return (pmm_bitmap[frame >> 6] &
            (1ULL << (frame & 63ULL))) != 0;
}

static void reserve_range(u64 start, u64 page_count)
{
    if (start >= NOVOS_PMM_MAX_PHYSICAL_ADDRESS)
        return;

    const u64 max_pages =
        (NOVOS_PMM_MAX_PHYSICAL_ADDRESS - start +
         NOVOS_PAGE_SIZE - 1) /
        NOVOS_PAGE_SIZE;

    if (page_count > max_pages)
        page_count = max_pages;

    for (u64 i = 0; i < page_count; ++i)
    {
        const u64 frame = (start / NOVOS_PAGE_SIZE) + i;

        if (!bitmap_test(frame))
        {
            bitmap_set(frame);
            if (pmm_free_pages > 0)
                --pmm_free_pages;
        }
    }
}

static void reserve_bytes(u64 start, u64 size)
{
    if (size == 0 || start >= NOVOS_PMM_MAX_PHYSICAL_ADDRESS)
        return;

    const u64 end =
        size > NOVOS_PMM_MAX_PHYSICAL_ADDRESS - start
            ? NOVOS_PMM_MAX_PHYSICAL_ADDRESS
            : start + size;

    const u64 firstPage = start / NOVOS_PAGE_SIZE;
    const u64 lastPage =
        (end + NOVOS_PAGE_SIZE - 1) / NOVOS_PAGE_SIZE;

    if (lastPage > firstPage)
        reserve_range(
            firstPage * NOVOS_PAGE_SIZE,
            lastPage - firstPage);
}

static void release_range(u64 start, u64 page_count)
{
    if (start >= NOVOS_PMM_MAX_PHYSICAL_ADDRESS)
        return;

    const u64 max_pages =
        (NOVOS_PMM_MAX_PHYSICAL_ADDRESS - start) /
        NOVOS_PAGE_SIZE;

    if (page_count > max_pages)
        page_count = max_pages;

    const u64 firstFrame = start / NOVOS_PAGE_SIZE;

    for (u64 i = 0; i < page_count; ++i)
    {
        const u64 frame = firstFrame + i;
        bitmap_clear(frame);
        ++pmm_free_pages;
    }
}

static bool reclaimable_efi_type(u32 type)
{
    /*
     * After ExitBootServices(), the loader/OS owns unused memory
     * of these types. Runtime memory (5/6), ACPI NVS and other
     * firmware-reserved types remain unavailable.
     */
    return type == 1 || /* EfiLoaderCode */
           type == 2 || /* EfiLoaderData */
           type == 3 || /* EfiBootServicesCode */
           type == 4 || /* EfiBootServicesData */
           type == 7;   /* EfiConventionalMemory */
}

extern "C" void pmm_initialize(BootInfo* bootInfo)
{
    for (u64 i = 0; i < NOVOS_PMM_BITMAP_WORDS; ++i)
        pmm_bitmap[i] = ~0ULL;

    pmm_free_pages = 0;

    if (bootInfo == nullptr ||
        bootInfo->memory_map_address == 0 ||
        bootInfo->memory_map_size == 0 ||
        bootInfo->memory_descriptor_size <
            sizeof(EfiMemoryDescriptor))
    {
        return;
    }

    const u64 entry_count =
        bootInfo->memory_map_size /
        bootInfo->memory_descriptor_size;

    for (u64 i = 0; i < entry_count; ++i)
    {
        auto* descriptor =
            reinterpret_cast<EfiMemoryDescriptor*>(
                bootInfo->memory_map_address +
                i * bootInfo->memory_descriptor_size);

        if (!reclaimable_efi_type(descriptor->type))
            continue;

        release_range(
            descriptor->physical_start,
            descriptor->number_of_pages);
    }

    /*
     * Never return memory occupied by the kernel, BootInfo,
     * memory-map buffer, or the framebuffer to the allocator.
     */
    reserve_bytes(
        bootInfo->kernel_image_base,
        bootInfo->kernel_image_size);

    reserve_bytes(
        bootInfo->boot_info_address,
        bootInfo->boot_info_size);

    reserve_bytes(
        bootInfo->memory_map_address,
        bootInfo->memory_map_size);

    reserve_bytes(
        bootInfo->framebuffer_base,
        bootInfo->framebuffer_size);

    /* Physical address zero is never a valid allocation result. */
    if (!bitmap_test(0))
    {
        bitmap_set(0);
        if (pmm_free_pages > 0)
            --pmm_free_pages;
    }
}

extern "C" u64 pmm_alloc_page()
{
    for (u64 word_index = 0;
         word_index < NOVOS_PMM_BITMAP_WORDS;
         ++word_index)
    {
        const u64 word = pmm_bitmap[word_index];

        if (word == ~0ULL)
            continue;

        for (u64 bit = 0; bit < 64; ++bit)
        {
            const u64 frame = word_index * 64ULL + bit;

            if (frame >= NOVOS_PMM_MAX_FRAMES)
                return 0;

            if ((word & (1ULL << bit)) == 0)
            {
                bitmap_set(frame);

                if (pmm_free_pages > 0)
                    --pmm_free_pages;

                return frame * NOVOS_PAGE_SIZE;
            }
        }
    }

    return 0;
}

extern "C" u64 pmm_alloc_contiguous(u64 pageCount)
{
    if (pageCount == 0 ||
        pageCount > NOVOS_PMM_MAX_FRAMES - 1ULL)
    {
        return 0;
    }

    u64 runStart = 1;
    u64 runLength = 0;

    for (u64 frame = 1;
         frame < NOVOS_PMM_MAX_FRAMES;
         ++frame)
    {
        if (!bitmap_test(frame))
        {
            if (runLength == 0)
                runStart = frame;

            ++runLength;

            if (runLength == pageCount)
            {
                for (u64 i = 0; i < pageCount; ++i)
                    bitmap_set(runStart + i);

                if (pmm_free_pages >= pageCount)
                    pmm_free_pages -= pageCount;
                else
                    pmm_free_pages = 0;

                return runStart * NOVOS_PAGE_SIZE;
            }
        }
        else
        {
            runLength = 0;
        }
    }

    return 0;
}

extern "C" void pmm_free_page(u64 physicalAddress)
{
    if (physicalAddress == 0 ||
        (physicalAddress & (NOVOS_PAGE_SIZE - 1ULL)) != 0 ||
        physicalAddress >= NOVOS_PMM_MAX_PHYSICAL_ADDRESS)
    {
        return;
    }

    const u64 frame =
        physicalAddress / NOVOS_PAGE_SIZE;

    if (bitmap_test(frame))
    {
        bitmap_clear(frame);
        ++pmm_free_pages;
    }
}

extern "C" u64 pmm_free_page_count()
{
    return pmm_free_pages;
}
