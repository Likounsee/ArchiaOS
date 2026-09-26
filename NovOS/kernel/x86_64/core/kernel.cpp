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

    debug_str("[BOOT] Boot information validated\n");

    debug_str("[BOOT] Memory map address: ");
    debug_hex64(bootInfo->memory_map_address);
    debug_str("\n");

    debug_str("[BOOT] Memory map size: ");
    debug_hex64(bootInfo->memory_map_size);
    debug_str("\n");

    debug_str("[BOOT] Descriptor size: ");
    debug_hex64(bootInfo->memory_descriptor_size);
    debug_str("\n");

    debug_str("[BOOT] Memory map entries: ");
    debug_hex64(bootInfo->memory_descriptor_count);
    debug_str("\n");

    debug_str("\n[NOVOS] === MEMORY ===\n");\n    debug_str("[PMM] Initializing physical memory manager\n");
    pmm_run_tests(bootInfo);

    debug_str("[MMU] Initializing paging\n");
    paging_run_tests();

    debug_str("\n[NOVOS] === CPU ===\n");\n    debug_str("[GDT] Initializing descriptor tables\n");
    gdt_initialize();
    debug_str("[GDT] Ready\n");

    debug_str("[TSS] Initializing task state segment\n");
    tss_initialize();
    debug_str("[TSS] Ready\n");
    debug_str("[TSS] Double-fault IST1 configured\n");

    debug_str("[IDT] Installing exception and IRQ gates\n");
    idt_initialize();
    debug_str("[IDT] Ready\n");

    debug_str("\n[NOVOS] === PLATFORM ===\n");\n    debug_str("[ACPI] Discovering firmware tables\n");
    if (acpi_initialize())
    {
        const AcpiInfo* acpi = acpi_get_info();

        debug_str("[ACPI] Ready\n");
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
        debug_str("[ACPI] Initialization failed, status: ");\n        debug_hex64(static_cast<u64>(acpi_get_status()));\n        debug_str("\n");
    }

    debug_str("[APIC] Initializing interrupt controller\n");
    if (!irq_initialize())
    {
        debug_str("[APIC] Initialization failed\n");
        for (;;)
            asm volatile ("cli; hlt");
    }
    debug_str("[APIC] Ready\n");

    debug_str("[IRQ] Testing Local APIC timer\n");
    if (irq_test_timer())
        debug_str("[IRQ] Timer interrupt test passed\n");
    else
        debug_str("[IRQ] Timer interrupt test FAILED\n");

    debug_str("\n[NOVOS] === EXCEPTION HANDLING ===\n");\n    debug_str("[EXC] Testing invalid-opcode recovery\n");
    idt_test_invalid_opcode();
    debug_str("[EXC] Invalid-opcode recovery passed\n");

    debug_str("[EXC] Testing double-fault IST1\n");
    idt_test_double_fault();

    debug_str("[EXC] Double-fault test returned unexpectedly\n");

    volatile unsigned short* vga =
        reinterpret_cast<volatile unsigned short*>(0xB8000);

    const char* text = "NOVOS KERNEL STARTED";

    for (int i = 0; text[i] != '\0'; ++i)
        vga[i] = static_cast<unsigned short>(0x0700 | text[i]);

    for (;;)
        asm volatile ("hlt");
}
