#pragma once

#include <stdint.h>
#include "idt.hpp"

static constexpr unsigned int SCHEDULER_MAX_CPUS = 256;
static constexpr unsigned int SCHEDULER_MAX_TASKS = 16;
static constexpr unsigned int SCHEDULER_QUANTUM_TICKS = 4;
static constexpr unsigned int SCHEDULER_TASK_STACK_SIZE = 16384;

using SchedulerThreadEntry = void (*)(void*);

enum SchedulerTaskState : uint32_t
{
    SCHEDULER_TASK_UNUSED = 0,
    SCHEDULER_TASK_READY = 1,
    SCHEDULER_TASK_RUNNING = 2,
    SCHEDULER_TASK_STOPPED = 3
};

struct SchedulerTask
{
    uint32_t id;
    uint32_t cpu;
    uint32_t state;
    uint32_t remaining_quantum;
    ExceptionFrame* saved_frame;
    SchedulerThreadEntry entry;
    void* argument;
    uint64_t stack_top;
};

struct SchedulerCpu
{
    uint32_t cpu_index;
    uint32_t current_task;
    uint32_t next_task;
    uint32_t task_count;
    uint64_t switches;
};

extern "C" bool scheduler_initialize(unsigned int cpu_count);
extern "C" bool scheduler_set_cpu_apic_ids(const unsigned int* apic_ids, unsigned int count);
extern "C" void scheduler_cpu_start(unsigned int cpu_index);
extern "C" bool scheduler_ready();
extern "C" bool scheduler_set_ready_for_kernel();
extern "C" unsigned int scheduler_current_cpu_index();
extern "C" bool scheduler_set_local_cpu_index(unsigned int cpu_index);
extern "C" ExceptionFrame* scheduler_timer_tick(unsigned int cpu_index, ExceptionFrame* current_frame);
extern "C" unsigned int scheduler_current_task(unsigned int cpu_index);
extern "C" unsigned int scheduler_task_count(unsigned int cpu_index);
extern "C" unsigned long long scheduler_switch_count(unsigned int cpu_index);
extern "C" bool scheduler_run_test();
extern "C" unsigned long long scheduler_task1_counter_get(unsigned int cpu_index);
extern "C" unsigned long long scheduler_take_bootstrap_stack();
extern "C" SchedulerThreadEntry scheduler_take_bootstrap_entry(void** argument);
extern "C" bool scheduler_create_kernel_thread(
    SchedulerThreadEntry entry, void* argument, unsigned int cpu_index,
    unsigned int* task_id);
extern "C" [[noreturn]] void scheduler_bootstrap_entry();
extern "C" bool scheduler_kernel_thread_test();
extern "C" [[noreturn]] void scheduler_thread_exit();
extern "C" bool scheduler_kernel_thread_exited(unsigned int cpu_index);
extern "C" unsigned long long scheduler_kernel_thread_counter(unsigned int cpu_index, unsigned int thread_index);
