#include "vfs.hpp"

static constexpr unsigned int VFS_MAX_NODES = 128;
static constexpr unsigned int VFS_NAME_SIZE = 32;
static constexpr unsigned int VFS_FILE_CAPACITY = 4096;

struct VfsNode
{
    uint32_t type;
    uint32_t parent;
    uint64_t size;
    char name[VFS_NAME_SIZE];
    uint8_t data[VFS_FILE_CAPACITY];
};

static VfsNode nodes[VFS_MAX_NODES] = {};
static bool initialized = false;

static bool text_equal(const char* a, const char* b)
{
    unsigned int i = 0;
    while (a[i] && b[i] && a[i] == b[i])
        ++i;
    return a[i] == 0 && b[i] == 0;
}

static unsigned int text_length(const char* s)
{
    unsigned int n = 0;
    while (s && s[n])
        ++n;
    return n;
}

static unsigned int find_node(const char* path)
{
    if (!initialized || !path || path[0] != '/')
        return 0;

    if (path[1] == 0)
        return 0;

    unsigned int current = 0;
    unsigned int start = 1;

    while (path[start])
    {
        unsigned int end = start;
        while (path[end] && path[end] != '/')
            ++end;

        char component[VFS_NAME_SIZE] = {};
        const unsigned int length = end - start;
        if (length == 0 || length >= VFS_NAME_SIZE)
            return 0;

        for (unsigned int i = 0; i < length; ++i)
            component[i] = path[start + i];

        unsigned int next = 0;
        for (unsigned int i = 1; i < VFS_MAX_NODES; ++i)
        {
            if (nodes[i].type != VFS_NODE_UNUSED &&
                nodes[i].parent == current &&
                text_equal(nodes[i].name, component))
            {
                next = i;
                break;
            }
        }

        if (next == 0)
            return 0;

        current = next;
        start = path[end] ? end + 1 : end;
    }

    return current;
}

static bool split_parent(const char* path, unsigned int* parent, char* name)
{
    if (!path || !parent || !name || path[0] != '/')
        return false;

    unsigned int length = text_length(path);
    if (length < 2 || length >= 256)
        return false;

    int slash = -1;
    for (unsigned int i = 1; i < length; ++i)
        if (path[i] == '/')
            slash = static_cast<int>(i);

    const unsigned int name_start = slash < 0 ? 1 : static_cast<unsigned int>(slash + 1);
    const unsigned int name_length = length - name_start;
    if (name_length == 0 || name_length >= VFS_NAME_SIZE)
        return false;

    for (unsigned int i = 0; i < name_length; ++i)
        name[i] = path[name_start + i];
    name[name_length] = 0;

    if (slash < 0)
    {
        *parent = 0;
        return true;
    }

    char parent_path[256] = {};
    for (int i = 0; i < slash; ++i)
        parent_path[i] = path[i];
    parent_path[slash] = 0;
    if (parent_path[0] == 0)
        parent_path[0] = '/';

    *parent = find_node(parent_path);
    return nodes[*parent].type == VFS_NODE_DIRECTORY;
}

extern "C" bool vfs_initialize()
{
    for (unsigned int i = 0; i < VFS_MAX_NODES; ++i)
        nodes[i] = {};

    nodes[0].type = VFS_NODE_DIRECTORY;
    nodes[0].parent = 0;
    nodes[0].name[0] = '/';
    initialized = true;
    return true;
}

extern "C" bool vfs_mkdir(const char* path)
{
    unsigned int parent = 0;
    char name[VFS_NAME_SIZE] = {};
    if (!split_parent(path, &parent, name))
        return false;

    if (find_node(path) != 0)
        return false;

    for (unsigned int i = 1; i < VFS_MAX_NODES; ++i)
    {
        if (nodes[i].type == VFS_NODE_UNUSED)
        {
            nodes[i].type = VFS_NODE_DIRECTORY;
            nodes[i].parent = parent;
            for (unsigned int j = 0; j < VFS_NAME_SIZE; ++j)
                nodes[i].name[j] = name[j];
            return true;
        }
    }
    return false;
}

extern "C" bool vfs_create(const char* path)
{
    unsigned int parent = 0;
    char name[VFS_NAME_SIZE] = {};
    if (!split_parent(path, &parent, name))
        return false;

    if (find_node(path) != 0)
        return false;

    for (unsigned int i = 1; i < VFS_MAX_NODES; ++i)
    {
        if (nodes[i].type == VFS_NODE_UNUSED)
        {
            nodes[i].type = VFS_NODE_FILE;
            nodes[i].parent = parent;
            nodes[i].size = 0;
            for (unsigned int j = 0; j < VFS_NAME_SIZE; ++j)
                nodes[i].name[j] = name[j];
            return true;
        }
    }
    return false;
}

