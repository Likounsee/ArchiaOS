#pragma once

struct CpuSecurityState
{
    bool nx_supported;
    bool nx_enabled;

    bool write_protect_enabled;

    bool umip_supported;
    bool umip_enabled;

    bool smep_supported;
    bool smep_enabled;

    bool smap_supported;
    bool smap_enabled;
};

extern "C" bool cpu_security_initialize();
extern "C" const CpuSecurityState* cpu_security_get_state();
extern "C" void cpu_security_print_report();
