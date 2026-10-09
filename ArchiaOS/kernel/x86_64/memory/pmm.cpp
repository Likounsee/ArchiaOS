#include "pmm.hpp"
#include "paging.hpp"
#include "../cpu/features.hpp"

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

static u64* pmm_bitmap = nullptr;
static u64* pmm_reserved_bitmap = nullptr;
static u64 pmm_bitmap_words = 0;
static u64 pmm_max_frames = 0;
static u64 pmm_free_pages = 0;

static inline void bitmap_set(u64 frame)
{
    pmm_bitmap[frame >> 6] |= 1ULL << (frame & 63ULL);
}

static inline void bitmap_clear(u64 frame)
{
    pmm_bitmap[frame >> 6] &= ~(1ULL << (frame & 63ULL));
}

static inline void reserved_set(u64 frame)
{
    pmm_reserved_bitmap[frame >> 6] |= 1ULL << (frame & 63ULL);
}

static inline bool reserved_test(u64 frame)
{
    return (pmm_reserved_bitmap[frame >> 6] &
            (1ULL << (frame & 63ULL))) != 0;
}

static inline bool bitmap_test(u64 frame)
{
    return (pmm_bitmap[frame >> 6] &
            (1ULL << (frame & 63ULL))) != 0;
}

static void reserve_range(u64 start, u64 page_count)
{
    if (start >= (pmm_max_frames * PAGE_SIZE))
        return;

    const u64 max_pages =
        (pmm_max_frames * PAGE_SIZE - start +
         PAGE_SIZE - 1) / PAGE_SIZE;

    if (page_count > max_pages)
        page_count = max_pages;

    for (u64 i = 0; i < page_count; ++i)
    {
        const u64 frame = (start / PAGE_SIZE) + i;

        if (!bitmap_test(frame))
        {
            bitmap_set(frame);
            if (pmm_free_pages > 0)
                --pmm_free_pages;
        }
        reserved_set(frame);
    }
}

static void reserve_bytes(u64 start, u64 size)
{
    if (size == 0 || pmm_max_frames == 0 || start >= pmm_max_frames * PAGE_SIZE)
        return;

    const u64 end =
        size > pmm_max_frames * PAGE_SIZE - start
            ? pmm_max_frames * PAGE_SIZE
            : start + size;

    const u64 firstPage = start / PAGE_SIZE;
    const u64 lastPage =
        (end + PAGE_SIZE - 1) / PAGE_SIZE;

    if (lastPage > firstPage)
        reserve_range(
            firstPage * PAGE_SIZE,
            lastPage - firstPage);
}

