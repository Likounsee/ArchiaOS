#pragma once
#include <stdint.h>
#include "../drivers/block.hpp"
#include "openfs/block_device.h"

extern "C" bool openfs_kernel_attach(
    const BlockDevice* device,
    uint64_t first_lba,
    uint32_t openfs_block_size,
    openfs_block_device_t* out_device);

extern "C" void openfs_kernel_detach(openfs_block_device_t* device);

extern "C" uint32_t openfs_kernel_test();
