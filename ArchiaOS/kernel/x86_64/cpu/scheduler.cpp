#include "scheduler.hpp"
#include "lapic.hpp"

static SchedulerCpu cpus[SCHEDULER_MAX_CPUS];
static SchedulerTask tasks[SCHEDULER_MAX_CPUS][SCHEDULER_MAX_TASKS];
alignas(4096) __attribute__((section(".data.scheduler_stack")))
static unsigned char task1_stacks[SCHEDULER_MAX_CPUS][SCHEDULER_TASK_STACK_SIZE] = {};
static volatile unsigned long long task1_counters[SCHEDULER_MAX_CPUS] = {};
static volatile unsigned char task1_started[SCHEDULER_MAX_CPUS] = {};
static unsigned int scheduler_cpu_count = 1;
static unsigned int scheduler_apic_ids[SCHEDULER_MAX_CPUS] = {};
static volatile bool scheduler_ready_flag = false;
static volatile unsigned long long bootstrap_stack_top[SCHEDULER_MAX_CPUS] = {};

extern "C" [[noreturn]] void scheduler_task1_entry()
{
    const unsigned int cpu = scheduler_current_cpu_index();

    if (cpu < SCHEDULER_MAX_CPUS &&
        __atomic_exchange_n(&task1_started[cpu], 1U, __ATOMIC_ACQ_REL) == 0)
    {
        const char* prefix = "SCHEDULER: TASK1 CPU ";
        for (const char* p = prefix; *p; ++p)
            asm volatile("outb %0,%1" : : "a"(*p), "Nd"(static_cast<unsigned short>(0xE9)) : "memory");
        const char digit = static_cast<char>('0' + (cpu % 10));
        asm volatile("outb %0,%1" : : "a"(digit), "Nd"(static_cast<unsigned short>(0xE9)) : "memory");
        asm volatile("outb %0,%1" : : "a"('\n'), "Nd"(static_cast<unsigned short>(0xE9)) : "memory");
    }

    for (;;)
    {
        if (cpu < SCHEDULER_MAX_CPUS)
            __atomic_fetch_add(&task1_counters[cpu], 1ULL, __ATOMIC_RELAXED);
        asm volatile("pause");
    }
}

extern "C" bool scheduler_initialize(unsigned int cpu_count)
{
    if (cpu_count == 0 || cpu_count > SCHEDULER_MAX_CPUS)
        return false;

    scheduler_cpu_count = cpu_count;
    scheduler_ready_flag = false;

    for (unsigned int cpu = 0; cpu < cpu_count; ++cpu)
    {
        cpus[cpu] = SchedulerCpu{cpu, 0, 1, 2, 0};
        tasks[cpu][0] = SchedulerTask{0, cpu, 1, SCHEDULER_QUANTUM_TICKS, nullptr};
        tasks[cpu][1] = SchedulerTask{1, cpu, 1, SCHEDULER_QUANTUM_TICKS, nullptr};
        task1_counters[cpu] = 0;
        task1_started[cpu] = 0;
        scheduler_apic_ids[cpu] = 0xFFFFFFFFU;
    }

    for (unsigned int cpu = 0; cpu < cpu_count; ++cpu)
        bootstrap_stack_top[cpu] = 0;
    return true;
}

extern "C" bool scheduler_set_cpu_apic_ids(
    const unsigned int* apic_ids,
    unsigned int count)
{
    if (apic_ids == nullptr || count == 0 || count > scheduler_cpu_count)
        return false;

    for (unsigned int cpu = 0; cpu < count; ++cpu)
        scheduler_apic_ids[cpu] = apic_ids[cpu];

    return true;
}

extern "C" void scheduler_cpu_start(unsigned int cpu_index)
{
    if (cpu_index < scheduler_cpu_count)
        cpus[cpu_index].current_task = 0;
}

extern "C" bool scheduler_ready()
{
    return __atomic_load_n(&scheduler_ready_flag, __ATOMIC_ACQUIRE);
}

extern "C" unsigned int scheduler_current_cpu_index()
{
    unsigned int local_index;
    unsigned int reserved;
    asm volatile("rdmsr"
        : "=a"(local_index), "=d"(reserved)
        : "c"(0xC0000101U));

    if (local_index == 0)
        return 0;

    --local_index;
    return local_index < scheduler_cpu_count ? local_index : 0;
}

extern "C" ExceptionFrame* scheduler_timer_tick(
    unsigned int cpu_index,
    ExceptionFrame* current_frame)
{
    if (cpu_index >= scheduler_cpu_count || current_frame == nullptr ||
        !scheduler_ready())
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
                reinterpret_cast<unsigned long long>(task1_stacks[cpu_index]) +
                SCHEDULER_TASK_STACK_SIZE;
            stack_top &= ~0xFULL;
            bootstrap_stack_top[cpu_index] = stack_top;
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

extern "C" unsigned long long scheduler_task1_counter_get(unsigned int cpu_index)
{
    if (cpu_index >= scheduler_cpu_count)
        return 0;
    return __atomic_load_n(&task1_counters[cpu_index], __ATOMIC_ACQUIRE);
}

extern "C" bool scheduler_run_test()
{
    if (!scheduler_initialize(1))
        return false;

    unsigned int apic_id = lapic_current_id();
    if (!scheduler_set_cpu_apic_ids(&apic_id, 1))
        return false;

    scheduler_ready_flag = true;

    ExceptionFrame test_frame = {};
    for (unsigned int i = 0; i < SCHEDULER_QUANTUM_TICKS; ++i)
        scheduler_timer_tick(0, &test_frame);

    if (scheduler_current_task(0) != 1 ||
        scheduler_switch_count(0) != 1)
        return false;

    for (unsigned int i = 0; i < SCHEDULER_QUANTUM_TICKS; ++i)
        scheduler_timer_tick(0, &test_frame);

    const bool ok =
        scheduler_current_task(0) == 0 &&
        scheduler_switch_count(0) == 2;
    scheduler_ready_flag = false;
    return ok;
}

extern "C" unsigned long long scheduler_take_bootstrap_stack()
{
    const unsigned int cpu = scheduler_current_cpu_index();
    if (cpu >= scheduler_cpu_count)
        return 0;
    return __atomic_exchange_n(&bootstrap_stack_top[cpu], 0ULL, __ATOMIC_ACQ_REL);
}

extern "C" bool scheduler_set_ready_for_kernel()
{
    if (scheduler_cpu_count == 0)
        return false;
    __atomic_store_n(&scheduler_ready_flag, true, __ATOMIC_RELEASE);
    return true;
}

extern "C" bool scheduler_set_local_cpu_index(unsigned int cpu_index)
{
    if (cpu_index >= SCHEDULER_MAX_CPUS)
        return false;

    const unsigned long long value =
        static_cast<unsigned long long>(cpu_index) + 1ULL;
    const unsigned int low = static_cast<unsigned int>(value);
    const unsigned int high = static_cast<unsigned int>(value >> 32);
    asm volatile("wrmsr" : : "c"(0xC0000101U), "a"(low), "d"(high) : "memory");
    return true;
}
