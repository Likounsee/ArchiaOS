#include "pci.hpp"

static constexpr unsigned int PCI_MAX_DEVICES = 64;
static PciDevice devices[PCI_MAX_DEVICES] = {};
static unsigned int device_count = 0;
static bool initialized = false;

static inline void io_out32(uint16_t port, uint32_t value)
{
    asm volatile("outl %0, %1" : : "a"(value), "Nd"(port));
}

static inline uint32_t io_in32(uint16_t port)
{
    uint32_t value;
    asm volatile("inl %1, %0" : "=a"(value) : "Nd"(port));
    return value;
}

static uint32_t config_read32(uint8_t bus, uint8_t device, uint8_t function, uint8_t offset)
{
    const uint32_t address =
        0x80000000U |
        (static_cast<uint32_t>(bus) << 16) |
        (static_cast<uint32_t>(device) << 11) |
        (static_cast<uint32_t>(function) << 8) |
        (offset & 0xFCU);

    io_out32(0xCF8, address);
    return io_in32(0xCFC);
}

extern "C" bool pci_initialize()
{
    device_count = 0;
    for (unsigned int i = 0; i < PCI_MAX_DEVICES; ++i)
        devices[i] = {};

    for (unsigned int device = 0; device < 32 && device_count < PCI_MAX_DEVICES; ++device)
    {
        const uint32_t first = config_read32(0, device, 0, 0);
        if ((first & 0xFFFFU) == 0xFFFFU)
            continue;

        const unsigned int functions =
            (config_read32(0, device, 0, 0x0C) & 0x00800000U) ? 8 : 1;

        for (unsigned int function = 0;
             function < functions && device_count < PCI_MAX_DEVICES;
             ++function)
        {
            const uint32_t id = config_read32(0, device, function, 0);
            if ((id & 0xFFFFU) == 0xFFFFU)
                continue;

            const uint32_t class_info =
                config_read32(0, device, function, 0x08);

            devices[device_count++] = PciDevice{
                0,
                static_cast<uint8_t>(device),
                static_cast<uint8_t>(function),
                static_cast<uint16_t>(id & 0xFFFFU),
                static_cast<uint16_t>(id >> 16),
                static_cast<uint8_t>(class_info >> 24),
                static_cast<uint8_t>(class_info >> 16),
                static_cast<uint8_t>(class_info >> 8)
            };
        }
    }

    initialized = true;
    return device_count != 0;
}

extern "C" unsigned int pci_device_count()
{
    return initialized ? device_count : 0;
}

extern "C" const PciDevice* pci_devices()
{
    return devices;
}

extern "C" const PciDevice* pci_find_class(uint8_t class_code, uint8_t subclass)
{
    for (unsigned int i = 0; i < device_count; ++i)
        if (devices[i].class_code == class_code &&
            devices[i].subclass == subclass)
            return &devices[i];
    return nullptr;
}

extern "C" bool pci_test()
{
    if (!pci_initialize())
        return false;

    for (unsigned int i = 0; i < device_count; ++i)
    {
        if (devices[i].vendor_id != 0xFFFFU)
            return true;
    }
    return false;
}
