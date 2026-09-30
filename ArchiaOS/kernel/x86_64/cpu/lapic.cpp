#include "lapic.hpp"

static constexpr unsigned int IA32_APIC_BASE_MSR = 0x1B;
static constexpr unsigned long long APIC_BASE_MASK = 0xFFFFF000ULL;
static constexpr unsigned long long APIC_ENABLE = 1ULL << 11;

static volatile unsigned char* lapic_base = nullptr;

static constexpr unsigned long long LAPIC_ID = 0x020;
static constexpr unsigned long long LAPIC_EOI = 0x0B0;
static constexpr unsigned long long LAPIC_SVR = 0x0F0;
static constexpr unsigned long long LAPIC_LVT_TIMER = 0x320;
static constexpr unsigned long long LAPIC_TIMER_INITIAL = 0x380;
static constexpr unsigned long long LAPIC_TIMER_DIVIDE = 0x3E0;

static constexpr unsigned int LAPIC_SVR_ENABLE = 1U << 8;
static constexpr unsigned int LAPIC_TIMER_PERIODIC = 1U << 17;

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

static inline volatile unsigned int* lapic_register(unsigned long long offset)
{
    return reinterpret_cast<volatile unsigned int*>(lapic_base + offset);
}

extern "C" bool lapic_initialize()
{
    if ((cpuid_edx(1) & (1U << 9)) == 0)
        return false;

    unsigned long long apic_base = rdmsr(IA32_APIC_BASE_MSR);

    if ((apic_base & APIC_ENABLE) == 0)
    {
        apic_base |= APIC_ENABLE;
        wrmsr(IA32_APIC_BASE_MSR, apic_base);
    }

    lapic_base = reinterpret_cast<volatile unsigned char*>(apic_base & APIC_BASE_MASK);

    if (lapic_base == nullptr)
        return false;

    *lapic_register(LAPIC_SVR) =
        (*lapic_register(LAPIC_SVR) & 0xFFFFFF00U) |
        LAPIC_SVR_ENABLE |
        0xFFU;

    *lapic_register(LAPIC_LVT_TIMER) = 0x10000U | 0x20U;

    *lapic_register(LAPIC_TIMER_DIVIDE) = 0x3U;

    lapic_ticks = 0;

    return true;
}

extern "C" void lapic_timer_start()
{
    *lapic_register(LAPIC_LVT_TIMER) =
        LAPIC_TIMER_PERIODIC | 0x20U;

    *lapic_register(LAPIC_TIMER_INITIAL) = 1000000U;
}

extern "C" void lapic_stop_timer()
{
    *lapic_register(LAPIC_LVT_TIMER) = 0x10000U | 0x20U;
    *lapic_register(LAPIC_TIMER_INITIAL) = 0;
}

extern "C" void lapic_eoi()
{
    *lapic_register(LAPIC_EOI) = 0;
}

extern "C" unsigned long long lapic_get_ticks()
{
    return lapic_ticks;
}

extern "C" void lapic_timer_interrupt()
{
    lapic_ticks = lapic_ticks + 1;
}
