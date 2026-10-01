#include "lapic.hpp"
#include "../memory/paging.hpp"
#include "features.hpp"
#include "cpuid.hpp"

static constexpr unsigned int IA32_APIC_BASE_MSR = 0x1B;
static constexpr unsigned long long APIC_BASE_MASK = 0xFFFFFFFFFFFFF000ULL;
static constexpr unsigned long long APIC_X2APIC_ENABLE = 1ULL << 10;
static constexpr unsigned long long APIC_ENABLE = 1ULL << 11;

static constexpr unsigned int IA32_X2APIC_SVR = 0x80F;
static constexpr unsigned int IA32_X2APIC_LVT_TIMER = 0x832;
static constexpr unsigned int IA32_X2APIC_INITIAL_COUNT = 0x838;
static constexpr unsigned int IA32_X2APIC_CURRENT_COUNT = 0x839;
static constexpr unsigned int IA32_X2APIC_DIVIDE = 0x83E;
static constexpr unsigned int IA32_X2APIC_EOI = 0x80B;

static bool x2apic_mode = false;
static volatile unsigned char* lapic_base = nullptr;

static constexpr unsigned long long LAPIC_EOI = 0x0B0;
static constexpr unsigned long long LAPIC_SVR = 0x0F0;
static constexpr unsigned long long LAPIC_LVT_TIMER = 0x320;
static constexpr unsigned long long LAPIC_TIMER_INITIAL = 0x380;
static constexpr unsigned long long LAPIC_TIMER_CURRENT = 0x390;
static constexpr unsigned long long LAPIC_TIMER_DIVIDE = 0x3E0;

static constexpr unsigned int LAPIC_SVR_ENABLE = 1U << 8;
static constexpr unsigned int LAPIC_TIMER_PERIODIC = 1U << 17;

static volatile unsigned long long lapic_ticks = 0;
static unsigned int lapic_timer_initial_count = 1000000U;

static inline unsigned long long rdmsr(unsigned int msr)
{
    unsigned int low;
    unsigned int high;

    asm volatile (
        "rdmsr"
        : "=a"(low), "=d"(high)
        : "c"(msr)
    );

    return (static_cast<unsigned long long>(high) << 32) | low;
}

static inline void wrmsr(unsigned int msr, unsigned long long value)
{
    unsigned int low = static_cast<unsigned int>(value);
    unsigned int high = static_cast<unsigned int>(value >> 32);

    asm volatile (
        "wrmsr"
        :
        : "c"(msr), "a"(low), "d"(high)
    );
}

static inline unsigned int cpuid_edx(unsigned int leaf)
{
    unsigned int eax;
    unsigned int ebx;
    unsigned int ecx;
    unsigned int edx;

    asm volatile (
        "cpuid"
        : "=a"(eax), "=b"(ebx), "=c"(ecx), "=d"(edx)
        : "a"(leaf)
    );

    return edx;
}

static inline unsigned long long read_tsc()
{
    unsigned int low;
    unsigned int high;

    asm volatile(
        "rdtsc"
        : "=a"(low), "=d"(high));

    return (static_cast<unsigned long long>(high) << 32) | low;
}

static unsigned long long detect_tsc_frequency_hz()
{
    const CpuInfo* cpu = cpu_get_info();
    if (cpu == nullptr || !cpu->features.invariant_tsc)
        return 0;

    /*
     * CPUID.15H describes the TSC/core-crystal ratio. Intel documents
     * TSC frequency as ECX * EBX / EAX when all three values are
     * enumerated. Fall back to CPUID.16H's nominal base frequency
     * only when the architectural TSC ratio is unavailable.
     */
    if (cpu->max_basic_leaf >= 0x15U)
    {
        const CpuidResult r = cpu_cpuid(0x15U, 0);
        if (r.eax != 0 && r.ebx != 0 && r.ecx != 0)
        {
            const unsigned long long crystal = r.ecx;
            const unsigned long long numerator = r.ebx;

            if (crystal <= (~0ULL / numerator))
            {
                const unsigned long long product =
                    crystal * numerator;
                const unsigned long long frequency =
                    product / r.eax;

                if (frequency != 0)
                    return frequency;
            }
        }
    }

    if (cpu->max_basic_leaf >= 0x16U)
    {
        const CpuidResult r = cpu_cpuid(0x16U, 0);
        const unsigned long long baseMHz =
            static_cast<unsigned long long>(r.eax & 0xFFFFU);

        if (baseMHz != 0 &&
            baseMHz <= (~0ULL / 1000000ULL))
        {
            return baseMHz * 1000000ULL;
        }
    }

    return 0;
}

static unsigned long long lapic_read(
    unsigned int xapicOffset,
    unsigned int x2apicMsr);

static void lapic_write(
    unsigned int xapicOffset,
    unsigned int x2apicMsr,
    unsigned long long value);

