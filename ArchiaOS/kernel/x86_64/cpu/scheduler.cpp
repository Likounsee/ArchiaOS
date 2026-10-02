#include "scheduler.hpp"

static SchedulerCpu cpus[SCHEDULER_MAX_CPUS];
static SchedulerTask tasks[SCHEDULER_MAX_CPUS][SCHEDULER_MAX_TASKS];
alignas(4096) __attribute__((section(".data.scheduler_stack"))) static unsigned char task1_stack[SCHEDULER_TASK_STACK_SIZE] = {};
static volatile unsigned long long task1_counter = 0;
static unsigned int scheduler_cpu_count = 1;

extern "C" [[noreturn]] void scheduler_task1_entry()
{
    for (;;)
    {
        __atomic_fetch_add(&task1_counter, 1ULL, __ATOMIC_RELAXED);
        asm volatile("pause");
    }
}

static void scheduler_debug_hex(const char* label, unsigned long long value)
{
    for (const char* p = label; *p; ++p)
        asm volatile("outb %0,%1" : : "a"(*p), "Nd"(static_cast<unsigned short>(0xE9)) : "memory");

    const char* digits = "0123456789ABCDEF";
    asm volatile("outb %0,%1" : : "a"('0'), "Nd"(static_cast<unsigned short>(0xE9)) : "memory");
    asm volatile("outb %0,%1" : : "a"('x'), "Nd"(static_cast<unsigned short>(0xE9)) : "memory");
    for (int i = 15; i >= 0; --i)
    {
        const char c = digits[(value >> (i * 4)) & 0xF];
        asm volatile("outb %0,%1" : : "a"(c), "Nd"(static_cast<unsigned short>(0xE9)) : "memory");
    }
    asm volatile("outb %0,%1" : : "a"('\n'), "Nd"(static_cast<unsigned short>(0xE9)) : "memory");
}

static ExceptionFrame* scheduler_make_task1_frame()
{
    unsigned long long stack_top =
        reinterpret_cast<unsigned long long>(task1_stack) +
        SCHEDULER_TASK_STACK_SIZE;
    stack_top &= ~0xFULL;
    stack_top -= sizeof(ExceptionFrame);

    ExceptionFrame* frame = reinterpret_cast<ExceptionFrame*>(stack_top);
    *frame = {};
    frame->rip = reinterpret_cast<unsigned long long>(&scheduler_task1_entry);
    frame->cs = 0x08;
    frame->rflags = 0x202;
    return frame;
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

    tasks[0][1].saved_frame = scheduler_make_task1_frame();
    scheduler_debug_hex("SCHED FRAME=", reinterpret_cast<unsigned long long>(tasks[0][1].saved_frame));
    scheduler_debug_hex("SCHED RIP=", tasks[0][1].saved_frame->rip);
    scheduler_debug_hex("SCHED CS=", tasks[0][1].saved_frame->cs);
    scheduler_debug_hex("SCHED FLAGS=", tasks[0][1].saved_frame->rflags);
    scheduler_debug_hex("SCHED STACK=", reinterpret_cast<unsigned long long>(task1_stack) + SCHEDULER_TASK_STACK_SIZE);
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
        if (next.saved_frame == nullptr)
            return current_frame;

        next.remaining_quantum = SCHEDULER_QUANTUM_TICKS;
        ++cpu.switches;
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
