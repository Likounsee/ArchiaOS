#include "vfs.hpp"
#include "openfs_adapter.hpp"
#include "openfs/crc32c.h"
#include "openfs/file.h"
#include "openfs/inode.h"
#include "openfs/path.h"
#include "openfs/mount.h"
#include "../drivers/block.hpp"

static openfs_block_device_t openfs_device{};
static openfs_mount_t openfs_mount_state{};
static bool initialized = false;
static uint32_t vfs_test_stage_value = 0U;
static void vfs_zero_buffer(char* buffer, unsigned int size) {
    for (unsigned int i = 0U; i < size; ++i)
        buffer[i] = 0;
}


static bool inode_count(uint64_t* count)
{
    if (!count || openfs_mount_state.superblock.block_size == 0U)
        return false;
    const uint64_t blocks = openfs_mount_state.superblock.inode_table_blocks;
    if (blocks > UINT64_MAX / openfs_mount_state.superblock.block_size)
        return false;
    const uint64_t bytes = blocks * static_cast<uint64_t>(openfs_mount_state.superblock.block_size);
    *count = bytes / OPENFS_INODE_SIZE;
    return *count != 0U;
}

static bool lookup_inode(const char* path, openfs_inode_t* inode)
{
    if (!initialized || !path || !inode)
        return false;

    uint64_t number = 0U;
    if (openfs_path_lookup_follow(
            &openfs_device, &openfs_mount_state.superblock, path, &number) != OPENFS_PATH_OK)
        return false;

    uint64_t count = 0U;
    if (!inode_count(&count))
        return false;

    return openfs_inode_read(
        &openfs_device,
        openfs_mount_state.superblock.inode_table_start,
        number,
        count,
        inode) == OPENFS_INODE_OK;
}

static uint32_t vfs_type_from_mode(uint32_t mode)
{
    if ((mode & OPENFS_INODE_TYPE_MASK) == OPENFS_INODE_MODE_DIRECTORY)
        return VFS_NODE_DIRECTORY;
    return VFS_NODE_FILE;
}

static bool decode_directory_entry(
    const uint8_t* raw,
    char* name,
    openfs_dir_entry_t* entry)
{
    if (!raw || !name || !entry)
        return false;

    bool empty = true;
    for (unsigned int i = 0U; i < OPENFS_DIR_ENTRY_SIZE; ++i)
    {
        if (raw[i] != 0U)
        {
            empty = false;
            break;
        }
    }
    if (empty)
        return false;

    static const char magic[] = "ODIR1";
    for (unsigned int i = 0U; i < 5U; ++i)
        if (raw[i] != static_cast<uint8_t>(magic[i]))
            return false;

    const uint32_t stored =
        static_cast<uint32_t>(raw[252U]) |
        (static_cast<uint32_t>(raw[253U]) << 8U) |
        (static_cast<uint32_t>(raw[254U]) << 16U) |
        (static_cast<uint32_t>(raw[255U]) << 24U);
    if (stored != openfs_crc32c(raw, 252U))
        return false;

    const unsigned int length = raw[7U];
    if (length == 0U || length > OPENFS_DIR_NAME_MAX)
        return false;

    entry->inode_number = 0U;
    entry->generation = 0U;
    for (unsigned int k = 0U; k < 8U; ++k)
    {
        entry->inode_number |= static_cast<uint64_t>(raw[8U + k]) << (8U * k);
        entry->generation |= static_cast<uint64_t>(raw[16U + k]) << (8U * k);
    }
    entry->type = raw[6U];

    if (entry->inode_number == 0U || entry->generation == 0U ||
        (entry->type != 1U && entry->type != 2U && entry->type != 3U))
        return false;

    for (unsigned int i = 24U + length; i < 252U; ++i)
        if (raw[i] != 0U)
            return false;

    for (unsigned int i = 0U; i < length; ++i)
        name[i] = static_cast<char>(raw[24U + i]);
    name[length] = 0;

    for (unsigned int i = 0U; i < length; ++i)
        if (name[i] == '/' || name[i] == 0)
            return false;

    return true;
}

