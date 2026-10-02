#include "smp.hpp"
#include "acpi.hpp"
#include "lapic.hpp"
#include "irq.hpp"
#include "gdt.hpp"
#include "tss.hpp"
#include "idt.hpp"
#include "../memory/pmm.hpp"
#include "../memory/paging.hpp"
#include "scheduler.hpp"

static constexpr uint64_t TRAMPOLINE_LIMIT = 0x100000ULL;
static constexpr uint64_t TRAMPOLINE_MAILBOX_OFFSET = 0x400ULL;

extern "C" unsigned char smp_trampoline_start[];
extern "C" unsigned char smp_trampoline_end[];

static volatile uint32_t online_count = 0;

static inline uint64_t smp_read_cr0()
{
    uint64_t value;
    asm volatile("mov %%cr0,%0" : "=r"(value));
    return value;
}

static inline uint64_t smp_read_cr4()
{
    uint64_t value;
    asm volatile("mov %%cr4,%0" : "=r"(value));
    return value;
}

static inline uint64_t smp_read_efer()
{
    uint32_t low;
    uint32_t high;
    asm volatile("rdmsr" : "=a"(low), "=d"(high) : "c"(0xC0000080U));
    return (static_cast<uint64_t>(high) << 32) | low;
}

static inline void smp_debug(const char* s)
{
    for (int i = 0; s[i] != '\0'; ++i)
        asm volatile("outb %0,%1" : : "a"(s[i]), "Nd"(static_cast<unsigned short>(0xE9)));
}

static void smp_debug_hex(const char* label, uint64_t value)
{
    static const char digits[] = "0123456789ABCDEF";
    smp_debug(label);
    for (int shift = 60; shift >= 0; shift -= 4)
    {
        const char ch = digits[(value >> shift) & 0xFULL];
        asm volatile("outb %0,%1" : : "a"(ch), "Nd"(static_cast<unsigned short>(0xE9)));
    }
    smp_debug("\n");
}

extern "C" void smp_ap_entry(
    unsigned int processorIndex,
    unsigned int apicId,
    unsigned long long mailboxPhysical)
{
    irq_disable();
    smp_debug("SMP: AP C++ ENTRY\n");
    tss_initialize_cpu(processorIndex);
    gdt_initialize_cpu(processorIndex);
    gdt_load_cpu(processorIndex);
    tss_load_cpu(processorIndex);
    if (!scheduler_set_local_cpu_index(processorIndex))
    {
        smp_debug("SMP: AP CPU INDEX SET FAILED\n");
        for (;;)
            asm volatile("hlt");
    }
    if (tss_current_selector() != 0x18U)
    {
        smp_debug("SMP: AP TSS LOAD FAILED\n");
        for (;;)
            asm volatile("hlt");
    }
    smp_debug("SMP: AP TSS OK\n");
    idt_load_current();

    /* Each AP has its own local APIC LVT/timer state. */
    if (!lapic_initialize())
    {
        smp_debug("SMP: AP LAPIC INIT FAILED\n");
        for (;;)
            asm volatile("hlt");
    }
    smp_debug("SMP: AP LAPIC OK\n");

    __atomic_fetch_add(&online_count, 1U, __ATOMIC_SEQ_CST);
    auto* mailbox = reinterpret_cast<SmpTrampolineMailbox*>(mailboxPhysical);
    __atomic_store_n(&mailbox->started, 1U, __ATOMIC_RELEASE);
    smp_debug("SMP: AP REPORTED ONLINE\n");

    (void)apicId;

    while (!scheduler_ready())
        asm volatile("pause");

    smp_debug("SMP: AP SCHEDULER READY\n");
    scheduler_cpu_start(processorIndex);
    lapic_timer_start();
    irq_enable();

    for (;;)
        asm volatile("hlt");
}

static bool wait_for_ap(
    volatile uint32_t* started)
{
    for (uint64_t timeout = 0; timeout < 200000000ULL; ++timeout)
    {
        if (__atomic_load_n(started, __ATOMIC_ACQUIRE) != 0)
            return true;

        asm volatile("pause");
    }

    return false;
}

