#pragma once

#include "pmm.hpp"

constexpr u64 NOVOS_PAGE_PRESENT = 1ULL << 0;
constexpr u64 NOVOS_PAGE_WRITE = 1ULL << 1;
constexpr u64 NOVOS_PAGE_HUGE = 1ULL << 7;

extern "C" bool paging_initialize();
extern "C" u64 paging_pml4_physical();
extern "C" u64 paging_translate(u64 virtualAddress);
extern "C" bool paging_map_identity(u64 physicalAddress);
extern "C" bool paging_is_enabled();
extern "C" void paging_run_tests();