extern "C" bool vfs_initialize()
{
    if (initialized)
        return true;

    const unsigned int device_count = block_device_count();
    for (unsigned int index = 1U; index <= device_count; ++index)
    {
        const BlockDevice* disk = block_get(index);
        if (!disk)
            continue;

        if (!openfs_kernel_attach(disk, 0U, 4096U, &openfs_device))
            continue;

        if (openfs_mount(&openfs_mount_state, &openfs_device) == OPENFS_MOUNT_OK)
        {
            initialized = true;
            return true;
        }

        openfs_kernel_detach(&openfs_device);
    }

    return false;
}

extern "C" bool vfs_sync()
{
    if (!initialized)
        return false;
    const bool ok = openfs_sync(&openfs_mount_state) == OPENFS_MOUNT_OK;
    if (!ok)
        vfs_test_stage_value = 13U;
    return ok;
}

extern "C" bool vfs_shutdown()
{
    if (!initialized)
        return true;

    if (openfs_unmount(&openfs_mount_state) != OPENFS_MOUNT_OK)
        return false;

    openfs_kernel_detach(&openfs_device);
    initialized = false;
    return true;
}

extern "C" bool vfs_mkdir(const char* path)
{
    if (!initialized || !path)
        return false;

    uint64_t inode = 0U;
    return openfs_path_mkdir(
        &openfs_device, &openfs_mount_state.superblock, path, &inode) == OPENFS_PATH_OK;
}

extern "C" bool vfs_create(const char* path)
{
    if (!initialized || !path)
        return false;

    uint64_t inode = 0U;
    return openfs_path_create(
        &openfs_device, &openfs_mount_state.superblock, path, 0100644U, &inode) == OPENFS_PATH_OK;
}

extern "C" bool vfs_unlink(const char* path)
{
    if (!initialized || !path || path[0] == 0 || (path[0] == '/' && path[1] == 0))
        return false;

    return openfs_path_unlink(
        &openfs_device, &openfs_mount_state.superblock, path) == OPENFS_PATH_OK;
}

extern "C" bool vfs_stat(const char* path, VfsStat* stat)
{
    if (!stat)
        return false;

    openfs_inode_t inode{};
    if (!lookup_inode(path, &inode))
        return false;

    stat->type = vfs_type_from_mode(inode.mode);
    stat->parent = static_cast<uint32_t>(inode.parent_inode);
    stat->size = inode.size;
    return true;
}