static bool calibrate_lapic_timer()
{
    const unsigned long long tscHz = detect_tsc_frequency_hz();
    if (tscHz == 0)
        return false;

    const unsigned long long calibrationTscTicks = tscHz / 50ULL;
    if (calibrationTscTicks == 0)
        return false;

    /*
     * Use a masked one-shot timer so calibration cannot generate an IRQ.
     * The APIC timer then runs at divide-by-16 while the invariant TSC
     * provides the reference interval.
     */
    lapic_write(
        LAPIC_LVT_TIMER,
        IA32_X2APIC_LVT_TIMER,
        0x10000U | 0x20U);
    lapic_write(
        LAPIC_TIMER_DIVIDE,
        IA32_X2APIC_DIVIDE,
        0x3U);

    constexpr unsigned int initialCount = 0xFFFFFFFFU;
    lapic_write(
        LAPIC_TIMER_INITIAL,
        IA32_X2APIC_INITIAL_COUNT,
        initialCount);

    const unsigned long long start = read_tsc();
    const unsigned long long target = start + calibrationTscTicks;

    while (read_tsc() < target)
        asm volatile("pause");

    const unsigned long long current =
        lapic_read(LAPIC_TIMER_CURRENT, IA32_X2APIC_CURRENT_COUNT);

    lapic_write(
        LAPIC_TIMER_INITIAL,
        IA32_X2APIC_INITIAL_COUNT,
        0);

    if (current >= initialCount)
        return false;

    const unsigned long long elapsed =
        static_cast<unsigned long long>(initialCount) - current;

    /*
     * The reference interval is 20 ms. A 100 Hz periodic tick is therefore
     * half that interval, so use half of the measured APIC decrements.
     */
    const unsigned long long count100Hz = elapsed / 2ULL;

    if (count100Hz == 0 || count100Hz > 0xFFFFFFFFULL)
        return false;

    lapic_timer_initial_count =
        static_cast<unsigned int>(count100Hz);

    return true;
}

static inline volatile unsigned int* lapic_register(unsigned long long offset)
{
    return reinterpret_cast<volatile unsigned int*>(lapic_base + offset);
}

static inline unsigned long long lapic_read(unsigned int xapicOffset, unsigned int x2apicMsr)
{
    return x2apic_mode
        ? rdmsr(x2apicMsr)
        : *lapic_register(xapicOffset);
}

static inline void lapic_write(unsigned int xapicOffset, unsigned int x2apicMsr, unsigned long long value)
{
    if (x2apic_mode)
        wrmsr(x2apicMsr, value);
    else
        *lapic_register(xapicOffset) = static_cast<unsigned int>(value);
}

extern "C" bool lapic_initialize()
{
    const CpuInfo* cpu = cpu_get_info();
    if (cpu == nullptr || cpu->max_basic_leaf < 1)
        return false;

    if (!cpu->features.x2apic &&
        (cpuid_edx(1) & (1U << 9)) == 0)
        return false;

    unsigned long long apic_base = rdmsr(IA32_APIC_BASE_MSR);

    const bool x2apic_supported = cpu->features.x2apic;

    if ((apic_base & APIC_ENABLE) == 0)
    {
        apic_base |= APIC_ENABLE;
        wrmsr(IA32_APIC_BASE_MSR, apic_base);
    }

    x2apic_mode = (apic_base & APIC_X2APIC_ENABLE) != 0;

    if (x2apic_mode && !x2apic_supported)
        return false;

    if (x2apic_mode)
        lapic_base = nullptr;
    else
        lapic_base = reinterpret_cast<volatile unsigned char*>(apic_base & APIC_BASE_MASK);

    if (!x2apic_mode && lapic_base == nullptr)
        return false;

    if (!x2apic_mode)
    {
        const u64 physical = apic_base & APIC_BASE_MASK;
        if (physical >= NOVOS_PMM_MAX_PHYSICAL_ADDRESS ||
            !paging_map_4k(physical, physical, PagingFlags{
                true, false, true, true, true}))
            return false;
        lapic_base = reinterpret_cast<volatile unsigned char*>(physical);
    }

    const unsigned long long svr =
        lapic_read(LAPIC_SVR, IA32_X2APIC_SVR);
    lapic_write(
        LAPIC_SVR,
        IA32_X2APIC_SVR,
        (svr & ~0xFFULL) | LAPIC_SVR_ENABLE | 0xFFULL);

    /* Mask LINT0/LINT1 and the APIC error vector until handlers exist. */
    lapic_write(0x350, 0x835, 0x10000U | 0xFFU);
    lapic_write(0x360, 0x836, 0x10000U | 0xFFU);
    lapic_write(0x370, 0x837, 0x10000U | 0xFEU);
    lapic_write(0x080, 0x808, 0);
    lapic_write(LAPIC_LVT_TIMER, IA32_X2APIC_LVT_TIMER, 0x10000U | 0x20U);
    lapic_write(LAPIC_TIMER_DIVIDE, IA32_X2APIC_DIVIDE, 0x3U);

    lapic_timer_initial_count = 1000000U;
    if (!calibrate_lapic_timer())
        lapic_timer_initial_count = 1000000U;

    lapic_ticks = 0;

    return true;
}

extern "C" void lapic_timer_start()
{
    lapic_write(
        LAPIC_LVT_TIMER,
        IA32_X2APIC_LVT_TIMER,
        LAPIC_TIMER_PERIODIC | 0x20U);

    lapic_write(LAPIC_TIMER_INITIAL, IA32_X2APIC_INITIAL_COUNT, 1000000U);
}

extern "C" void lapic_stop_timer()
{
    lapic_write(
        LAPIC_LVT_TIMER,
        IA32_X2APIC_LVT_TIMER,
        0x10000U | 0x20U);
    lapic_write(LAPIC_TIMER_INITIAL, IA32_X2APIC_INITIAL_COUNT, 0);
}

extern "C" void lapic_eoi()
{
    lapic_write(LAPIC_EOI, IA32_X2APIC_EOI, 0);
}

extern "C" unsigned long long lapic_get_ticks()
{
    return lapic_ticks;
}

extern "C" void lapic_timer_interrupt()
{
    lapic_ticks = lapic_ticks + 1;
}
