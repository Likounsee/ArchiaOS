#include "scheduler.hpp"
#include "lapic.hpp"

static SchedulerCpu cpus[SCHEDULER_MAX_CPUS];
static SchedulerTask tasks[SCHEDULER_MAX_CPUS][SCHEDULER_MAX_TASKS];
alignas(4096) __attribute__((section(".data.scheduler_stack")))
static unsigned char task_stacks[SCHEDULER_MAX_CPUS][SCHEDULER_MAX_TASKS][SCHEDULER_TASK_STACK_SIZE] = {};
static volatile unsigned long long task1_counters[SCHEDULER_MAX_CPUS] = {};
static volatile unsigned long long kernel_thread_counters[SCHEDULER_MAX_CPUS][2] = {};
static unsigned int scheduler_cpu_count = 1;
static unsigned int scheduler_apic_ids[SCHEDULER_MAX_CPUS] = {};
static volatile bool scheduler_ready_flag = false;
static volatile unsigned long long bootstrap_stack_top[SCHEDULER_MAX_CPUS] = {};
static volatile SchedulerThreadEntry bootstrap_entry[SCHEDULER_MAX_CPUS] = {};
static volatile void* bootstrap_argument[SCHEDULER_MAX_CPUS] = {};

extern "C" [[noreturn]] void scheduler_task1_entry(void*)
{
    const unsigned int cpu = scheduler_current_cpu_index();
    for (;;)
    {
        if (cpu < SCHEDULER_MAX_CPUS)
            __atomic_fetch_add(&task1_counters[cpu], 1ULL, __ATOMIC_RELAXED);
        asm volatile("pause");
    }
}

static bool task_ready(const SchedulerTask& task)
{
    return task.state == SCHEDULER_TASK_READY ||
           task.state == SCHEDULER_TASK_RUNNING;
}

extern "C" bool scheduler_initialize(unsigned int cpu_count)
{
    if (cpu_count == 0 || cpu_count > SCHEDULER_MAX_CPUS)
        return false;

    scheduler_cpu_count = cpu_count;
    scheduler_ready_flag = false;

    for (unsigned int cpu = 0; cpu < cpu_count; ++cpu)
    {
        cpus[cpu] = SchedulerCpu{cpu, 0, 1, 1, 0};
        for (unsigned int task = 0; task < SCHEDULER_MAX_TASKS; ++task)
            tasks[cpu][task] = SchedulerTask{
                task, cpu, SCHEDULER_TASK_UNUSED, SCHEDULER_QUANTUM_TICKS,
                nullptr, nullptr, nullptr, 0
            };

        tasks[cpu][0] = SchedulerTask{
            0, cpu, SCHEDULER_TASK_RUNNING, SCHEDULER_QUANTUM_TICKS,
            nullptr, nullptr, nullptr,
            reinterpret_cast<uint64_t>(task_stacks[cpu][0]) + SCHEDULER_TASK_STACK_SIZE
        };
        tasks[cpu][1] = SchedulerTask{
            1, cpu, SCHEDULER_TASK_READY, SCHEDULER_QUANTUM_TICKS,
            nullptr, scheduler_task1_entry, nullptr,
            reinterpret_cast<uint64_t>(task_stacks[cpu][1]) + SCHEDULER_TASK_STACK_SIZE
        };

        task1_counters[cpu] = 0;
        kernel_thread_counters[cpu][0] = 0;
        kernel_thread_counters[cpu][1] = 0;
        scheduler_apic_ids[cpu] = 0xFFFFFFFFU;
        bootstrap_stack_top[cpu] = 0;
        bootstrap_entry[cpu] = nullptr;
        bootstrap_argument[cpu] = nullptr;
    }
    return true;
}

