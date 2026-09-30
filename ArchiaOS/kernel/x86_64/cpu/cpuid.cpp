#include "cpuid.hpp"
#include "features.hpp"

static CpuInfo cpu_info{};

extern "C" CpuidResult cpu_cpuid(unsigned int leaf, unsigned int subleaf)
{
    CpuidResult result{};
    asm volatile(
        "cpuid"
        : "=a"(result.eax), "=b"(result.ebx),
          "=c"(result.ecx), "=d"(result.edx)
        : "a"(leaf), "c"(subleaf));
    return result;
}

extern "C" unsigned long long cpu_read_xcr0()
{
    unsigned int eax;
    unsigned int edx;
    asm volatile(
        "xgetbv"
        : "=a"(eax), "=d"(edx)
        : "c"(0));
    return (static_cast<unsigned long long>(edx) << 32) | eax;
}

static bool bit(unsigned int value, unsigned int position)
{
    return (value & (1U << position)) != 0;
}

static void copy_vendor(const CpuidResult& r)
{
    *reinterpret_cast<unsigned int*>(&cpu_info.vendor[0]) = r.ebx;
    *reinterpret_cast<unsigned int*>(&cpu_info.vendor[4]) = r.edx;
    *reinterpret_cast<unsigned int*>(&cpu_info.vendor[8]) = r.ecx;
    cpu_info.vendor[12] = '\0';
}

static void copy_brand()
{
    for (unsigned int i = 0; i < sizeof(cpu_info.brand); ++i)
        cpu_info.brand[i] = '\0';

    if (cpu_info.max_extended_leaf < 0x80000004U)
        return;

    unsigned int* out = reinterpret_cast<unsigned int*>(cpu_info.brand);
    for (unsigned int leaf = 0; leaf < 3; ++leaf)
    {
        CpuidResult r = cpu_cpuid(0x80000002U + leaf, 0);
        out[leaf * 4 + 0] = r.eax;
        out[leaf * 4 + 1] = r.ebx;
        out[leaf * 4 + 2] = r.ecx;
        out[leaf * 4 + 3] = r.edx;
    }
    cpu_info.brand[48] = '\0';
}

static void decode_family_model(const CpuidResult& r)
{
    const unsigned int base_family = (r.eax >> 8) & 0xFU;
    const unsigned int base_model = (r.eax >> 4) & 0xFU;
    const unsigned int extended_family = (r.eax >> 20) & 0xFFU;
    const unsigned int extended_model = (r.eax >> 16) & 0xFU;

    cpu_info.stepping = r.eax & 0xFU;
    cpu_info.family = base_family;
    if (base_family == 0xFU)
        cpu_info.family += extended_family;

    cpu_info.model = base_model;
    if (base_family == 0x6U || base_family == 0xFU)
        cpu_info.model |= extended_model << 4;
}

static void enumerate_security_capabilities()
{
    CpuidResult leaf1 = cpu_cpuid(1, 0);
    CpuidResult leaf7 = cpu_cpuid(7, 0);

    cpu_info.features.x2apic = bit(leaf1.ecx, 21);
    cpu_info.features.pcid = bit(leaf1.ecx, 17);
    cpu_info.features.tsc_deadline = bit(leaf1.ecx, 24);

    cpu_info.features.invpcid = bit(leaf7.ebx, 10);
    cpu_info.features.smap = bit(leaf7.ebx, 20);
    cpu_info.features.sgx = bit(leaf7.ebx, 2);
    cpu_info.features.umip = bit(leaf7.ecx, 2);
    cpu_info.features.fsgsbase = bit(leaf7.ebx, 0);
}

