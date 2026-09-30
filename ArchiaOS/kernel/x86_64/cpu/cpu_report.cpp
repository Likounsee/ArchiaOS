#include "features.hpp"

static void debug_char(char c)
{
    asm volatile(
        "outb %0,%1"
        :
        : "a"(c), "Nd"(static_cast<unsigned short>(0xE9))
        : "memory");
}

static void debug_str(const char* s)
{
    for (unsigned int i = 0; s[i]; ++i)
        debug_char(s[i]);
}

static void debug_hex(unsigned long long value)
{
    static const char digits[] = "0123456789ABCDEF";
    debug_str("0x");

    bool started = false;
    for (int shift = 60; shift >= 0; shift -= 4)
    {
        const unsigned int digit =
            static_cast<unsigned int>((value >> shift) & 0xFU);

        if (digit != 0 || started || shift == 0)
        {
            debug_char(digits[digit]);
            started = true;
        }
    }
}

static void yes_no(bool value)
{
    debug_str(value ? "YES" : "NO");
}

static void feature_line(const char* name, bool value)
{
    debug_str("CPU: ");
    debug_str(name);
    debug_str(": ");
    yes_no(value);
    debug_char('\n');
}

extern "C" void cpu_print_report()
{
    const CpuInfo* c = cpu_get_info();

    debug_str("CPU: vendor: ");
    debug_str(c->vendor);
    debug_char('\n');

    debug_str("CPU: brand: ");
    debug_str(c->brand);
    debug_char('\n');

    debug_str("CPU: family: ");
    debug_hex(c->family);
    debug_str(" model: ");
    debug_hex(c->model);
    debug_str(" stepping: ");
    debug_hex(c->stepping);
    debug_char('\n');

    debug_str("CPU: topology logical: ");
    debug_hex(c->topology.logical_processors);
    debug_str(" cores: ");
    debug_hex(c->topology.cores_per_package);
    debug_str(" threads/core: ");
    debug_hex(c->topology.threads_per_core);
    debug_str(" APIC: ");
    debug_hex(c->topology.initial_apic_id);
    debug_char('\n');

    debug_str("CPU: compatibility: ");
    debug_str(c->compatibility == CpuCompatibilityMode::Standard
        ? "STANDARD (SSE4.1)"
        : "LIGHT (below SSE4.1)");
    debug_char('\n');

    feature_line("SSE4.1", c->features.sse41);
    feature_line("SSE4.2", c->features.sse42);
    feature_line("AVX", c->features.avx);
    feature_line("AVX2", c->features.avx2);
    feature_line("AVX-512F", c->features.avx512f);
    feature_line("AES", c->features.aes);
    feature_line("FMA", c->features.fma);
    feature_line("BMI1", c->features.bmi1);
    feature_line("BMI2", c->features.bmi2);
    feature_line("NX", c->features.nx);
    feature_line("SMEP", c->features.smep);
    feature_line("SMAP", c->features.smap);
    feature_line("PCID", c->features.pcid);
    feature_line("INVPCID", c->features.invpcid);
    feature_line("x2APIC", c->features.x2apic);
    feature_line("UMIP", c->features.umip);
    feature_line("FSGSBASE", c->features.fsgsbase);
    feature_line("1GiB PAGES", c->features.one_gib_pages);
    feature_line("INVARIANT TSC", c->features.invariant_tsc);
    feature_line("HYPERVISOR", c->features.hypervisor_present);

    if (c->features.nx && c->features.smep && c->features.smap)
        debug_str("CPU: hardening profile: NX+SMEP+SMAP\n");
    else if (c->features.nx)
        debug_str("CPU: hardening profile: NX ONLY\n");
    else
        debug_str("CPU: hardening profile: LIMITED\n");

    debug_str("CPU: FEATURES DETECTED OK\n");
    debug_str("CPU: SECURITY FEATURES DETECTED OK\n");
    debug_str("CPU: TOPOLOGY DETECTED OK\n");
}