extern "C" bool vfs_readdir(const char* path, uint32_t index, VfsDirEntry* entry)
{
    if (!entry || !initialized) {
        vfs_test_stage_value = 70U;
        return false;
    }

    openfs_inode_t directory{};
    if (!lookup_inode(path, &directory)) {
        vfs_test_stage_value = 71U;
        return false;
    }
    if ((directory.mode & OPENFS_INODE_TYPE_MASK) != OPENFS_INODE_MODE_DIRECTORY) {
        vfs_test_stage_value = 72U;
        return false;
    }
    if (directory.size % OPENFS_DIR_ENTRY_SIZE != 0U) {
        vfs_test_stage_value = 73U;
        return false;
    }

    const uint64_t entries = directory.size / OPENFS_DIR_ENTRY_SIZE;
    uint32_t visible = 0U;

    for (uint64_t n = 0U; n < entries; ++n)
    {
        if (n > UINT64_MAX / OPENFS_DIR_ENTRY_SIZE)
            return false;

        uint8_t raw[OPENFS_DIR_ENTRY_SIZE]{};
        size_t got = 0U;
        const openfs_file_result_t dir_read = openfs_file_read(
            &openfs_device,
            &openfs_mount_state.superblock,
            &directory,
            n * OPENFS_DIR_ENTRY_SIZE,
            raw,
            sizeof(raw),
            &got);
        if (dir_read != OPENFS_FILE_OK || got != sizeof(raw)) {
            vfs_test_stage_value = 74U;
            return false;
        }

        char name[OPENFS_DIR_NAME_MAX + 1U]{};
        openfs_dir_entry_t dir_entry{};
        if (!decode_directory_entry(raw, name, &dir_entry))
            continue;

        if (visible++ != index)
            continue;

        entry->type = dir_entry.type == 2U ? VFS_NODE_DIRECTORY : VFS_NODE_FILE;
        entry->size = 0U;

        openfs_inode_t child{};
        uint64_t count = 0U;
        if (!inode_count(&count) ||
            openfs_inode_read(
                &openfs_device,
                openfs_mount_state.superblock.inode_table_start,
                dir_entry.inode_number,
                count,
                &child) != OPENFS_INODE_OK) {
            vfs_test_stage_value = 75U;
            return false;
        }

        if (child.generation != dir_entry.generation) {
            vfs_test_stage_value = 76U;
            return false;
        }

        const uint32_t child_mode = child.mode & OPENFS_INODE_TYPE_MASK;
        const uint8_t expected_type =
            child_mode == OPENFS_INODE_MODE_DIRECTORY ? 2U :
            child_mode == OPENFS_INODE_MODE_SYMLINK ? 3U :
            child_mode == OPENFS_INODE_MODE_REGULAR ? 1U : 0U;
        if (expected_type == 0U || expected_type != dir_entry.type) {
            vfs_test_stage_value = 77U;
            return false;
        }

        entry->size = child.size;

        unsigned int i = 0U;
        for (; i + 1U < sizeof(entry->name) && name[i]; ++i)
            entry->name[i] = name[i];
        entry->name[i] = 0;
        return true;
    }

    return false;
}

extern "C" bool vfs_write(
    const char* path, uint64_t offset, const void* data, uint64_t size)
{
    if (!initialized || !data || size > static_cast<uint64_t>(SIZE_MAX))
        return false;

    openfs_inode_t inode{};
    if (!lookup_inode(path, &inode) ||
        (inode.mode & OPENFS_INODE_TYPE_MASK) != OPENFS_INODE_MODE_REGULAR)
        return false;

    return openfs_file_write(
        &openfs_device,
        &openfs_mount_state.superblock,
        &inode,
        offset,
        data,
        static_cast<size_t>(size)) == OPENFS_FILE_OK;
}

extern "C" bool vfs_read(
    const char* path, uint64_t offset, void* data, uint64_t size,
    uint64_t* read_size)
{
    if (read_size)
        *read_size = 0U;
    if (!initialized || !data || size > static_cast<uint64_t>(SIZE_MAX))
        return false;

    openfs_inode_t inode{};
    if (!lookup_inode(path, &inode) ||
        (inode.mode & OPENFS_INODE_TYPE_MASK) != OPENFS_INODE_MODE_REGULAR)
        return false;

    size_t got = 0U;
    if (openfs_file_read(
            &openfs_device,
            &openfs_mount_state.superblock,
            &inode,
            offset,
            data,
            static_cast<size_t>(size),
            &got) != OPENFS_FILE_OK)
        return false;

    if (read_size)
        *read_size = got;
    return true;
}

extern "C" uint32_t vfs_test_stage()
{
    return vfs_test_stage_value;
}

