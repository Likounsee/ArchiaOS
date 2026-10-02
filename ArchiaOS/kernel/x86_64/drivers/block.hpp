#pragma once
#include <stdint.h>

enum BlockDeviceType : uint32_t { BLOCK_DEVICE_MEMORY=1, BLOCK_DEVICE_AHCI=2, BLOCK_DEVICE_NVME=3, BLOCK_DEVICE_USB=4 };

struct BlockDevice {
    uint32_t id;
    BlockDeviceType type;
    uint32_t sector_size;
    uint64_t sector_count;
    bool (*read)(const BlockDevice*,uint64_t,uint32_t,void*);
    bool (*write)(const BlockDevice*,uint64_t,uint32_t,const void*);
    void* context;
};

extern "C" bool block_initialize();
extern "C" bool block_register(BlockDevice*,uint32_t*);
extern "C" const BlockDevice* block_get(uint32_t);
extern "C" unsigned int block_device_count();
extern "C" bool block_read(const BlockDevice*,uint64_t,uint32_t,void*);
extern "C" bool block_write(const BlockDevice*,uint64_t,uint32_t,const void*);
extern "C" bool block_memory_test();
