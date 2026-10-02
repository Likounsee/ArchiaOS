#pragma once
#include <stdint.h>
#include "../drivers/block.hpp"

struct GptPartition {
    uint32_t index;
    uint64_t first_lba;
    uint64_t last_lba;
    uint64_t attributes;
    uint8_t type_guid[16];
    uint8_t unique_guid[16];
};
extern "C" bool gpt_read_partitions(const BlockDevice*,GptPartition*,uint32_t,uint32_t*);
extern "C" bool gpt_test();
