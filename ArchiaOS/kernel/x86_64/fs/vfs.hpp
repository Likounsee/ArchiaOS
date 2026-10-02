#pragma once

#include <stdint.h>

enum VfsNodeType : uint32_t
{
    VFS_NODE_UNUSED = 0,
    VFS_NODE_DIRECTORY = 1,
    VFS_NODE_FILE = 2
};

extern "C" bool vfs_initialize();
extern "C" bool vfs_mkdir(const char* path);
extern "C" bool vfs_create(const char* path);
extern "C" bool vfs_write(const char* path, uint64_t offset, const void* data, uint64_t size);
extern "C" bool vfs_read(const char* path, uint64_t offset, void* data, uint64_t size, uint64_t* read_size);
extern "C" bool vfs_test();
