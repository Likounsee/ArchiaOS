#include "smp.hpp"
#include "acpi.hpp"
#include "lapic.hpp"
#include "irq.hpp"
#include "gdt.hpp"
#include "idt.hpp"
#include "../memory/pmm.hpp"
#include "../memory/paging.hpp"

static constexpr uint64_t TRAMPOLINE_LIMIT = 0x100000ULL;
static constexpr uint64_t TRAMPOLINE_MAILBOX_OFFSET = 0x200ULL;
static constexpr uint64_t TRAMPOLINE_CR3_LIMIT = 0x100000000ULL;

extern "C" unsigned char smp_trampoline_start[];
extern "C" unsigned char smp_trampoline_end[];

static volatile uint32_t online_count = 0;

static inline uint64_t smp_read_cr0()\n{\n    uint64_t value;\n    asm volatile("mov %%cr0,%0" : "=r"(value));\n    return value;\n}\n\nstatic inline uint64_t smp_read_cr4()\n{\n    uint64_t value;\n    asm volatile("mov %%cr4,%0" : "=r"(value));\n    return value;\n}\n\nstatic inline uint64_t smp_read_efer()\n{\n    uint32_t low;\n    uint32_t high;\n    asm volatile("rdmsr" : "=a"(low), "=d"(high) : "c"(0xC0000080U));\n    return (static_cast<uint64_t>(high) << 32) | low;\n}

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
    unsigned int apicId)
{
    irq_disable();
    gdt_load_current();
    idt_load_current();

    __atomic_fetch_add(&online_count, 1U, __ATOMIC_SEQ_CST);

    (void)processorIndex;
    (void)apicId;

    for (;;)
        asm volatile("hlt");
}

static bool wait_for_ap(
    volatile uint32_t* started)
{
    for (uint64_t timeout = 0; timeout < 20000000ULL; ++timeout)
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

    auto* trampoline =
        reinterpret_cast<unsigned char*>(trampolineVirtual);

    for (uint64_t i = 0; i < NOVOS_PAGE_SIZE; ++i)
        trampoline[i] = 0;

    for (uint64_t i = 0; i < trampolineSize; ++i)
        trampoline[i] = trampolineSource[i];

    smp_debug_hex("SMP: TRAMPOLINE BYTE0 HHDM=", trampoline[0]);
    smp_debug_hex("SMP: TRAMPOLINE BYTE0 IDENTITY=", reinterpret_cast<volatile unsigned char*>(trampolinePhysical)[0]);

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
