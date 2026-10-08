#pragma once

#include "pmm.hpp"

constexpr u64 PAGE_PRESENT = 1ULL << 0;
constexpr u64 PAGE_WRITE = 1ULL << 1;
constexpr u64 PAGE_USER = 1ULL << 2;
constexpr u64 PAGE_HUGE = 1ULL << 7;
constexpr u64 PAGE_NO_EXECUTE = 1ULL << 63;

/*
 * Four-level paging layout:
 *   low half  = bootstrap identity map
 *   high half = HHDM/direct physical map
 *
 * 0xFFFF800000000000 is 48-bit canonical and corresponds to PML4 index 256.
 */
constexpr u64 HHDM_BASE = 0xFFFF800000000000ULL;

/* Reserved virtual-address layout for the next VM stages. */
constexpr u64 KERNEL_VIRTUAL_BASE = 0xFFFFFFFF80000000ULL;
constexpr u64 KERNEL_HEAP_BASE = 0xFFFF900000000000ULL;
constexpr u64 KERNEL_HEAP_SIZE = 0x0000001000000000ULL; /* 64 GiB */
constexpr u64 USER_VIRTUAL_BASE = 0x0000000000400000ULL;
constexpr u64 USER_VIRTUAL_TOP = 0x00007FFFFFFFF000ULL;

struct PagingFlags
{
    bool writable;
    bool user;
    bool executable;
    bool cache_disable;
    bool write_through;
};

extern "C" bool paging_initialize();
extern "C" u64 paging_pml4_physical();
extern "C" u64 paging_max_physical_address();
extern "C" u64 paging_translate(u64 virtualAddress);
extern "C" bool paging_map_identity(u64 physicalAddress);
extern "C" bool paging_map_4k(
    u64 virtualAddress,
    u64 physicalAddress,
    PagingFlags flags);
extern "C" u64 paging_get_4k_entry(u64 virtualAddress);
extern "C" bool paging_unmap_4k(u64 virtualAddress);
extern "C" u64 paging_physical_to_virtual(u64 physicalAddress);
extern "C" u64 paging_virtual_to_physical(u64 virtualAddress);
extern "C" bool paging_activate();
extern "C" bool paging_is_enabled();
extern "C" void paging_run_tests();
