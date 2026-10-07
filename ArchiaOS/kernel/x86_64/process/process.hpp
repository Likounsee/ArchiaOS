#pragma once

#include <stdint.h>
#include "address_space.hpp"

struct ExceptionFrame;

struct Process
{
    uint32_t pid;
    uint32_t state;
    AddressSpace address_space;
    uint64_t entry;
    uint64_t user_stack_top;
};

enum ProcessState : uint32_t
{
    PROCESS_UNUSED = 0,
    PROCESS_READY = 1,
    PROCESS_RUNNING = 2,
    PROCESS_EXITED = 3
};

extern "C" bool process_create_elf(
    Process* process, const uint8_t* image, uint64_t image_size);
extern "C" bool process_register(Process* process);
extern "C" Process* process_find(uint32_t pid);
extern "C" bool process_unregister(Process* process);
extern "C" bool process_destroy(Process* process);
extern "C" bool process_activate(Process* process);
extern "C" uint32_t process_current_pid();
extern "C" unsigned long long process_syscall_count();
extern "C" bool process_handle_syscall(ExceptionFrame* frame);
extern "C" bool process_ipc_user_ok();
extern "C" bool process_run_ring3_test();
