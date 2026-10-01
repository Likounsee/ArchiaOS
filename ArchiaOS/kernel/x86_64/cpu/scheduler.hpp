#pragma once

#include <stdint.h>

static constexpr unsigned int SCHEDULER_MAX_CPUS = 256;
static constexpr unsigned int SCHEDULER_MAX_TASKS = 16;
static constexpr unsigned int SCHEDULER_QUANTUM_TICKS = 4;

struct SchedulerTask
{
    uint32_t id;
    uint32_t cpu;
    uint32_t state;
    uint32_t remaining_quantum;
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
extern "C" void scheduler_timer_tick(unsigned int cpu_index);
extern "C" unsigned int scheduler_current_task(unsigned int cpu_index);
extern "C" unsigned long long scheduler_switch_count(unsigned int cpu_index);
extern "C" bool scheduler_run_test();
