#pragma once

#include <stdint.h>

struct IpcMessage
{
    uint32_t sender_pid;
    uint32_t code;
    uint64_t value;
};

extern "C" int ipc_create(uint32_t owner_pid);
extern "C" bool ipc_destroy(int endpoint, uint32_t owner_pid);
extern "C" void ipc_destroy_owner(uint32_t owner_pid);
extern "C" bool ipc_send(int endpoint, uint32_t sender_pid, uint32_t code, uint64_t value);
extern "C" bool ipc_receive(int endpoint, uint32_t receiver_pid, IpcMessage* message);
extern "C" bool ipc_test();
