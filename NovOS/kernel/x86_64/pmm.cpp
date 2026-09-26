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

static_assert(sizeof(EfiMemoryDescriptor) == 40,
              "EFI memory descriptor layout must be 40 bytes");

static u64 pmm_bitmap[NOVOS_PMM_BITMAP_WORDS];
static u64 pmm_free_pages = 0;

static inline void bitmap_set(u64 frame)
{
    pmm_bitmap[frame >> 6] |= (1ULL << (frame & 63ULL));
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
    for (u64 i = 0; i < page_count; ++i)
    {
        u64 address = start + (i * NOVOS_PAGE_SIZE);

        if (address >= NOVOS_PMM_MAX_PHYSICAL_ADDRESS)
            break;

        bitmap_set(address / NOVOS_PAGE_SIZE);
    }
}

static void release_range(u64 start, u64 page_count)
{
    for (u64 i = 0; i < page_count; ++i)
    {
        u64 address = start + (i * NOVOS_PAGE_SIZE);

        if (address >= NOVOS_PMM_MAX_PHYSICAL_ADDRESS)
            break;

        bitmap_clear(address / NOVOS_PAGE_SIZE);
        ++pmm_free_pages;
    }
}

extern "C" void pmm_initialize(BootInfo* bootInfo)
{
    for (u64 i = 0; i < NOVOS_PMM_BITMAP_WORDS; ++i)
        pmm_bitmap[i] = ~0ULL;

    pmm_free_pages = 0;

    if (bootInfo == nullptr ||
        bootInfo->memory_map_address == 0 ||
        bootInfo->memory_map_size == 0 ||
        bootInfo->memory_descriptor_size < sizeof(EfiMemoryDescriptor))
    {
        return;
    }

    /*
     * EfiConventionalMemory = 7.
     * Only conventional memory is initially released. This keeps all
     * boot-services/runtime/reserved regions unavailable to the kernel.
     */
    constexpr u32 EFI_CONVENTIONAL_MEMORY = 7;

    u64 entry_count =
        bootInfo->memory_map_size /
        bootInfo->memory_descriptor_size;

    for (u64 i = 0; i < entry_count; ++i)
    {
        auto* descriptor =
            reinterpret_cast<EfiMemoryDescriptor*>(
                bootInfo->memory_map_address +
                (i * bootInfo->memory_descriptor_size)
            );

        if (descriptor->type != EFI_CONVENTIONAL_MEMORY)
            continue;

        release_range(
            descriptor->physical_start,
            descriptor->number_of_pages
        );
    }

    /*
     * Physical address zero is kept reserved. It is useful as an invalid
     * page-allocation sentinel and should never be handed to the kernel.
     */
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
        u64 word = pmm_bitmap[word_index];

        if (word == ~0ULL)
            continue;

        for (u64 bit = 0; bit < 64; ++bit)
        {
            u64 frame = (word_index * 64ULL) + bit;

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

extern "C" void pmm_free_page(u64 physicalAddress)
{
    if (physicalAddress == 0 ||
        (physicalAddress & (NOVOS_PAGE_SIZE - 1ULL)) != 0 ||
        physicalAddress >= NOVOS_PMM_MAX_PHYSICAL_ADDRESS)
    {
        return;
    }

    u64 frame = physicalAddress / NOVOS_PAGE_SIZE;

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