static void release_range(u64 start, u64 page_count)
{
    if (pmm_max_frames == 0 || start >= pmm_max_frames * PAGE_SIZE)
        return;

    const u64 max_pages =
        (pmm_max_frames * PAGE_SIZE - start) /
        PAGE_SIZE;

    if (page_count > max_pages)
        page_count = max_pages;

    const u64 firstFrame = start / PAGE_SIZE;

    for (u64 i = 0; i < page_count; ++i)
    {
        const u64 frame = firstFrame + i;
        if (reserved_test(frame))
            continue;

        if (bitmap_test(frame))
        {
            bitmap_clear(frame);
            ++pmm_free_pages;
        }
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

static bool reserve_boot_page_table_tree(u64 physical, unsigned int level)
{
    if (physical == 0 ||
        (physical & (PAGE_SIZE - 1ULL)) != 0 ||
        physical >= pmm_max_frames * PAGE_SIZE)
        return false;

    reserve_range(physical, 1);

    if (level == 1)
        return true;

    /*
     * Page-table frames are physical addresses, not guaranteed identity-mapped
     * virtual addresses. Walk them through the shared HHDM just like the
     * allocator metadata, and fail safely if the frame is outside that map.
     */
    const u64 virtual_address = paging_physical_to_virtual(physical);
    if (!virtual_address)
        return false;
    auto* entries = reinterpret_cast<const u64*>(virtual_address);
    for (unsigned int i = 0; i < 512; ++i)
    {
        const u64 entry = entries[i];
        if ((entry & 1ULL) == 0)
            continue;

        if (level <= 3 && (entry & (1ULL << 7)) != 0)
            continue;

        const u64 child = entry & 0x000FFFFFFFFFF000ULL;
        if (!reserve_boot_page_table_tree(child, level - 1))
            return false;
    }

    return true;
}

static bool reserve_boot_page_tables()
{
    u64 cr3 = 0;
    asm volatile("mov %%cr3, %0" : "=r"(cr3));
    cr3 &= ~0xFFFULL;

    return reserve_boot_page_table_tree(cr3, 4);
}

extern "C" bool pmm_initialize(BootInfo* bootInfo)
{
    pmm_bitmap = nullptr;
    pmm_reserved_bitmap = nullptr;
    pmm_bitmap_words = 0;
    pmm_max_frames = 0;
    pmm_free_pages = 0;

    if (bootInfo == nullptr ||
        bootInfo->pmm_bitmap_base == 0 ||
        bootInfo->pmm_bitmap_size < 8192ULL ||
        (bootInfo->pmm_bitmap_size & (PAGE_SIZE - 1ULL)) != 0 ||
        bootInfo->pmm_bitmap_base >= PMM_MAX_PHYSICAL_ADDRESS ||
        bootInfo->pmm_bitmap_size >
            PMM_MAX_PHYSICAL_ADDRESS - bootInfo->pmm_bitmap_base)
        return false;

    /*
     * Paging is already active when the kernel adopts the UEFI mappings.
     * Keep allocator metadata reachable through the shared HHDM, not the
     * low identity alias that disappears in isolated process address spaces.
     */
    const u64 bitmap_virtual =
        paging_physical_to_virtual(bootInfo->pmm_bitmap_base);
    if (!bitmap_virtual ||
        bootInfo->pmm_bitmap_size >
            paging_max_physical_address() - bootInfo->pmm_bitmap_base)
        return false;

    pmm_bitmap = reinterpret_cast<u64*>(bitmap_virtual);
    pmm_bitmap_words = (bootInfo->pmm_bitmap_size / 2ULL) / sizeof(u64);
    pmm_reserved_bitmap = pmm_bitmap + pmm_bitmap_words;
    const u64 bitmap_frames = pmm_bitmap_words * 64ULL;
    u64 supported_frames = PMM_MAX_FRAMES;
    const CpuInfo* cpu = cpu_get_info();
    if (cpu == nullptr || cpu->physical_address_bits < 32)
        return false;
    if (cpu->physical_address_bits < 63)
    {
        const u64 cpu_physical_limit = 1ULL << cpu->physical_address_bits;
        supported_frames = cpu_physical_limit / PAGE_SIZE;
    }
    pmm_max_frames = bitmap_frames < supported_frames
        ? bitmap_frames
        : supported_frames;
    if (pmm_max_frames == 0)
        return false;

    const u64 supported_physical_limit =
        pmm_max_frames * PAGE_SIZE;
    if (bootInfo->pmm_bitmap_base >= supported_physical_limit ||
        bootInfo->pmm_bitmap_size >
            supported_physical_limit - bootInfo->pmm_bitmap_base)
        return false;

    for (u64 i = 0; i < pmm_bitmap_words; ++i)
    {
        pmm_bitmap[i] = ~0ULL;
        pmm_reserved_bitmap[i] = 0;
    }

    pmm_free_pages = 0;

    if (pmm_bitmap == nullptr ||
        bootInfo == nullptr ||
        bootInfo->memory_map_address == 0 ||
        bootInfo->memory_map_size == 0 ||
        bootInfo->memory_descriptor_size <
            sizeof(EfiMemoryDescriptor) ||
        bootInfo->memory_map_size %
            bootInfo->memory_descriptor_size != 0 ||
        bootInfo->memory_map_address >= supported_physical_limit ||
        bootInfo->memory_map_size >
            supported_physical_limit - bootInfo->memory_map_address)
    {
        return false;
    }

    /*
     * BootInfo stores the EFI map's physical address. Parse it through the
     * HHDM rather than assuming the firmware's low identity mapping survives.
     */
    const u64 paging_limit = paging_max_physical_address();
    if (bootInfo->memory_map_address >= paging_limit ||
        bootInfo->memory_map_size >
            paging_limit - bootInfo->memory_map_address)
        return false;
    const u64 memory_map_virtual =
        paging_physical_to_virtual(bootInfo->memory_map_address);
    if (!memory_map_virtual)
        return false;

    const u64 entry_count =
        bootInfo->memory_map_size /
        bootInfo->memory_descriptor_size;

    if (bootInfo->memory_descriptor_count != entry_count)
        return false;

    for (u64 i = 0; i < entry_count; ++i)
    {
        auto* descriptor =
            reinterpret_cast<EfiMemoryDescriptor*>(
                memory_map_virtual +
                i * bootInfo->memory_descriptor_size);

        if (reclaimable_efi_type(descriptor->type))
        {
            release_range(
                descriptor->physical_start,
                descriptor->number_of_pages);
        }
        else
        {
            /* Firmware-reserved pages must remain non-releasable after boot. */
            reserve_range(
                descriptor->physical_start,
                descriptor->number_of_pages);
        }
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

    reserve_bytes(
        bootInfo->pmm_bitmap_base,
        bootInfo->pmm_bitmap_size);

    /*
     * The kernel adopts the UEFI transition CR3 rather than replacing it.
     * Its page-table pages came from EfiLoaderData, so they must remain
     * reserved while that hierarchy is still active.
     */
    if (!reserve_boot_page_tables())
        return false;

    /* Physical address zero is never a valid allocation result. */
    if (!bitmap_test(0))
    {
        bitmap_set(0);
        if (pmm_free_pages > 0)
            --pmm_free_pages;
    }

    return true;
}

extern "C" u64 pmm_alloc_page()
{
    const u64 firstGeneralFrame = 0x100000ULL / PAGE_SIZE;
    for (u64 word_index = firstGeneralFrame / 64ULL;
         word_index < pmm_bitmap_words;
         ++word_index)
    {
        const u64 word = pmm_bitmap[word_index];

        if (word == ~0ULL)
            continue;

        for (u64 bit = 0; bit < 64; ++bit)
        {
            const u64 frame = word_index * 64ULL + bit;

            if (frame >= pmm_max_frames)
                return 0;
            if (frame < firstGeneralFrame)
                continue;

            if ((word & (1ULL << bit)) == 0)
            {
                bitmap_set(frame);

                if (pmm_free_pages > 0)
                    --pmm_free_pages;

                return frame * PAGE_SIZE;
            }
        }
    }

    return 0;
}

extern "C" u64 pmm_alloc_page_below(u64 exclusiveLimit)
{
    if (exclusiveLimit <= PAGE_SIZE)
        return 0;

    /*
     * Only return complete pages strictly below the exclusive limit.
     * An unaligned limit therefore excludes the page containing its
     * final byte.
     */
    u64 maxFrame = exclusiveLimit / PAGE_SIZE;
    if (maxFrame > pmm_max_frames)
        maxFrame = pmm_max_frames;

    /*
     * AP startup vectors are required below 1 MiB. Scan only the bounded
     * low-memory portion instead of changing the general allocator policy.
     */
    for (u64 frame = 1; frame < maxFrame; ++frame)
    {
        if (!bitmap_test(frame))
        {
            bitmap_set(frame);
            if (pmm_free_pages > 0)
                --pmm_free_pages;
            return frame * PAGE_SIZE;
        }
    }

    return 0;
}

extern "C" u64 pmm_alloc_page_above(u64 inclusiveBase)
{
    if (pmm_max_frames == 0 ||
        inclusiveBase > UINT64_MAX - (PAGE_SIZE - 1ULL))
        return 0;

    u64 frame = (inclusiveBase + PAGE_SIZE - 1ULL) / PAGE_SIZE;
    if (frame >= pmm_max_frames)
        return 0;

    for (u64 current = frame; current < pmm_max_frames; ++current)
    {
        if (!bitmap_test(current))
        {
            bitmap_set(current);
            if (pmm_free_pages > 0)
                --pmm_free_pages;
            return current * PAGE_SIZE;
        }
    }
    return 0;
}

extern "C" u64 pmm_alloc_contiguous(u64 pageCount)
{
    if (pageCount == 0 ||
        pmm_max_frames <= 1ULL ||
        pageCount > pmm_max_frames - 1ULL)
    {
        return 0;
    }

    const u64 firstGeneralFrame = 0x200000ULL / PAGE_SIZE;
    u64 runStart = firstGeneralFrame;
    u64 runLength = 0;

    for (u64 frame = firstGeneralFrame;
         frame < pmm_max_frames;
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

                return runStart * PAGE_SIZE;
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
        (physicalAddress & (PAGE_SIZE - 1ULL)) != 0 ||
        pmm_max_frames == 0 || physicalAddress >= pmm_max_frames * PAGE_SIZE)
    {
        return;
    }

    const u64 frame =
        physicalAddress / PAGE_SIZE;

    if (reserved_test(frame))
        return;

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

extern "C" u64 pmm_max_physical_address()
{
    return pmm_max_frames * PAGE_SIZE;
}
