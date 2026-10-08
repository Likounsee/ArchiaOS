#include "ipc.hpp"

static constexpr unsigned int IPC_MAX_ENDPOINTS = 32;
static constexpr unsigned int IPC_QUEUE_DEPTH = 32;

struct IpcEndpoint
{
    bool used;
    uint32_t owner_pid;
    uint32_t head;
    uint32_t tail;
    uint32_t count;
    IpcMessage queue[IPC_QUEUE_DEPTH];
};

static IpcEndpoint endpoints[IPC_MAX_ENDPOINTS] = {};
static volatile unsigned char ipc_lock = 0;

static inline uint64_t ipc_enter_critical()
{
    uint64_t flags;
    asm volatile("pushfq; popq %0; cli" : "=r"(flags) : : "memory");
    while (__atomic_test_and_set(&ipc_lock, __ATOMIC_ACQUIRE))
        asm volatile("pause" : : : "memory");
    return flags;
}

static inline void ipc_leave_critical(uint64_t flags)
{
    if (flags & (1ULL << 9))
        asm volatile("sti" : : : "memory");
}

extern "C" int ipc_create(uint32_t owner_pid)
{
    if (!owner_pid)
        return -1;

    const uint64_t flags = ipc_enter_critical();
    for (unsigned int i = 0; i < IPC_MAX_ENDPOINTS; ++i)
    {
        if (!endpoints[i].used)
        {
            endpoints[i] = {};
            endpoints[i].used = true;
            endpoints[i].owner_pid = owner_pid;
            ipc_leave_critical(flags);
            return static_cast<int>(i);
        }
    }
    ipc_leave_critical(flags);
    return -1;
}

extern "C" bool ipc_destroy(int endpoint, uint32_t owner_pid)
{
    if (endpoint < 0 || endpoint >= static_cast<int>(IPC_MAX_ENDPOINTS))
        return false;

    const uint64_t flags = ipc_enter_critical();
    IpcEndpoint& e = endpoints[endpoint];
    const bool ok = e.used && e.owner_pid == owner_pid;
    if (ok)
        e = {};
    ipc_leave_critical(flags);
    return ok;
}

extern "C" bool ipc_send(
    int endpoint, uint32_t sender_pid, uint32_t code, uint64_t value)
{
    if (!sender_pid || endpoint < 0 || endpoint >= static_cast<int>(IPC_MAX_ENDPOINTS))
        return false;

    const uint64_t flags = ipc_enter_critical();
    IpcEndpoint& e = endpoints[endpoint];
    if (!e.used || e.count == IPC_QUEUE_DEPTH)
    {
        ipc_leave_critical(flags);
        return false;
    }

    e.queue[e.head] = {sender_pid, code, value};
    e.head = (e.head + 1) % IPC_QUEUE_DEPTH;
    ++e.count;
    ipc_leave_critical(flags);
    return true;
}

extern "C" bool ipc_receive(
    int endpoint, uint32_t receiver_pid, IpcMessage* message)
{
    if (endpoint < 0 || endpoint >= static_cast<int>(IPC_MAX_ENDPOINTS) ||
        !message)
        return false;

    const uint64_t flags = ipc_enter_critical();
    IpcEndpoint& e = endpoints[endpoint];
    if (!e.used || e.owner_pid != receiver_pid || e.count == 0)
    {
        ipc_leave_critical(flags);
        return false;
    }

    *message = e.queue[e.tail];
    e.tail = (e.tail + 1) % IPC_QUEUE_DEPTH;
    --e.count;
    ipc_leave_critical(flags);
    return true;
}

extern "C" void ipc_destroy_owner(uint32_t owner_pid)
{
    if (!owner_pid)
        return;

    const uint64_t flags = ipc_enter_critical();
    for (unsigned int i = 0; i < IPC_MAX_ENDPOINTS; ++i)
    {
        if (endpoints[i].used && endpoints[i].owner_pid == owner_pid)
            endpoints[i] = {};
    }
    ipc_leave_critical(flags);
}

extern "C" bool ipc_test()
{
    if (ipc_create(0) >= 0)
        return false;

    const int endpoint = ipc_create(1);
    if (endpoint < 0)
        return false;

    IpcMessage unauthorized{};
    if (ipc_receive(endpoint, 42, &unauthorized) ||
        ipc_destroy(endpoint, 42))
        return false;

    if (ipc_send(endpoint, 0, 0x1234U, 0x1122334455667788ULL))
        return false;

    const bool sent = ipc_send(endpoint, 42, 0x1234U, 0x1122334455667788ULL);
    IpcMessage message{};
    const bool received = ipc_receive(endpoint, 1, &message);
    if (!sent || !received ||
        message.sender_pid != 42 ||
        message.code != 0x1234U ||
        message.value != 0x1122334455667788ULL)
        return false;

    for (unsigned int i = 0; i < IPC_QUEUE_DEPTH; ++i)
        if (!ipc_send(endpoint, 42, i, i))
            return false;
    if (ipc_send(endpoint, 42, 0xFFFFU, 0))
        return false;

    for (unsigned int i = 0; i < IPC_QUEUE_DEPTH; ++i)
    {
        IpcMessage queued{};
        if (!ipc_receive(endpoint, 1, &queued) ||
            queued.code != i ||
            queued.value != i)
            return false;
    }

    int extra_endpoints[IPC_MAX_ENDPOINTS - 1] = {};
    for (unsigned int i = 0; i < IPC_MAX_ENDPOINTS - 1; ++i)
    {
        extra_endpoints[i] = ipc_create(1);
        if (extra_endpoints[i] < 0)
            return false;
    }
    if (ipc_create(1) >= 0)
        return false;

    for (unsigned int i = 0; i < IPC_MAX_ENDPOINTS - 1; ++i)
        if (!ipc_destroy(extra_endpoints[i], 1))
            return false;

    return ipc_destroy(endpoint, 1);
}
