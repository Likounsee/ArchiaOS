#pragma once

#include <stdint.h>
#include "../../../common/boot_info.h"

using u64 = uint64_t;
using u32 = uint32_t;

constexpr u64 NOVOS_PAGE_SIZE = 0x1000ULL;
constexpr u64 NOVOS_PMM_MAX_PHYSICAL_ADDRESS = 0x8000000000ULL; /* 512 GiB */
constexpr u64 NOVOS_PMM_MAX_FRAMES =
    NOVOS_PMM_MAX_PHYSICAL_ADDRESS / NOVOS_PAGE_SIZE;
extern "C" bool pmm_initialize(BootInfo* bootInfo);
extern "C" u64 pmm_alloc_page();
extern "C" u64 pmm_alloc_page_below(u64 exclusiveLimit);
extern "C" u64 pmm_alloc_page_above(u64 inclusiveBase);
extern "C" u64 pmm_alloc_contiguous(u64 pageCount);
extern "C" void pmm_free_page(u64 physicalAddress);
extern "C" u64 pmm_free_page_count();
