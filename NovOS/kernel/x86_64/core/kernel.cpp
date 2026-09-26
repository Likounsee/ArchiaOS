#include "../../../common/boot_info.h"
#include "gdt.hpp"
#include "idt.hpp"
#include "tss.hpp"
#include "pmm.hpp"
#include "paging.hpp"
#include "irq.hpp"
#include "acpi.hpp"

static inline void debug_char(char c)
{
    asm volatile (
        "outb %0, %1"
        :
        : "a"(c),
          "Nd"(static_cast<unsigned short>(0xE9))
    );
}

static void debug_str(const char* s)
{
    for (int i = 0; s[i] != '\0'; ++i)
        debug_char(s[i]);
}

static void debug_hex64(u64 value)
{
    const char* digits = "0123456789ABCDEF";
    debug_char('0');
    debug_char('x');
    for (int i = 0; i < 16; ++i)
        debug_char(digits[(value >> ((15 - i) * 4)) & 0xF]);
}

extern "C" void pmm_run_tests(BootInfo* bootInfo);
extern "C" void paging_run_tests();

extern "C" void kernel_main(BootInfo* bootInfo)
{
    if (bootInfo == nullptr)
    {
        debug_str("BOOT INFO NULL\n");
        for (;;) asm volatile ("hlt");
    }

    if (bootInfo->magic != NOVOS_BOOT_INFO_MAGIC)
    {
        debug_str("BOOT INFO BAD MAGIC\n");
        for (;;) asm volatile ("hlt");
    }

    if (bootInfo->version != NOVOS_BOOT_INFO_VERSION)
    {
        debug_str("BOOT INFO BAD VERSION\n");
        for (;;) asm volatile ("hlt");
    }

    if (bootInfo->memory_map_address == 0 ||
        bootInfo->memory_map_size == 0 ||
        bootInfo->memory_descriptor_size == 0)
    {
        debug_str("BOOT INFO BAD MEMORY MAP\n");
        for (;;) asm volatile ("hlt");
    }

    debug_str("BOOT INFO VALID\n");

    debug_str("MEMORY MAP ADDRESS: ");
    debug_hex64(bootInfo->memory_map_address);
    debug_str("\n");

    debug_str("MEMORY MAP SIZE: ");
    debug_hex64(bootInfo->memory_map_size);
    debug_str("\n");

    debug_str("MEMORY MAP DESCRIPTOR SIZE: ");
    debug_hex64(bootInfo->memory_descriptor_size);
    debug_str("\n");

    debug_str("MEMORY MAP ENTRIES: ");
    debug_hex64(bootInfo->memory_descriptor_count);
    debug_str("\n");

    debug_str("INITIALIZING PMM\n");
    pmm_run_tests(bootInfo);

    debug_str("INITIALIZING PAGING\n");
    paging_run_tests();

    debug_str("INITIALIZING GDT\n");
    gdt_initialize();
    debug_str("GDT INITIALIZED\n");

    debug_str("INITIALIZING TSS\n");
    tss_initialize();
    debug_str("TSS INITIALIZED\n");
    debug_str("DOUBLE FAULT IST1 CONFIGURED\n");

    debug_str("INITIALIZING IDT\n");
    idt_initialize();
    debug_str("IDT INITIALIZED\n");

    debug_str("INITIALIZING ACPI\n");
    if (acpi_initialize())
    {
        const AcpiInfo* acpi = acpi_get_info();

        debug_str("ACPI INITIALIZED\n");
        debug_str("RSDP: ");
        debug_hex64(acpi->rsdp_address);
        debug_str("\nMADT: ");
        debug_hex64(acpi->madt_address);
        debug_str("\nLOCAL APIC: ");
        debug_hex64(acpi->local_apic_address);
        debug_str("\nIO APIC: ");
        debug_hex64(acpi->ioapic_address);
        debug_str("\nPROCESSORS: ");
        debug_hex64(acpi->processor_count);
        debug_str("\nIO APICS: ");
        debug_hex64(acpi->ioapic_count);
        debug_str("\nINTERRUPT OVERRIDES: ");
        debug_hex64(acpi->interrupt_override_count);
        debug_str("\n");
    }
    else
    {
        debug_str("ACPI INITIALIZATION FAILED\n");
    }

    debug_str("INITIALIZING IRQ / LOCAL APIC\n");
    if (!irq_initialize())
    {
        debug_str("IRQ / LOCAL APIC INITIALIZATION FAILED\n");
        for (;;)
            asm volatile ("cli; hlt");
    }
    debug_str("IRQ / LOCAL APIC INITIALIZED\n");

    debug_str("TESTING LOCAL APIC TIMER IRQ\n");
    if (irq_test_timer())
        debug_str("LOCAL APIC TIMER IRQ TEST PASS\n");
    else
        debug_str("LOCAL APIC TIMER IRQ TEST FAIL\n");

    debug_str("TESTING #UD EXCEPTION RETURN\n");
    idt_test_invalid_opcode();
    debug_str("#UD RETURNED SUCCESSFULLY\n");

    debug_str("TESTING DOUBLE FAULT IST1\n");
    idt_test_double_fault();

    debug_str("DOUBLE FAULT TEST RETURNED\n");

    volatile unsigned short* vga =
        reinterpret_cast<volatile unsigned short*>(0xB8000);

    const char* text = "NOVOS KERNEL STARTED";

    for (int i = 0; text[i] != '\0'; ++i)
        vga[i] = static_cast<unsigned short>(0x0700 | text[i]);

    for (;;)
        asm volatile ("hlt");
}
