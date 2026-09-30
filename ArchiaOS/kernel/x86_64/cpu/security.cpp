#include "security.hpp"
#include "features.hpp"

static constexpr unsigned int IA32_EFER = 0xC0000080U;
static constexpr unsigned long long EFER_NXE = 1ULL << 11;

static constexpr unsigned long long CR0_WP = 1ULL << 16;

static constexpr unsigned long long CR4_UMIP = 1ULL << 11;
static constexpr unsigned long long CR4_SMEP = 1ULL << 20;
static constexpr unsigned long long CR4_SMAP = 1ULL << 21;

static CpuSecurityState state{};

static inline unsigned long long read_cr0()
{
    unsigned long long value;
    asm volatile("mov %%cr0, %0" : "=r"(value));
    return value;
}

static inline void write_cr0(unsigned long long value)
{
    asm volatile("mov %0, %%cr0" : : "r"(value) : "memory");
}

static inline unsigned long long read_cr4()
{
    unsigned long long value;
    asm volatile("mov %%cr4, %0" : "=r"(value));
    return value;
}

static inline void write_cr4(unsigned long long value)
{
    asm volatile("mov %0, %%cr4" : : "r"(value) : "memory");
}

static inline unsigned long long read_msr(unsigned int msr)
{
    unsigned int eax;
    unsigned int edx;

    asm volatile(
        "rdmsr"
        : "=a"(eax), "=d"(edx)
        : "c"(msr));

    return (static_cast<unsigned long long>(edx) << 32) | eax;
}

static inline void write_msr(unsigned int msr, unsigned long long value)
{
    const unsigned int eax = static_cast<unsigned int>(value);
    const unsigned int edx = static_cast<unsigned int>(value >> 32);

    asm volatile(
        "wrmsr"
        :
        : "c"(msr), "a"(eax), "d"(edx)
        : "memory");
}

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

extern "C" bool cpu_security_initialize()
{
    state = {};

    const CpuInfo* cpu = cpu_get_info();
    if (!cpu)
        return false;

    state.nx_supported = cpu->features.nx;
    state.umip_supported = cpu->features.umip;
    state.smep_supported = cpu->features.smep;
    state.smap_supported = cpu->features.smap;

    /*
     * CR0.WP is architectural in long mode. Enable it before the kernel
     * starts introducing read-only mappings so supervisor writes cannot
     * silently bypass R/W=0 later.
     */
    unsigned long long cr0 = read_cr0();
    cr0 |= CR0_WP;
    write_cr0(cr0);
    state.write_protect_enabled =
        (read_cr0() & CR0_WP) != 0;

    /*
     * NX is an architectural page-table feature exposed through EFER.NXE.
     * The CPUID NX bit is checked before touching EFER, as required by the
     * AMD64 architecture manual and Intel system-programming documentation.
     */
    if (state.nx_supported)
    {
        unsigned long long efer = read_msr(IA32_EFER);
        efer |= EFER_NXE;
        write_msr(IA32_EFER, efer);

        state.nx_enabled =
            (read_msr(IA32_EFER) & EFER_NXE) != 0;
    }

    /*
     * UMIP is safe to enable now because it only changes CPL>0 behaviour.
     * User mode does not exist yet, but enabling the architectural control
     * now means future ring-3 code will inherit the protection.
     */
    if (state.umip_supported)
    {
        unsigned long long cr4 = read_cr4();
        cr4 |= CR4_UMIP;
        write_cr4(cr4);

        state.umip_enabled =
            (read_cr4() & CR4_UMIP) != 0;
    }

    /*
     * SMEP/SMAP are deliberately not enabled yet. Our bootstrap address
     * space is currently supervisor-only identity paging and the kernel has
     * no user mappings. Enabling them becomes meaningful only when the page
     * table layer can create and audit U/S mappings and the page-fault path
     * is ready to handle the resulting protection faults.
     */
    state.smep_enabled = (read_cr4() & CR4_SMEP) != 0;
    state.smap_enabled = (read_cr4() & CR4_SMAP) != 0;

    return state.write_protect_enabled &&
           (!state.nx_supported || state.nx_enabled) &&
           (!state.umip_supported || state.umip_enabled);
}

extern "C" const CpuSecurityState* cpu_security_get_state()
{
    return &state;
}

extern "C" void cpu_security_print_report()
{
    debug_str("CPU: security activation
");

    debug_str("CPU: NXE: ");
    debug_str(state.nx_enabled ? "ACTIVE
" :
              state.nx_supported ? "SUPPORTED/NOT ACTIVE
" :
              "NOT SUPPORTED
");

    debug_str("CPU: CR0.WP: ");
    debug_str(state.write_protect_enabled ? "ACTIVE
" : "NOT ACTIVE
");

    debug_str("CPU: UMIP: ");
    debug_str(state.umip_enabled ? "ACTIVE
" :
              state.umip_supported ? "SUPPORTED/NOT ACTIVE
" :
              "NOT SUPPORTED
");

    debug_str("CPU: SMEP: ");
    debug_str(state.smep_enabled ? "ACTIVE
" :
              state.smep_supported ? "DEFERRED (USER PAGES)
" :
              "NOT SUPPORTED
");

    debug_str("CPU: SMAP: ");
    debug_str(state.smap_enabled ? "ACTIVE
" :
              state.smap_supported ? "DEFERRED (USER PAGES)
" :
              "NOT SUPPORTED
");

    debug_str("CPU: SECURITY ACTIVATION OK
");
}
