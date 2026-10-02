#pragma once
#include <stdint.h>
#include "pci.hpp"

enum StorageControllerType : uint32_t
{
    STORAGE_CONTROLLER_AHCI = 1,
    STORAGE_CONTROLLER_NVME = 2
};

struct StorageController
{
    StorageControllerType type;
    uint64_t mmio_base;
    const PciDevice* pci;
};

extern "C" bool storage_initialize();
extern "C" unsigned int storage_controller_count();
extern "C" const StorageController* storage_controller_get(unsigned int index);
extern "C" bool storage_test();
