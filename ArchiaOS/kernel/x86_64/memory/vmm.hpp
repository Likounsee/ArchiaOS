#pragma once
#include <stdint.h>

using u64 = uint64_t;

struct VmmRegion
{
    u64 base;
    u64 pages;
    bool user;
    bool writable;
    bool executable;
    bool mapped;
    bool used;
};

extern "C" bool vmm_initialize();
extern "C" u64 vmm_alloc_pages(u64 pages, bool user, bool writable, bool executable);
extern "C" bool vmm_free_pages(u64 base, u64 pages);
extern "C" bool vmm_reserve(u64 base, u64 pages, bool user);
extern "C" bool vmm_release(u64 base, u64 pages);
extern "C" bool vmm_is_mapped(u64 virtual_address);
extern "C" u64 vmm_allocated_pages();
extern "C" void vmm_run_tests();
