#pragma once
#include <stdint.h>
using u64 = uint64_t;
extern "C" bool heap_initialize();
extern "C" void* kmalloc(u64 size);
extern "C" void kfree(void* pointer);
extern "C" u64 heap_used_bytes();
extern "C" void heap_run_tests();
