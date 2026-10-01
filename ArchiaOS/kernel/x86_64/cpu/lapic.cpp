#include "lapic.hpp"
#include "../memory/paging.hpp"

static constexpr unsigned int IA32_APIC_BASE_MSR = 0x1B;
static constexpr unsigned long long APIC_BASE_MASK = 0xFFFFFFFFFFFFF000ULL;
static constexpr unsigned long long APIC_X2APIC_ENABLE = 1ULL << 10;
static constexpr unsigned long long APIC_ENABLE = 1ULL << 11;

static constexpr unsigned int IA32_X2APIC_SVR = 0x80F;
static constexpr unsigned int IA32_X2APIC_LVT_TIMER = 0x832;
static constexpr unsigned int IA32_X2APIC_INITIAL_COUNT = 0x838;
static constexpr unsigned int IA32_X2APIC_DIVIDE = 0x83E;
static constexpr unsigned int IA32_X2APIC_EOI = 0x80B;

static bool x2apic_mode = false;
static volatile unsigned char* lapic_base = nullptr;

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

static inline unsigned int cpuid_ecx(unsigned int leaf)
{
    unsigned int eax;
    unsigned int ebx;
    unsigned int ecx;
    unsigned int edx;

    asm volatile (
        "cpuid"
        : "=a"(eax), "=b"(ebx), "=c"(ecx), "=d"(edx)
        : "a"(leaf));

    return ecx;
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
    if ((cpuid_edx(1) & (1U << 9)) == 0)
        return false;

    unsigned long long apic_base = rdmsr(IA32_APIC_BASE_MSR);

    const bool x2apic_supported =
        (cpuid_ecx(1) & (1U << 21)) != 0;

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
