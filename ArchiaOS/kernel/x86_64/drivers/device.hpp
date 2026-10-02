#pragma once

#include <stdint.h>

enum DeviceType : uint32_t
{
    DEVICE_PCI = 1,
    DEVICE_INPUT = 2,
    DEVICE_DISPLAY = 3,
    DEVICE_STORAGE = 4,
    DEVICE_NETWORK = 5
};

struct Device
{
    uint32_t id;
    uint32_t type;
    uint32_t state;
    uint64_t resource0;
    uint64_t resource1;
};

extern "C" bool device_manager_initialize();
extern "C" bool device_manager_register(DeviceType type, uint64_t resource0, uint64_t resource1, uint32_t* id);
extern "C" const Device* device_manager_get(uint32_t id);
extern "C" unsigned int device_manager_count();
extern "C" bool device_manager_test();