static void enumerate_features()
{
    CpuidResult r = cpu_cpuid(1, 0);

    cpu_info.features.mmx = bit(r.edx, 23);
    cpu_info.features.sse = bit(r.edx, 25);
    cpu_info.features.sse2 = bit(r.edx, 26);

    cpu_info.features.sse3 = bit(r.ecx, 0);
    cpu_info.features.ssse3 = bit(r.ecx, 9);
    cpu_info.features.sse41 = bit(r.ecx, 19);
    cpu_info.features.sse42 = bit(r.ecx, 20);

    cpu_info.features.xsave = bit(r.ecx, 26);
    cpu_info.features.osxsave = bit(r.ecx, 27);
    cpu_info.features.avx = bit(r.ecx, 28);
    cpu_info.features.aes = bit(r.ecx, 25);
    cpu_info.features.pclmulqdq = bit(r.ecx, 1);
    cpu_info.features.popcnt = bit(r.ecx, 23);
    cpu_info.features.rdrand = bit(r.ecx, 30);
    cpu_info.features.fma = bit(r.ecx, 12);

    cpu_info.features.pcid = bit(r.ecx, 17);
    cpu_info.features.x2apic = bit(r.ecx, 21);
    cpu_info.features.tsc_deadline = bit(r.ecx, 24);

    cpu_info.features.syscall_sysret = false;
    cpu_info.features.rdtscp = false;
    cpu_info.features.invariant_tsc = false;
    cpu_info.features.fsgsbase = false;
    cpu_info.features.smep = false;
    cpu_info.features.smap = false;
    cpu_info.features.umip = false;
    cpu_info.features.invpcid = false;
    cpu_info.features.one_gib_pages = bit(r.edx, 26);
    cpu_info.features.avx2 = false;
    cpu_info.features.avx512f = false;
    cpu_info.features.bmi1 = false;
    cpu_info.features.bmi2 = false;
    cpu_info.features.rdseed = false;

    if (cpu_info.max_basic_leaf >= 7)
    {
        CpuidResult f7 = cpu_cpuid(7, 0);
        cpu_info.features.fsgsbase = bit(f7.ebx, 0);
        cpu_info.features.bmi1 = bit(f7.ebx, 3);
        cpu_info.features.avx2 = bit(f7.ebx, 5);
        cpu_info.features.smep = bit(f7.ebx, 7);
        cpu_info.features.bmi2 = bit(f7.ebx, 8);
        cpu_info.features.invpcid = bit(f7.ebx, 10);
        cpu_info.features.rdseed = bit(f7.ebx, 18);
        cpu_info.features.smap = bit(f7.ebx, 20);
        cpu_info.features.avx512f = bit(f7.ebx, 16);
        cpu_info.features.umip = bit(f7.ecx, 2);
    }

    if (cpu_info.max_extended_leaf >= 0x80000001U)
    {
        CpuidResult ext = cpu_cpuid(0x80000001U, 0);
        cpu_info.features.syscall_sysret = bit(ext.edx, 11);
        cpu_info.features.rdtscp = bit(ext.edx, 27);
        cpu_info.features.nx = bit(ext.edx, 20);
    }

    if (cpu_info.max_extended_leaf >= 0x80000007U)
    {
        CpuidResult power = cpu_cpuid(0x80000007U, 0);
        cpu_info.features.invariant_tsc = bit(power.edx, 8);
    }

    cpu_info.features.hypervisor_present = bit(r.ecx, 31);

    cpu_info.features.hybrid = false;
    cpu_info.features.core_type = CpuCoreType::Unknown;

    if (cpu_info.max_basic_leaf >= 7)
    {
        CpuidResult f7 = cpu_cpuid(7, 0);
        cpu_info.features.hybrid = bit(f7.edx, 15);
    }

    if (cpu_info.max_basic_leaf >= 0x1AU)
    {
        CpuidResult hybrid = cpu_cpuid(0x1AU, 0);
        const unsigned int type = hybrid.eax >> 24;
        if (type == 0x20U)
            cpu_info.features.core_type = CpuCoreType::ECoreOrAtom;
        else if (type == 0x40U)
            cpu_info.features.core_type = CpuCoreType::PCoreOrCore;
    }
}

