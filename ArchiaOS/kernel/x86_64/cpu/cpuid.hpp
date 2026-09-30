#pragma once

struct CpuidResult
{
    unsigned int eax;
    unsigned int ebx;
    unsigned int ecx;
    unsigned int edx;
};

extern "C" CpuidResult cpu_cpuid(unsigned int leaf, unsigned int subleaf);
extern "C" unsigned long long cpu_read_xcr0();
