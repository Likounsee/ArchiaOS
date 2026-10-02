#include "scheduler.hpp"

static SchedulerCpu cpus[SCHEDULER_MAX_CPUS];
static SchedulerTask tasks[SCHEDULER_MAX_CPUS][SCHEDULER_MAX_TASKS];
static unsigned int scheduler_cpu_count = 1;

extern "C" bool scheduler_initialize(unsigned int cpu_count)
{
    if (cpu_count == 0 || cpu_count > SCHEDULER_MAX_CPUS)
        return false;

    scheduler_cpu_count = cpu_count;
    for (unsigned int cpu = 0; cpu < cpu_count; ++cpu)
    {
        cpus[cpu] = SchedulerCpu{cpu, 0, 1, 2, 0};
        tasks[cpu][0] = SchedulerTask{0, cpu, 1, SCHEDULER_QUANTUM_TICKS};
        tasks[cpu][1] = SchedulerTask{1, cpu, 1, SCHEDULER_QUANTUM_TICKS};
    }
    return true;
}

extern "C" void scheduler_timer_tick(unsigned int cpu_index)
{
    if (cpu_index >= scheduler_cpu_count)
        return;

    SchedulerCpu& cpu = cpus[cpu_index];
    SchedulerTask& current = tasks[cpu_index][cpu.current_task];

    if (current.remaining_quantum > 0)
        --current.remaining_quantum;

    if (current.remaining_quantum == 0)
    {
        const unsigned int old = cpu.current_task;
        cpu.current_task = cpu.next_task;
        cpu.next_task = old;
        tasks[cpu_index][cpu.current_task].remaining_quantum = SCHEDULER_QUANTUM_TICKS;
        ++cpu.switches;
    }
}

extern "C" unsigned int scheduler_current_task(unsigned int cpu_index)
{
    if (cpu_index >= scheduler_cpu_count)
        return 0;
    return cpus[cpu_index].current_task;
}

extern "C" unsigned long long scheduler_switch_count(unsigned int cpu_index)
{
    if (cpu_index >= scheduler_cpu_count)
        return 0;
    return cpus[cpu_index].switches;
}

extern "C" bool scheduler_run_test()
{
    if (!scheduler_initialize(1))
        return false;

    for (unsigned int i = 0; i < SCHEDULER_QUANTUM_TICKS; ++i)
        scheduler_timer_tick(0);

    if (scheduler_current_task(0) != 1 ||
        scheduler_switch_count(0) != 1)
        return false;

    for (unsigned int i = 0; i < SCHEDULER_QUANTUM_TICKS; ++i)
        scheduler_timer_tick(0);

    return scheduler_current_task(0) == 0 &&
           scheduler_switch_count(0) == 2;
}