static bool enumerate_topology_leaf(unsigned int leaf)
{
    if (cpu_info.max_basic_leaf < leaf)
        return false;

    unsigned int logical = 0;
    unsigned int threads = 0;
    unsigned int apic = 0;
    bool saw_level = false;

    for (unsigned int subleaf = 0; subleaf < 8; ++subleaf)
    {
        CpuidResult r = cpu_cpuid(leaf, subleaf);
        const unsigned int level_type = (r.ecx >> 8) & 0xFFU;

        if ((r.ebx & 0xFFFFU) == 0 || level_type == 0)
            break;

        saw_level = true;
        apic = r.edx;

        if (level_type == 1)
            threads = r.ebx & 0xFFFFU;

        if (level_type == 2)
            logical = r.ebx & 0xFFFFU;
    }

    if (!saw_level)
        return false;

    if (logical == 0)
        logical = threads;

    if (threads == 0)
        threads = 1;

    cpu_info.topology.logical_processors = logical;
    cpu_info.topology.threads_per_core = threads;
    cpu_info.topology.cores_per_package =
        logical >= threads ? logical / threads : 1;
    cpu_info.topology.package_count = 1;
    cpu_info.topology.initial_apic_id = apic;
    cpu_info.topology.enumerated = true;
    return true;
}

static void enumerate_topology()
{
    cpu_info.topology = {};

    if (enumerate_topology_leaf(0x1FU))
        return;

    if (enumerate_topology_leaf(0xBU))
        return;

    if (cpu_info.max_extended_leaf >= 0x8000001EU)
    {
        CpuidResult topo = cpu_cpuid(0x8000001EU, 0);
        CpuidResult ext8 = cpu_cpuid(0x80000008U, 0);

        const unsigned int cores = (ext8.ecx & 0xFFU) + 1U;
        const unsigned int logical = (cpu_cpuid(1, 0).ebx >> 16) & 0xFFU;
        unsigned int threads = 1;

        if (cores != 0 && logical >= cores && logical % cores == 0)
            threads = logical / cores;

        cpu_info.topology.logical_processors = logical ? logical : 1;
        cpu_info.topology.threads_per_core = threads;
        cpu_info.topology.cores_per_package = cores;
        cpu_info.topology.package_count = 1;
        cpu_info.topology.initial_apic_id = topo.eax;
        cpu_info.topology.enumerated = true;
        return;
    }

    // Legacy x86 fallback. Intel documents CPUID.1 EBX[23:16] as the
    // logical-processor count and CPUID.4 EAX[31:26] as the core count
    // when deterministic cache parameters are available.
    CpuidResult leaf1 = cpu_cpuid(1, 0);
    const unsigned int logical =
        (leaf1.ebx >> 16) & 0xFFU;

    unsigned int cores = 1;
    if (cpu_info.max_basic_leaf >= 4)
    {
        CpuidResult leaf4 = cpu_cpuid(4, 0);
        if ((leaf4.eax & 0x1FU) != 0)
            cores = ((leaf4.eax >> 26) & 0x3FU) + 1U;
    }

    if (cores > logical && logical != 0)
        cores = logical;

    unsigned int threads = 1;
    if (cores != 0 && logical >= cores && logical % cores == 0)
        threads = logical / cores;

    cpu_info.topology.logical_processors = logical ? logical : 1;
    cpu_info.topology.threads_per_core = threads;
    cpu_info.topology.cores_per_package = cores;
    cpu_info.topology.package_count = 1;
    cpu_info.topology.initial_apic_id = leaf1.ebx >> 24;
    cpu_info.topology.enumerated = true;
}

extern "C" void cpu_initialize()
{
    cpu_info = {};

    CpuidResult leaf0 = cpu_cpuid(0, 0);
    cpu_info.max_basic_leaf = leaf0.eax;
    copy_vendor(leaf0);

    CpuidResult ext0 = cpu_cpuid(0x80000000U, 0);
    cpu_info.max_extended_leaf = ext0.eax;

    decode_family_model(cpu_cpuid(1, 0));
    copy_brand();
    enumerate_features();
    enumerate_topology();

    cpu_info.compatibility =
        cpu_info.features.sse41
            ? CpuCompatibilityMode::Standard
            : CpuCompatibilityMode::Light;
}

extern "C" const CpuInfo* cpu_get_info()
{
    return &cpu_info;
}
