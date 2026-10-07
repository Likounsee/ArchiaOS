#include "vfs.hpp"
#include "openfs_adapter.hpp"
#include "openfs/crc32c.h"
#include "openfs/file.h"
#include "openfs/inode.h"
#include "openfs/path.h"
#include "openfs/mount.h"
#include "../drivers/block.hpp"
#include "gpt.hpp"
extern "C" volatile uint32_t openfs_debug_stored;
extern "C" volatile uint32_t openfs_debug_calculated;
extern "C" volatile uint32_t openfs_debug_reason;

static openfs_block_device_t openfs_device{};
static openfs_mount_t openfs_mount_state{};
static bool initialized = false;


static void vfs_test_debug(const char* s)
{
    for (int i = 0; s[i] != '\0'; ++i)
        asm volatile("outb %0,%1"
                     : : "a"(s[i]),
                         "Nd"(static_cast<unsigned short>(0xE9)));
}

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
    vfs_test_debug("VFS INIT: DEVICES ");
    bool has_gpt_partition = false;

    for (unsigned int index = 1U; index <= device_count; ++index)
    {
        const BlockDevice* disk = block_get(index);
        if (disk && disk->type == BLOCK_DEVICE_PARTITION)
            has_gpt_partition = true;
    }

    vfs_test_debug(has_gpt_partition ? "VFS INIT: GPT PARTITION\n" : "VFS INIT: NO GPT PARTITION\n");

    /* A discovered GPT layout is authoritative: only the explicit
       ArchiaOS/OpenFS system partition may become the VFS root.
       Raw-disk fallback is retained only for non-partitioned test media. */
    for (unsigned int pass = 0U; pass < 2U; ++pass)
    {
        for (unsigned int index = 1U; index <= device_count; ++index)
        {
            const BlockDevice* disk = block_get(index);
            if (!disk)
                continue;

            const bool is_partition = disk->type == BLOCK_DEVICE_PARTITION;
            if (pass == 0U)
            {
                if (!is_partition || !gpt_partition_is_archiaos_system(disk))
                    continue;
            }
            else
            {
                if (is_partition || has_gpt_partition)
                    continue;
            }

            if (!openfs_kernel_attach(disk, 0U, 4096U, &openfs_device))
            {
                vfs_test_debug("VFS INIT: ATTACH FAIL\n");
                continue;
            }
            vfs_test_debug("VFS INIT: ATTACH OK\n");

            const openfs_format_result_t sr = openfs_read_superblock(&openfs_device, &openfs_mount_state.superblock);
            vfs_test_debug("VFS INIT: SB blocks="); { const char hh[]="0123456789ABCDEF"; for(int sh=60;sh>=0;sh-=4) asm volatile("outb %0,%1"::"a"(hh[(openfs_mount_state.superblock.total_blocks>>sh)&15U]),"Nd"(static_cast<unsigned short>(0xE9))); asm volatile("outb %0,%1"::"a"((char)32),"Nd"(static_cast<unsigned short>(0xE9))); for(int sh=28;sh>=0;sh-=4) asm volatile("outb %0,%1"::"a"(hh[(openfs_mount_state.superblock.block_size>>sh)&15U]),"Nd"(static_cast<unsigned short>(0xE9))); asm volatile("outb %0,%1"::"a"((char)10),"Nd"(static_cast<unsigned short>(0xE9))); }
            vfs_test_debug("VFS INIT: SUPER=");
            const char sd[]="0123456789ABCDEF"; const unsigned int sv=static_cast<unsigned int>(sr);
            for(int sh=4;sh>=0;sh-=4) asm volatile("outb %0,%1" : : "a"(sd[(sv>>sh)&15U]), "Nd"(static_cast<unsigned short>(0xE9)));
            asm volatile("outb %0,%1" : : "a"(static_cast<char>(10)), "Nd"(static_cast<unsigned short>(0xE9)));
            const openfs_mount_result_t mount_result = openfs_mount(&openfs_mount_state, &openfs_device);
            if (mount_result == OPENFS_MOUNT_OK)
            {
                initialized = true;
                return true;
            }

            vfs_test_debug("VFS INIT: CRC ");
            const char hx[]="0123456789ABCDEF"; for(int sh=28;sh>=0;sh-=4) asm volatile("outb %0,%1"::"a"(hx[(openfs_debug_stored>>sh)&15U]),"Nd"(static_cast<unsigned short>(0xE9))); asm volatile("outb %0,%1"::"a"((char)32),"Nd"(static_cast<unsigned short>(0xE9))); for(int sh=28;sh>=0;sh-=4) asm volatile("outb %0,%1"::"a"(hx[(openfs_debug_calculated>>sh)&15U]),"Nd"(static_cast<unsigned short>(0xE9))); asm volatile("outb %0,%1"::"a"((char)10),"Nd"(static_cast<unsigned short>(0xE9)));
            vfs_test_debug("VFS INIT: MOUNT FAIL CODE=");
            vfs_test_debug("VFS INIT: REASON="); for(int sh=4;sh>=0;sh-=4) asm volatile("outb %0,%1"::"a"(hx[(openfs_debug_reason>>sh)&15U]),"Nd"(static_cast<unsigned short>(0xE9))); asm volatile("outb %0,%1"::"a"((char)10),"Nd"(static_cast<unsigned short>(0xE9)));
            const unsigned int mr=static_cast<unsigned int>(mount_result);
            const char md[]="0123456789ABCDEF";
            for(int sh=4;sh>=0;sh-=4) asm volatile("outb %0,%1" : : "a"(md[(mr>>sh)&15U]), "Nd"(static_cast<unsigned short>(0xE9)));
            asm volatile("outb %0,%1" : : "a"(static_cast<char>(10)), "Nd"(static_cast<unsigned short>(0xE9)));
            openfs_kernel_detach(&openfs_device);
        }
    }

    return false;
}

