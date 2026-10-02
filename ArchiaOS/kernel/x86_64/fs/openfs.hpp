#pragma once
#include <stdint.h>
#include "../drivers/block.hpp"

static constexpr uint32_t OPENFS_MAGIC = 0x4F504E46U;
static constexpr uint32_t OPENFS_VERSION = 1;
static constexpr uint32_t OPENFS_BLOCK_SIZE = 4096;
static constexpr uint32_t OPENFS_INODE_SIZE = 128;
static constexpr uint32_t OPENFS_NAME_SIZE = 48;

struct OpenFsInode {
    uint32_t mode;
    uint32_t links;
    uint64_t size;
    uint64_t direct[8];
    uint64_t created;
    uint64_t modified;
    uint8_t reserved[32];
} __attribute__((packed));

extern "C" bool openfs_format(const BlockDevice* device);
extern "C" bool openfs_mount(const BlockDevice* device);
extern "C" bool openfs_create(const char* name, uint32_t mode);
extern "C" bool openfs_mkdir(const char* path);
extern "C" bool openfs_unlink(const char* name);
extern "C" bool openfs_write(const char* name, uint64_t offset, const void* data, uint64_t size);
extern "C" bool openfs_read(const char* name, uint64_t offset, void* data, uint64_t size, uint64_t* read_size);
extern "C" bool openfs_test();
