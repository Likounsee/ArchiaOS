#include "storage.hpp"

static constexpr unsigned int STORAGE_MAX_CONTROLLERS = 8;
static StorageController controllers[STORAGE_MAX_CONTROLLERS] = {};
static unsigned int controller_count = 0;

static bool add_controller(StorageControllerType type,const PciDevice* pci,uint64_t mmio)
{
    if (!pci||controller_count>=STORAGE_MAX_CONTROLLERS)
        return false;
    controllers[controller_count++] = {type,mmio,pci};
    return true;
}

extern "C" bool storage_initialize()
{
    controller_count=0;
    for (unsigned int i=0;i<STORAGE_MAX_CONTROLLERS;++i)
        controllers[i]={};

    if (!pci_device_count() && !pci_initialize())
        return false;

    for (unsigned int i=0;i<pci_device_count();++i)
    {
        const PciDevice* device=&pci_devices()[i];
        if (device->class_code!=0x01)
            continue;

        if (device->subclass==0x06 && device->programming_interface==0x01)
        {
            const PciBar* bar=pci_get_bar(device,5);
            if (bar&&bar->present&&bar->base)
                add_controller(STORAGE_CONTROLLER_AHCI,device,bar->base);
        }
        else if (device->subclass==0x08 && device->programming_interface==0x02)
        {
            const PciBar* bar=pci_get_bar(device,0);
            if (bar&&bar->present&&bar->base)
                add_controller(STORAGE_CONTROLLER_NVME,device,bar->base);
        }
    }
    return true;
}

extern "C" unsigned int storage_controller_count()
{
    return controller_count;
}

extern "C" const StorageController* storage_controller_get(unsigned int index)
{
    if (index>=controller_count)
        return nullptr;
    return &controllers[index];
}

extern "C" bool storage_test()
{
    if (!storage_initialize())
        return false;

    for (unsigned int i=0;i<controller_count;++i)
    {
        const StorageController* c=&controllers[i];
        if (!c->pci||!c->mmio_base)
            return false;
        if (c->type!=STORAGE_CONTROLLER_AHCI &&
            c->type!=STORAGE_CONTROLLER_NVME)
            return false;
    }
    return true;
}
