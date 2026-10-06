#pragma once

#include <stdint.h>

struct PciBar
{
    uint64_t base;
    uint32_t flags;
    bool present;
};

struct PciDevice
{
    uint8_t bus;
    uint8_t device;
    uint8_t function;
    uint16_t vendor_id;
    uint16_t device_id;
    uint8_t class_code;
    uint8_t subclass;
    uint8_t programming_interface;
    PciBar bars[6];
};

extern "C" bool pci_initialize();
extern "C" unsigned int pci_device_count();
extern "C" const PciDevice* pci_devices();
extern "C" const PciDevice* pci_find_class(uint8_t class_code, uint8_t subclass);
extern "C" const PciBar* pci_get_bar(const PciDevice* device, uint8_t index);
extern "C" bool pci_test();
extern "C" bool pci_enable_bus_master(const PciDevice* device);
