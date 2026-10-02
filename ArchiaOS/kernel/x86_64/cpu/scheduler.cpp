#include "scheduler.hpp"

static SchedulerCpu cpus[SCHEDULER_MAX_CPUS];
static SchedulerTask tasks[SCHEDULER_MAX_CPUS][SCHEDULER_MAX_TASKS];
alignas(4096) __attribute__((section(".data.scheduler_stack"))) static unsigned char task1_stack[SCHEDULER_TASK_STACK_SIZE] = {};
static volatile unsigned long long task1_counter = 0;
static unsigned int scheduler_cpu_count = 1;
static volatile unsigned long long bootstrap_stack_top = 0;

extern "C" [[noreturn]] void scheduler_task1_entry()
{
    for (;;)
    {
        __atomic_fetch_add(&task1_counter, 1ULL, __ATOMIC_RELAXED);
        asm volatile("pause");
    }
}

extern "C" bool scheduler_initialize(unsigned int cpu_count)
{
    if (cpu_count == 0 || cpu_count > SCHEDULER_MAX_CPUS)
        return false;

    scheduler_cpu_count = cpu_count;
    for (unsigned int cpu = 0; cpu < cpu_count; ++cpu)
    {
        cpus[cpu] = SchedulerCpu{cpu, 0, 1, 2, 0};
        tasks[cpu][0] = SchedulerTask{0, cpu, 1, SCHEDULER_QUANTUM_TICKS, nullptr};
        tasks[cpu][1] = SchedulerTask{1, cpu, 1, SCHEDULER_QUANTUM_TICKS, nullptr};
    }

    tasks[0][1].saved_frame = nullptr;
    bootstrap_stack_top = 0;
    task1_counter = 0;
    return true;
}

extern "C" ExceptionFrame* scheduler_timer_tick(
    unsigned int cpu_index,
    ExceptionFrame* current_frame)
{
    if (cpu_index >= scheduler_cpu_count || current_frame == nullptr)
        return current_frame;

    SchedulerCpu& cpu = cpus[cpu_index];
    SchedulerTask& current = tasks[cpu_index][cpu.current_task];
    current.saved_frame = current_frame;

    if (current.remaining_quantum > 0)
        --current.remaining_quantum;

    if (current.remaining_quantum == 0)
    {
        const unsigned int old = cpu.current_task;
        cpu.current_task = cpu.next_task;
        cpu.next_task = old;

        SchedulerTask& next = tasks[cpu_index][cpu.current_task];
        next.remaining_quantum = SCHEDULER_QUANTUM_TICKS;
        ++cpu.switches;

        if (next.saved_frame == nullptr && next.id == 1)
        {
            unsigned long long stack_top =
                reinterpret_cast<unsigned long long>(task1_stack) +
                SCHEDULER_TASK_STACK_SIZE;
            stack_top &= ~0xFULL;
            bootstrap_stack_top = stack_top;
            return current_frame;
        }

        if (next.saved_frame == nullptr)
            return current_frame;

        return next.saved_frame;
    }

    return current_frame;
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

extern "C" unsigned long long scheduler_task1_counter_get()
{
    return __atomic_load_n(&task1_counter, __ATOMIC_ACQUIRE);
}

extern "C" bool scheduler_run_test()
{
    if (!scheduler_initialize(1))
        return false;

    ExceptionFrame test_frame = {};
    for (unsigned int i = 0; i < SCHEDULER_QUANTUM_TICKS; ++i)
        scheduler_timer_tick(0, &test_frame);

    if (scheduler_current_task(0) != 1 ||
        scheduler_switch_count(0) != 1)
        return false;

    for (unsigned int i = 0; i < SCHEDULER_QUANTUM_TICKS; ++i)
        scheduler_timer_tick(0, &test_frame);

    return scheduler_current_task(0) == 0 &&
           scheduler_switch_count(0) == 2;
}

extern "C" unsigned long long scheduler_take_bootstrap_stack()
{
    return __atomic_exchange_n(&bootstrap_stack_top, 0ULL, __ATOMIC_ACQ_REL);
}
