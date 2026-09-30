#pragma once

enum class CpuCompatibilityMode
{
    Standard,
    Light
};

struct CpuFeatures
{
    bool mmx;
    bool sse;
    bool sse2;
    bool sse3;
    bool ssse3;
    bool sse41;
    bool sse42;

    bool xsave;
    bool osxsave;
    bool avx;
    bool avx2;
    bool avx512f;

    bool aes;
    bool pclmulqdq;
    bool popcnt;
    bool rdrand;
    bool rdseed;
    bool bmi1;
    bool bmi2;
    bool fma;

    bool nx;
    bool syscall_sysret;
    bool rdtscp;
    bool invariant_tsc;
    bool fsgsbase;
    bool pcid;
    bool tsc_deadline;
    bool x2apic;
    bool smep;
    bool smap;
    bool umip;
    bool invpcid;
    bool one_gib_pages;

    bool hypervisor_present;
};

struct CpuTopology
{
    unsigned int logical_processors;
    unsigned int threads_per_core;
    unsigned int cores_per_package;
    unsigned int package_count;
    unsigned int initial_apic_id;
    bool enumerated;
};

struct CpuInfo
{
    char vendor[13];
    char brand[49];

    unsigned int max_basic_leaf;
    unsigned int max_extended_leaf;
    unsigned int family;
    unsigned int model;
    unsigned int stepping;

    CpuFeatures features;
    CpuTopology topology;
    CpuCompatibilityMode compatibility;
};

extern "C" void cpu_initialize();
extern "C" const CpuInfo* cpu_get_info();
extern "C" void cpu_print_report();