extern "C" bool vfs_unlink(const char* path)
{
    const unsigned int node = find_node(path);
    if (node == 0 || node >= VFS_MAX_NODES || nodes[node].type == VFS_NODE_UNUSED)
        return false;
    if (nodes[node].type == VFS_NODE_DIRECTORY)
    {
        for (unsigned int i = 1; i < VFS_MAX_NODES; ++i)
            if (nodes[i].type != VFS_NODE_UNUSED && nodes[i].parent == node)
                return false;
    }
    nodes[node] = {};
    return true;
}

extern "C" bool vfs_stat(const char* path, VfsStat* stat)
{
    if (!stat)
        return false;
    const unsigned int node = find_node(path);
    if (node == 0 && (!path || path[0] != '/' || path[1] != 0))
        return false;
    if (node >= VFS_MAX_NODES || nodes[node].type == VFS_NODE_UNUSED)
        return false;
    stat->type = nodes[node].type;
    stat->parent = nodes[node].parent;
    stat->size = nodes[node].size;
    return true;
}

extern "C" bool vfs_readdir(const char* path, uint32_t index, VfsDirEntry* entry)
{
    if (!entry)
        return false;
    const unsigned int node = find_node(path);
    if (node >= VFS_MAX_NODES || nodes[node].type != VFS_NODE_DIRECTORY)
        return false;
    uint32_t seen = 0;
    for (unsigned int i = 1; i < VFS_MAX_NODES; ++i)
    {
        if (nodes[i].type == VFS_NODE_UNUSED || nodes[i].parent != node)
            continue;
        if (seen++ != index)
            continue;
        entry->type = nodes[i].type;
        entry->size = nodes[i].size;
        for (unsigned int j = 0; j < sizeof(entry->name); ++j)
            entry->name[j] = nodes[i].name[j];
        return true;
    }
    return false;
}

extern "C" bool vfs_write(
    const char* path, uint64_t offset, const void* data, uint64_t size)
{
    const unsigned int node = find_node(path);
    if (node == 0 || nodes[node].type != VFS_NODE_FILE ||
        !data || offset > VFS_FILE_CAPACITY ||
        size > VFS_FILE_CAPACITY - offset)
        return false;

    const auto* source = reinterpret_cast<const uint8_t*>(data);
    for (uint64_t i = 0; i < size; ++i)
        nodes[node].data[offset + i] = source[i];

    if (offset + size > nodes[node].size)
        nodes[node].size = offset + size;
    return true;
}

extern "C" bool vfs_read(
    const char* path, uint64_t offset, void* data, uint64_t size,
    uint64_t* read_size)
{
    if (read_size)
        *read_size = 0;

    const unsigned int node = find_node(path);
    if (node == 0 || nodes[node].type != VFS_NODE_FILE ||
        !data || offset > nodes[node].size)
        return false;

    const uint64_t available = nodes[node].size - offset;
    const uint64_t count = size < available ? size : available;
    auto* destination = reinterpret_cast<uint8_t*>(data);
    for (uint64_t i = 0; i < count; ++i)
        destination[i] = nodes[node].data[offset + i];

    if (read_size)
        *read_size = count;
    return true;
}

extern "C" bool vfs_test()
{
    if (!vfs_initialize() ||
        !vfs_mkdir("/system") ||
        !vfs_create("/system/hello"))
        return false;

    static const char message[] = "ArchiaOS VFS";
    char buffer[sizeof(message)] = {};
    uint64_t read_size = 0;

    if (!vfs_write("/system/hello", 0, message, sizeof(message)) ||
        !vfs_read("/system/hello", 0, buffer, sizeof(buffer), &read_size) ||
        read_size != sizeof(message))
        return false;

    for (unsigned int i = 0; i < sizeof(message); ++i)
        if (buffer[i] != message[i])
            return false;

    VfsStat stat{};
    VfsDirEntry entry{};
    if (!vfs_stat("/system/hello", &stat) ||
        stat.type != VFS_NODE_FILE || stat.size != sizeof(message) ||
        !vfs_readdir("/system", 0, &entry) ||
        entry.type != VFS_NODE_FILE || !text_equal(entry.name, "hello") ||
        entry.size != sizeof(message) ||
        vfs_readdir("/system", 1, &entry) ||
        !vfs_unlink("/system/hello") ||
        vfs_stat("/system/hello", &stat) ||
        vfs_read("/system/hello", 0, buffer, sizeof(buffer), &read_size))
        return false;

    return true;
}
