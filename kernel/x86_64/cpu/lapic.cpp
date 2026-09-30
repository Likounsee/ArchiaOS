#include "lapic.hpp"

static constexpr unsigned int IA32_APIC_BASE_MSR = 0x1B;
static constexpr unsigned long long APIC_BASE_MASK = 0xFFFFF000ULL;
static constexpr unsigned long long APIC_ENABLE = 1ULL << 11;
static constexpr unsigned long long APIC_X2APIC = 1ULL << 10;
static constexpr unsigned int CPUID_X2APIC = 1U << 21;
static constexpr unsigned int CPUID_APIC = 1U << 9;

static constexpr unsigned long long LAPIC_ID = 0x020;
static constexpr unsigned long long LAPIC_EOI = 0x0B0;
static constexpr unsigned long long LAPIC_SVR = 0x0F0;
static constexpr unsigned long long LAPIC_LVT_TIMER = 0x320;
static constexpr unsigned long long LAPIC_TIMER_INITIAL = 0x380;
static constexpr unsigned long long LAPIC_TIMER_DIVIDE = 0x3E0;

static constexpr unsigned int LAPIC_SVR_ENABLE = 1U << 8;
static constexpr unsigned int LAPIC_TIMER_PERIODIC = 1U << 17;

static volatile unsigned char* lapic_base = nullptr;
static bool lapic_x2apic = false;
static volatile unsigned long long lapic_ticks = 0;

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

static inline void cpuid(
    unsigned int leaf,
    unsigned int& eax,
    unsigned int& ebx,
    unsigned int& ecx,
    unsigned int& edx)
{
    asm volatile (
        "cpuid"
        : "=a"(eax), "=b"(ebx), "=c"(ecx), "=d"(edx)
        : "a"(leaf), "c"(0));
}

static inline unsigned int cpuid_ecx(unsigned int leaf)
{
    unsigned int eax;
    unsigned int ebx;
    unsigned int ecx;
    unsigned int edx;

    cpuid(leaf, eax, ebx, ecx, edx);
    return ecx;
}

static inline volatile unsigned int* lapic_register(
    unsigned long long offset)
{
    return reinterpret_cast<volatile unsigned int*>(lapic_base + offset);
}

static inline unsigned int x2apic_msr(unsigned long long offset)
{
    return 0x800U + static_cast<unsigned int>(offset >> 4);
}

static inline unsigned int lapic_read(unsigned long long offset)
{
    if (lapic_x2apic)
        return static_cast<unsigned int>(
            rdmsr(x2apic_msr(offset)));

    return *lapic_register(offset);
}

static inline void lapic_write(
    unsigned long long offset,
    unsigned int value)
{
    if (lapic_x2apic)
    {
        wrmsr(x2apic_msr(offset), value);
        return;
    }

    *lapic_register(offset) = value;
}

extern "C" bool lapic_initialize()
{
    const unsigned int cpuidEcx = cpuid_ecx(1);

    if ((cpuidEcx & CPUID_APIC) == 0)
        return false;

    unsigned long long apic_base =
        rdmsr(IA32_APIC_BASE_MSR);

    if ((apic_base & APIC_ENABLE) == 0)
    {
        apic_base |= APIC_ENABLE;
        wrmsr(IA32_APIC_BASE_MSR, apic_base);
        apic_base = rdmsr(IA32_APIC_BASE_MSR);
    }

    /*
     * Prefer x2APIC when the CPU supports it. In x2APIC mode APIC
     * registers are accessed through architecturally defined MSRs,
     * avoiding a dependency on a separately mapped MMIO window.
     */
    if ((cpuidEcx & CPUID_X2APIC) != 0)
    {
        if ((apic_base & APIC_X2APIC) == 0)
        {
            apic_base |= APIC_X2APIC | APIC_ENABLE;
            wrmsr(IA32_APIC_BASE_MSR, apic_base);
            apic_base = rdmsr(IA32_APIC_BASE_MSR);
        }

        if ((apic_base & APIC_X2APIC) != 0)
            lapic_x2apic = true;
    }

    if (!lapic_x2apic)
    {
        lapic_base =
            reinterpret_cast<volatile unsigned char*>(
                apic_base & APIC_BASE_MASK);

        if (lapic_base == nullptr)
            return false;
    }

    lapic_write(
        LAPIC_SVR,
        (lapic_read(LAPIC_SVR) & 0xFFFFFF00U) |
        LAPIC_SVR_ENABLE |
        0xFFU);

    lapic_write(
        LAPIC_LVT_TIMER,
        0x10000U | 0x20U);

    lapic_write(
        LAPIC_TIMER_DIVIDE,
        0x3U);

    lapic_ticks = 0;

    return true;
}

extern "C" void lapic_timer_start()
{
    lapic_write(
        LAPIC_LVT_TIMER,
        LAPIC_TIMER_PERIODIC | 0x20U);

    lapic_write(
        LAPIC_TIMER_INITIAL,
        1000000U);
}

extern "C" void lapic_stop_timer()
{
    lapic_write(
        LAPIC_LVT_TIMER,
        0x10000U | 0x20U);

    lapic_write(
        LAPIC_TIMER_INITIAL,
        0);
}

extern "C" void lapic_eoi()
{
    lapic_write(LAPIC_EOI, 0);
}

extern "C" unsigned long long lapic_get_ticks()
{
    return lapic_ticks;
}

extern "C" void lapic_timer_interrupt()
{
    ++lapic_ticks;
}