extern "C" bool vfs_test()
{
    vfs_test_stage_value = 0U;
    if (!vfs_initialize())
    {
        vfs_test_stage_value = 1U;
        return false;
    }

    if (!vfs_mkdir("/vfs-test") ||
        !vfs_create("/vfs-test/hello"))
    {
        vfs_test_stage_value = 2U;
        return false;
    }

    static const char message[] = "ArchiaOS OpenFS VFS";
    static uint8_t large_message[9000];
    static uint8_t large_buffer[9000];
    char buffer[sizeof(message)]{};
    uint64_t read_size = 0U;

    for (unsigned int i = 0U; i < sizeof(large_message); ++i)
        large_message[i] = static_cast<uint8_t>((i * 37U) ^ (i >> 3U));

    if (!vfs_write("/vfs-test/hello", 0U, message, sizeof(message)) ||
        !vfs_read("/vfs-test/hello", 0U, buffer, sizeof(buffer), &read_size) ||
        read_size != sizeof(message) ||
        !vfs_create("/vfs-test/large") ||
        !vfs_write("/vfs-test/large", 0U, large_message, sizeof(large_message)) ||
        !vfs_read("/vfs-test/large", 0U, large_buffer, sizeof(large_buffer), &read_size) ||
        read_size != sizeof(large_message))
        return false;

    for (unsigned int i = 0U; i < sizeof(large_message); ++i)
        if (large_buffer[i] != large_message[i])
        {
            vfs_test_stage_value = 4U;
            return false;
        }

    for (unsigned int i = 0U; i < sizeof(message); ++i)
        if (buffer[i] != message[i])
        {
            vfs_test_stage_value = 5U;
            return false;
        }

    VfsStat stat{};
    VfsDirEntry entry{};
    if (!vfs_stat("/vfs-test/hello", &stat) ||
        stat.type != VFS_NODE_FILE ||
        stat.size != sizeof(message))
    {
        vfs_test_stage_value = 61U;
        return false;
    }

    bool found_hello = false;
    bool found_large = false;
    for (uint32_t index = 0U; index < 3U; ++index)
    {
        if (!vfs_readdir("/vfs-test", index, &entry))
        {
            if (index != 2U)
            {
                vfs_test_stage_value = 62U;
                return false;
            }
            break;
        }

        if (entry.name[0] == 'h' && entry.name[1] == 'e' &&
            entry.name[2] == 'l' && entry.name[3] == 'l' &&
            entry.name[4] == 'o' && entry.name[5] == 0)
        {
            if (found_hello || entry.type != VFS_NODE_FILE ||
                entry.size != sizeof(message))
            {
                vfs_test_stage_value = 63U;
                return false;
            }
            found_hello = true;
        }
        else if (entry.name[0] == 'l' && entry.name[1] == 'a' &&
                 entry.name[2] == 'r' && entry.name[3] == 'g' &&
                 entry.name[4] == 'e' && entry.name[5] == 0)
        {
            if (found_large || entry.type != VFS_NODE_FILE ||
                entry.size != sizeof(large_message))
            {
                vfs_test_stage_value = 64U;
                return false;
            }
            found_large = true;
        }
        else
        {
            vfs_test_stage_value = 65U;
            return false;
        }
    }

    if (!found_hello || !found_large)
    {
        vfs_test_stage_value = 66U;
        return false;
    }

    if (!vfs_sync() || !vfs_shutdown())
    {
        vfs_test_stage_value = 7U;
        return false;
    }

    if (!vfs_initialize())
    {
        vfs_test_stage_value = 8U;
        return false;
    }

    vfs_zero_buffer(buffer, sizeof(buffer));
    read_size = 0U;
    if (!vfs_read("/vfs-test/hello", 0U, buffer, sizeof(buffer), &read_size) ||
        read_size != sizeof(message) ||
        !vfs_read("/vfs-test/large", 0U, large_buffer, sizeof(large_buffer), &read_size) ||
        read_size != sizeof(large_message))
    {
        vfs_test_stage_value = 9U;
        return false;
    }
    for (unsigned int i = 0U; i < sizeof(message); ++i)
        if (buffer[i] != message[i])
        {
            vfs_test_stage_value = 10U;
            return false;
        }
    for (unsigned int i = 0U; i < sizeof(large_message); ++i)
        if (large_buffer[i] != large_message[i])
        {
            vfs_test_stage_value = 11U;
            return false;
        }

    if (!vfs_unlink("/vfs-test/large") ||
        !vfs_unlink("/vfs-test/hello") ||
        !vfs_unlink("/vfs-test") ||
        vfs_stat("/vfs-test/hello", &stat))
    {
        vfs_test_stage_value = 12U;
        return false;
    }

    return openfs_sync(&openfs_mount_state) == OPENFS_MOUNT_OK;
}
