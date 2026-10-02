#pragma once
#include <stdint.h>
using u64 = uint64_t;

struct AddressSpace
{
    u64 pml4_physical;
    bool active;
};

extern "C" bool address_space_create(AddressSpace* space);
extern "C" bool address_space_map(AddressSpace* space, u64 virtual_address, u64 physical_address, bool writable, bool executable);
extern "C" bool address_space_activate(AddressSpace* space);
extern "C" bool address_space_destroy(AddressSpace* space);
extern "C" bool address_space_is_user_mapped(const AddressSpace* space, u64 virtual_address);
extern "C" void address_space_run_tests();