extern "C" bool smp_initialize(const AcpiInfo* acpi)
{
    online_count = 1;

    if (acpi == nullptr || acpi->processor_count == 0)
        return false;

    const unsigned int currentApicId = lapic_current_id();
    const unsigned int processorLimit =
        acpi->processor_count < AcpiInfo::MAX_PROCESSORS
            ? acpi->processor_count
            : AcpiInfo::MAX_PROCESSORS;

    unsigned char* trampolineSource = smp_trampoline_start;
    const uint64_t trampolineSize =
        static_cast<uint64_t>(smp_trampoline_end - smp_trampoline_start);

    if (trampolineSize == 0 || trampolineSize > NOVOS_PAGE_SIZE)
        return false;

    const uint64_t trampolinePhysical =
        pmm_alloc_page_below(TRAMPOLINE_LIMIT);

    if (trampolinePhysical == 0)
        return false;

    const uint64_t trampolineVirtual =
        paging_physical_to_virtual(trampolinePhysical);

    if (trampolineVirtual == 0 ||
        !paging_map_4k(
            trampolineVirtual,
            trampolinePhysical,
            PagingFlags{true, false, true, false, false}))
        return false;

    /*
     * The AP executes the trampoline through its physical/identity address
     * immediately after CR0.PG is enabled. Explicitly install the same
     * executable 4 KiB mapping in the identity half; relying on the shared
     * bootstrap hierarchy is too implicit for this critical transition.
     */
    if (!paging_map_4k(
            trampolinePhysical,
            trampolinePhysical,
            PagingFlags{true, false, true, false, false}))
        return false;

    auto* trampoline =
        reinterpret_cast<unsigned char*>(trampolineVirtual);

    for (uint64_t i = 0; i < NOVOS_PAGE_SIZE; ++i)
        trampoline[i] = 0;

    for (uint64_t i = 0; i < trampolineSize; ++i)
        trampoline[i] = trampolineSource[i];

    /*
     * The startup vector is the physical page number divided by 4 KiB.
     * SIPI vectors are limited to 8 bits, hence the low-memory allocation.
     */
    const unsigned int startupVector =
        static_cast<unsigned int>(trampolinePhysical >> 12);

    if (startupVector == 0 || startupVector > 0xFFU)
        return false;

    smp_debug_hex("SMP: TRAMPOLINE PHYS=", trampolinePhysical);
    smp_debug_hex("SMP: STARTUP VECTOR=", startupVector);
    smp_debug_hex("SMP: BSP APIC ID=", currentApicId);
    smp_debug("SMP: TRAMPOLINE READY\\n");

    auto* mailbox =
        reinterpret_cast<SmpTrampolineMailbox*>(
            trampoline + TRAMPOLINE_MAILBOX_OFFSET);

    const uint64_t cr3 = paging_pml4_physical();
    if (cr3 == 0 || cr3 >= 0x100000000ULL)
        return false;

    const uint64_t cr0 = smp_read_cr0();
    const uint64_t cr4 = smp_read_cr4() & ~(1ULL << 17);
    const uint64_t efer = smp_read_efer();

    const uint64_t entry =
        reinterpret_cast<uint64_t>(&smp_ap_entry);

    unsigned int expectedOnline = 1;

    for (unsigned int index = 0; index < processorLimit; ++index)
    {
        const unsigned int apicId = acpi->processor_apic_ids[index];
        smp_debug_hex("SMP: TARGET APIC ID=", apicId);

        if (apicId == currentApicId)
            continue;

        const uint64_t stackPhysical = pmm_alloc_page();
        if (stackPhysical == 0)
            return false;

        const uint64_t stackVirtual =
            paging_physical_to_virtual(stackPhysical);

        if (stackVirtual == 0 ||
            !paging_map_4k(
                stackVirtual,
                stackPhysical,
                PagingFlags{true, false, false, false, false}))
            return false;

        mailbox->cr3_physical = cr3;
        mailbox->entry_virtual = entry;
        mailbox->stack_virtual =
            (stackVirtual + NOVOS_PAGE_SIZE) & ~0xFULL;
        mailbox->cr0 = cr0;
        mailbox->cr4 = cr4;
        mailbox->efer = efer;
        mailbox->processor_index = index;
        mailbox->apic_id = apicId;
        __atomic_store_n(&mailbox->started, 0U, __ATOMIC_RELEASE);
        mailbox->reserved = 0;

        smp_debug("SMP: SENDING AP STARTUP\\n");
        if (!lapic_startup_cpu(apicId, startupVector))
        {
            smp_debug("SMP: AP STARTUP IPI FAILED\\n");
            return false;
        }

        smp_debug("SMP: AP STARTUP IPI SENT\\n");
        if (!wait_for_ap(&mailbox->started))
        {
            smp_debug("SMP: AP START TIMEOUT\\n");
            return false;
        }

        smp_debug("SMP: AP REPORTED ONLINE\\n");

        ++expectedOnline;

        /*
         * A single mailbox is reused sequentially. The AP is already inside
         * smp_ap_entry and no longer reads it after publishing 'started'.
         */
    }

    if (__atomic_load_n(&online_count, __ATOMIC_ACQUIRE) != expectedOnline)
        return false;

    return true;
}

extern "C" unsigned int smp_online_count()
{
    return __atomic_load_n(&online_count, __ATOMIC_ACQUIRE);
}