[[maybe_unused]] static void vfs_debug_hex(unsigned int value)
{
    static const char digits[]="0123456789ABCDEF";
    for (const char* p="VFS VALUE 0x"; *p; ++p) asm volatile("outb %0,%1" : : "a"(*p), "Nd"(static_cast<unsigned short>(0xE9)));
    for (int shift=28; shift>=0; shift-=4)
        asm volatile("outb %0,%1" : : "a"(digits[(value>>shift)&0xFU]), "Nd"(static_cast<unsigned short>(0xE9)));
    asm volatile("outb %0,%1" : : "a"(static_cast<char>(10)), "Nd"(static_cast<unsigned short>(0xE9)));
}

extern "C" bool vfs_sync()
{
    if (!initialized)
        return false;
    const bool ok = openfs_sync(&openfs_mount_state) == OPENFS_MOUNT_OK;
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
        return false;
    }

    openfs_inode_t directory{};
    if (!lookup_inode(path, &directory)) {
        return false;
    }
    if ((directory.mode & OPENFS_INODE_TYPE_MASK) != OPENFS_INODE_MODE_DIRECTORY) {
        return false;
    }
    if (directory.size % OPENFS_DIR_ENTRY_SIZE != 0U) {
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
            return false;
        }

        if (child.generation != dir_entry.generation) {
            return false;
        }

        const uint32_t child_mode = child.mode & OPENFS_INODE_TYPE_MASK;
        const uint8_t expected_type =
            child_mode == OPENFS_INODE_MODE_DIRECTORY ? 2U :
            child_mode == OPENFS_INODE_MODE_SYMLINK ? 3U :
            child_mode == OPENFS_INODE_MODE_REGULAR ? 1U : 0U;
        if (expected_type == 0U || expected_type != dir_entry.type) {
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

extern "C" bool vfs_test()
{
    vfs_test_debug("VFS TEST: START\n");
    if (!vfs_initialize())
    {
        return false;
    }

    vfs_test_debug("VFS TEST: INIT OK\n");
    if (!vfs_mkdir("/vfs-test") ||
        !vfs_create("/vfs-test/hello"))
    {
        return false;
    }

    vfs_test_debug("VFS TEST: CREATE OK\n");
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
            return false;
        }

    vfs_test_debug("VFS TEST: PERSISTED IO OK\n");
    for (unsigned int i = 0U; i < sizeof(message); ++i)
        if (buffer[i] != message[i])
        {
            return false;
        }

    vfs_test_debug("VFS TEST: IO OK\n");
    VfsStat stat{};
    VfsDirEntry entry{};
    if (!vfs_stat("/vfs-test/hello", &stat) ||
        stat.type != VFS_NODE_FILE ||
        stat.size != sizeof(message))
    {
        return false;
    }

    vfs_test_debug("VFS TEST: STAT OK\n");
    bool found_hello = false;
    bool found_large = false;
    for (uint32_t index = 0U; index < 3U; ++index)
    {
        if (!vfs_readdir("/vfs-test", index, &entry))
        {
            if (index != 2U)
            {
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
                return false;
            }
            found_large = true;
        }
        else
        {
            return false;
        }
    }

    vfs_test_debug("VFS TEST: READDIR OK\n");
    if (!found_hello || !found_large)
    {
        return false;
    }

    vfs_test_debug("VFS TEST: PRE-SHUTDOWN OK\n");
    if (!vfs_sync() || !vfs_shutdown())
    {
        return false;
    }

    vfs_test_debug("VFS TEST: SHUTDOWN OK\n");
    if (!vfs_initialize())
    {
        return false;
    }

    vfs_test_debug("VFS TEST: REINIT OK\n");
    vfs_zero_buffer(buffer, sizeof(buffer));
    read_size = 0U;
    if (!vfs_read("/vfs-test/hello", 0U, buffer, sizeof(buffer), &read_size) ||
        read_size != sizeof(message) ||
        !vfs_read("/vfs-test/large", 0U, large_buffer, sizeof(large_buffer), &read_size) ||
        read_size != sizeof(large_message))
    {
        return false;
    }
    for (unsigned int i = 0U; i < sizeof(message); ++i)
        if (buffer[i] != message[i])
        {
            return false;
        }
    for (unsigned int i = 0U; i < sizeof(large_message); ++i)
        if (large_buffer[i] != large_message[i])
        {
            return false;
        }

    vfs_test_debug("VFS TEST: PERSISTENCE VERIFIED\n");
    if (!vfs_unlink("/vfs-test/large") ||
        !vfs_unlink("/vfs-test/hello") ||
        !vfs_unlink("/vfs-test") ||
        vfs_stat("/vfs-test/hello", &stat))
    {
        return false;
    }

    vfs_test_debug("VFS TEST: UNLINK OK\n");
    const bool result = openfs_sync(&openfs_mount_state) == OPENFS_MOUNT_OK;
    vfs_test_debug(result ? "VFS TEST: PASS\n" : "VFS TEST: FINAL SYNC FAIL\n");
    return result;
}