extern "C" bool scheduler_set_cpu_apic_ids(
    const unsigned int* apic_ids, unsigned int count)
{
    if (!apic_ids || count == 0 || count > scheduler_cpu_count)
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

extern "C" bool scheduler_create_kernel_thread(
    SchedulerThreadEntry entry, void* argument, unsigned int cpu_index,
    unsigned int* task_id)
{
    if (!entry || cpu_index >= scheduler_cpu_count)
        return false;

    for (unsigned int id = 2; id < SCHEDULER_MAX_TASKS; ++id)
    {
        SchedulerTask& task = tasks[cpu_index][id];
        if (task.state != SCHEDULER_TASK_UNUSED)
            continue;

        task = SchedulerTask{
            id, cpu_index, SCHEDULER_TASK_READY, SCHEDULER_QUANTUM_TICKS,
            nullptr, entry, argument,
            reinterpret_cast<uint64_t>(task_stacks[cpu_index][id]) +
                SCHEDULER_TASK_STACK_SIZE
        };
        ++cpus[cpu_index].task_count;
        if (task_id)
            *task_id = id;
        return true;
    }
    return false;
}

extern "C" ExceptionFrame* scheduler_timer_tick(
    unsigned int cpu_index, ExceptionFrame* current_frame)
{
    if (cpu_index >= scheduler_cpu_count || !current_frame ||
        !scheduler_ready())
        return current_frame;

    SchedulerCpu& cpu = cpus[cpu_index];
    SchedulerTask& current = tasks[cpu_index][cpu.current_task];
    current.saved_frame = current_frame;

    if (current.remaining_quantum > 0)
        --current.remaining_quantum;

    if (current.remaining_quantum != 0)
        return current_frame;

    const unsigned int old = cpu.current_task;
    unsigned int candidate = (old + 1) % SCHEDULER_MAX_TASKS;

    for (unsigned int i = 0; i < SCHEDULER_MAX_TASKS; ++i)
    {
        const unsigned int id = (candidate + i) % SCHEDULER_MAX_TASKS;
        if (id != old && task_ready(tasks[cpu_index][id]))
        {
            candidate = id;
            break;
        }
        candidate = old;
    }

    if (candidate == old)
    {
        current.remaining_quantum = SCHEDULER_QUANTUM_TICKS;
        return current_frame;
    }

    cpu.current_task = candidate;
    cpu.next_task = old;
    SchedulerTask& next = tasks[cpu_index][candidate];
    next.state = SCHEDULER_TASK_RUNNING;
    next.remaining_quantum = SCHEDULER_QUANTUM_TICKS;
    ++cpu.switches;

    if (!next.saved_frame)
    {
        unsigned long long stack_top = next.stack_top & ~0xFULL;
        bootstrap_stack_top[cpu_index] = stack_top;
        bootstrap_entry[cpu_index] = next.entry;
        bootstrap_argument[cpu_index] = next.argument;
        return current_frame;
    }

    return next.saved_frame;
}

extern "C" unsigned int scheduler_current_task(unsigned int cpu_index)
{
    return cpu_index < scheduler_cpu_count ? cpus[cpu_index].current_task : 0;
}

extern "C" unsigned long long scheduler_switch_count(unsigned int cpu_index)
{
    return cpu_index < scheduler_cpu_count ? cpus[cpu_index].switches : 0;
}

extern "C" unsigned long long scheduler_task1_counter_get(unsigned int cpu_index)
{
    return cpu_index < scheduler_cpu_count
        ? __atomic_load_n(&task1_counters[cpu_index], __ATOMIC_ACQUIRE) : 0;
}

extern "C" unsigned long long scheduler_take_bootstrap_stack()
{
    const unsigned int cpu = scheduler_current_cpu_index();
    if (cpu >= scheduler_cpu_count)
        return 0;
    return __atomic_exchange_n(&bootstrap_stack_top[cpu], 0ULL, __ATOMIC_ACQ_REL);
}

extern "C" SchedulerThreadEntry scheduler_take_bootstrap_entry(void** argument)
{
    const unsigned int cpu = scheduler_current_cpu_index();
    if (cpu >= scheduler_cpu_count)
        return nullptr;

    SchedulerThreadEntry entry =
        __atomic_exchange_n(&bootstrap_entry[cpu], nullptr, __ATOMIC_ACQ_REL);
    void* arg = const_cast<void*>(__atomic_exchange_n(&bootstrap_argument[cpu], nullptr, __ATOMIC_ACQ_REL));
    if (argument)
        *argument = arg;
    return entry;
}

extern "C" [[noreturn]] void scheduler_bootstrap_entry()
{
    void* argument = nullptr;
    SchedulerThreadEntry entry = scheduler_take_bootstrap_entry(&argument);
    if (entry)
        entry(argument);

    for (;;)
        asm volatile("cli; hlt");
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

    if (scheduler_current_task(0) != 1 || scheduler_switch_count(0) != 1)
        return false;

    for (unsigned int i = 0; i < SCHEDULER_QUANTUM_TICKS; ++i)
        scheduler_timer_tick(0, &test_frame);

    const bool ok = scheduler_current_task(0) == 0 &&
                    scheduler_switch_count(0) == 2;
    scheduler_ready_flag = false;
    return ok;
}

static void kernel_thread_test_entry0(void*)
{
    const unsigned int cpu = scheduler_current_cpu_index();
    for (unsigned int i = 0; i < 100000; ++i)
    {
        __atomic_fetch_add(&kernel_thread_counters[cpu][0], 1ULL, __ATOMIC_RELAXED);
        asm volatile("pause");
    }
    for (;;)
        asm volatile("pause");
}

static void kernel_thread_test_entry1(void*)
{
    const unsigned int cpu = scheduler_current_cpu_index();
    for (unsigned int i = 0; i < 100000; ++i)
    {
        __atomic_fetch_add(&kernel_thread_counters[cpu][1], 1ULL, __ATOMIC_RELAXED);
        asm volatile("pause");
    }
    for (;;)
        asm volatile("pause");
}

extern "C" bool scheduler_kernel_thread_test()
{
    unsigned int cpu = scheduler_current_cpu_index();
    unsigned int a = 0;
    unsigned int b = 0;
    if (!scheduler_create_kernel_thread(kernel_thread_test_entry0, nullptr, cpu, &a) ||
        !scheduler_create_kernel_thread(kernel_thread_test_entry1, nullptr, cpu, &b))
        return false;

    if (a == b || a < 2 || b < 2)
        return false;

    return true;
}

extern "C" unsigned long long scheduler_kernel_thread_counter(unsigned int cpu_index, unsigned int thread_index)
{
    if (cpu_index >= scheduler_cpu_count || thread_index >= 2)
        return 0;
    return __atomic_load_n(&kernel_thread_counters[cpu_index][thread_index], __ATOMIC_ACQUIRE);
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

    const unsigned long long value = static_cast<unsigned long long>(cpu_index) + 1ULL;
    const unsigned int low = static_cast<unsigned int>(value);
    const unsigned int high = static_cast<unsigned int>(value >> 32);
    asm volatile("wrmsr" : : "c"(0xC0000101U), "a"(low), "d"(high) : "memory");
    return true;
}
