#include "../../../common/boot_info.h"
#include "../cpu/gdt.hpp"
#include "../cpu/tss.hpp"
#include "../cpu/idt.hpp"
#include "../cpu/irq.hpp"
#include "../cpu/acpi.hpp"
#include "../memory/pmm.hpp"
#include "../memory/paging.hpp"
#include "../cpu/features.hpp"
#include "../cpu/security.hpp"
#include "../cpu/smp.hpp"
#include "../cpu/scheduler.hpp"
#include "../cpu/ioapic.hpp"
#include "../cpu/lapic.hpp"

using UINT32 = unsigned int;
using UINT64 = unsigned long long;

static inline void debug_char(char c)
{
    asm volatile(
        "outb %0,%1"
        :
        : "a"(c), "Nd"(static_cast<unsigned short>(0xE9))
        : "memory");
}

static void debug_str(const char* s)
{
    for (int i = 0; s[i]; ++i)
        debug_char(s[i]);
}

static void halt()
{
    for (;;)
        asm volatile("cli; hlt");
}

static void put_pixel(
    volatile UINT32* fb,
    UINT32 pitchPixels,
    UINT32 x,
    UINT32 y,
    UINT32 pixel)
{
    fb[static_cast<UINT64>(y) * pitchPixels + x] = pixel;
}

static void fill_rect(
    volatile UINT32* fb,
    UINT32 pitchPixels,
    UINT32 width,
    UINT32 height,
    UINT32 x,
    UINT32 y,
    UINT32 w,
    UINT32 h,
    UINT32 pixel)
{
    if (x >= width || y >= height)
        return;

    if (w > width - x)
        w = width - x;

    if (h > height - y)
        h = height - y;

    for (UINT32 py = 0; py < h; ++py)
        for (UINT32 px = 0; px < w; ++px)
            put_pixel(fb, pitchPixels, x + px, y + py, pixel);
}

extern "C" void pmm_run_tests(BootInfo* bootInfo);

extern "C" void kernel_main(BootInfo* bootInfo)
{
    debug_str("ARCHIAOS KERNEL STARTED\n");
    debug_str("Architecture: x86_64\n");

    if (!bootInfo)
    {
        debug_str("[KERNEL] BootInfo NULL\n");
        halt();
    }

    if (bootInfo->magic != NOVOS_BOOT_INFO_MAGIC)
    {
        debug_str("[KERNEL] BootInfo BAD MAGIC\n");
        halt();
    }

    if (bootInfo->version == 0 ||
        bootInfo->version > NOVOS_BOOT_INFO_VERSION ||
        bootInfo->size < sizeof(BootInfo))
    {
        debug_str("[KERNEL] BootInfo BAD VERSION/SIZE\n");
        halt();
    }

    if (!bootInfo->kernel_image_base ||
        !bootInfo->kernel_image_size ||
        !bootInfo->boot_info_address ||
        bootInfo->boot_info_size < sizeof(BootInfo))
    {
        debug_str("[KERNEL] BootInfo ABI addresses BAD\n");
        halt();
    }

    if (!bootInfo->memory_map_address ||
        !bootInfo->memory_map_size ||
        bootInfo->memory_descriptor_size < 40 ||
        bootInfo->memory_descriptor_size > 4096 ||
        bootInfo->memory_map_size <
            bootInfo->memory_descriptor_size ||
        bootInfo->memory_map_size %
            bootInfo->memory_descriptor_size != 0 ||
        bootInfo->memory_descriptor_count !=
            bootInfo->memory_map_size / bootInfo->memory_descriptor_size)
    {
        debug_str("[KERNEL] Memory map BAD\n");
        halt();
    }

    constexpr UINT64 bootstrapPhysicalLimit = NOVOS_PMM_MAX_PHYSICAL_ADDRESS;
    const auto physical_range_ok = [](UINT64 base, UINT64 size) -> bool
    {
        return base != 0 && size != 0 &&
               base < bootstrapPhysicalLimit &&
               size <= bootstrapPhysicalLimit - base;
    };

    if (!physical_range_ok(bootInfo->kernel_image_base,
                           bootInfo->kernel_image_size) ||
        !physical_range_ok(bootInfo->boot_info_address,
                           bootInfo->boot_info_size) ||
        !physical_range_ok(bootInfo->memory_map_address,
                           bootInfo->memory_map_size))
    {
        debug_str("[KERNEL] BOOT PHYSICAL RANGE EXCEEDS BOOTSTRAP LIMIT\\n");
        halt();
    }

    if (!physical_range_ok(bootInfo->framebuffer_base,
                           bootInfo->framebuffer_size))
    {
        debug_str("[KERNEL] FRAMEBUFFER PHYSICAL RANGE EXCEEDS BOOTSTRAP LIMIT\\n");
        halt();
    }

    if (!bootInfo->framebuffer_base ||
        !bootInfo->framebuffer_width ||
        !bootInfo->framebuffer_height)
    {
        debug_str("[KERNEL] Framebuffer BAD\n");
        halt();
    }

    const UINT64 minimumPitch =
        static_cast<UINT64>(bootInfo->framebuffer_width) * 4ULL;

    if (static_cast<UINT64>(bootInfo->framebuffer_pitch) < minimumPitch)
    {
        debug_str("[KERNEL] Framebuffer PITCH BAD\n");
        halt();
    }

    const UINT64 requiredFramebufferBytes =
        static_cast<UINT64>(bootInfo->framebuffer_pitch) *
        static_cast<UINT64>(bootInfo->framebuffer_height);

    if (requiredFramebufferBytes > bootInfo->framebuffer_size)
    {
        debug_str("[KERNEL] Framebuffer SIZE BAD\n");
        halt();
    }

    debug_str("BootInfo: OK\n");
    debug_str("Memory Map: OK\n");
    debug_str("Framebuffer: OK\n");

    debug_str("CPU: detecting CPUID/features/topology\n");
    cpu_initialize();
    cpu_print_report();

    debug_str("CPU: initializing GDT\n");
    gdt_initialize();
    debug_str("CPU: GDT OK\n");

    debug_str("CPU: initializing TSS\n");
    tss_initialize();
    debug_str("CPU: TSS OK\n");

    debug_str("CPU: initializing IDT\n");
    idt_initialize();
    debug_str("CPU: IDT 256 VECTORS OK\n");

    debug_str("PMM: initializing\n");
    pmm_initialize(bootInfo);
    debug_str("PMM: running tests\n");
    pmm_run_tests(bootInfo);
    debug_str("PMM: TESTS OK\n");

    /*
     * Enable CR0.WP/NXE/UMIP before creating 4 KiB mappings that use
     * execute-disable. IA32_EFER.NXE must be enabled before a present
     * paging entry is allowed to carry XD=1.
     */
    debug_str("CPU: activating hardware security protections\n");
    if (!cpu_security_initialize())
    {
        debug_str("[KERNEL] CPU SECURITY ACTIVATION FAILED\n");
        halt();
    }
    cpu_security_print_report();

    debug_str("MM: initializing paging\n");
    if (!paging_initialize())
    {
        debug_str("[KERNEL] PAGING INIT FAILED\n");
        halt();
    }
    paging_run_tests();
    debug_str("MM: paging ACTIVE\n");

    debug_str("CPU: testing invalid opcode handler\n");
    exception_expect_invalid_opcode(
        reinterpret_cast<unsigned long long>(&idt_test_invalid_opcode));
    idt_test_invalid_opcode();
    if (exception_invalid_opcode_test_active())
    {
        debug_str("[KERNEL] INVALID OPCODE TEST DID NOT TRAP\n");
        halt();
    }
    debug_str("CPU: INVALID OPCODE HANDLER OK\n");

    debug_str("IRQ: initializing LAPIC\n");
    if (!irq_initialize())
    {
        debug_str("[KERNEL] IRQ/LAPIC INIT FAILED\n");
        halt();
    }
    debug_str("IRQ: LAPIC OK\n");

    debug_str("IRQ: testing timer\n");
    if (!irq_test_timer())
    {
        debug_str("[KERNEL] IRQ TIMER TEST FAILED\n");
        halt();
    }
    debug_str("IRQ: TIMER TEST OK\n");
    if (irq_dispatch_count_get() < 3)
    {
        debug_str("[KERNEL] IRQ DISPATCH CHAIN VALIDATION FAILED\n");
        halt();
    }
    debug_str("IRQ: DISPATCH CHAIN OK\n");

    debug_str("SCHEDULER: initializing\n");
    if (!scheduler_initialize(1) || !scheduler_run_test())
    {
        debug_str("[KERNEL] SCHEDULER TEST FAILED\n");
        halt();
    }
    debug_str("SCHEDULER: ROUND-ROBIN TEST OK\n");

    debug_str("ACPI: parsing RSDP/MADT\n");
    if (!acpi_initialize(bootInfo->acpi_rsdp_address))
    {
        debug_str("[KERNEL] ACPI INIT FAILED\n");
        halt();
    }

    const AcpiInfo* acpi = acpi_get_info();
    if (!acpi ||
        acpi->rsdp_address == 0 ||
        acpi->root_table_address == 0 ||
        acpi->madt_address == 0 ||
        acpi->local_apic_address == 0 ||
        acpi->processor_count == 0)
    {
        debug_str("[KERNEL] ACPI DATA INVALID\n");
        halt();
    }

    debug_str("ACPI: RSDP/MADT OK\n");
    debug_str("ACPI: CPU/IOAPIC tables parsed\n");

    debug_str("IOAPIC: initializing redirection table\n");
    if (!ioapic_initialize(acpi))
    {
        debug_str("[KERNEL] IOAPIC INIT FAILED\n");
        halt();
    }

    const unsigned int bspApicId = lapic_current_id();
    if (!ioapic_route_isa_irq(acpi, 1, 0x21, bspApicId))
    {
        debug_str("[KERNEL] IOAPIC KEYBOARD ROUTE FAILED\n");
        halt();
    }

    unsigned long long keyboardRoute = 0;
    if (!ioapic_read_redirection(1, &keyboardRoute) ||
        (keyboardRoute & 0xFFULL) != 0x21ULL ||
        (keyboardRoute & (1ULL << 16)) != 0)
    {
        debug_str("[KERNEL] IOAPIC KEYBOARD ROUTE TEST FAILED\n");
        halt();
    }
    debug_str("IOAPIC: KEYBOARD IRQ ROUTE OK\n");
    debug_str("IRQ: KEYBOARD HANDLER READY\n");

    debug_str("SMP: starting application processors\n");
    if (!smp_initialize(acpi))
    {
        debug_str("[KERNEL] SMP INIT FAILED\\n");
        halt();
    }
    debug_str("SMP: APPLICATION PROCESSORS ONLINE\n");

    if (bootInfo->framebuffer_base)
    {
        /* Framebuffer is MMIO/video memory: use UC page mappings. */
        const UINT64 first = bootInfo->framebuffer_base & ~(NOVOS_PAGE_SIZE - 1ULL);
        const UINT64 end = bootInfo->framebuffer_base +
            static_cast<UINT64>(bootInfo->framebuffer_pitch) *
            static_cast<UINT64>(bootInfo->framebuffer_height);
        for (UINT64 page = first; page < end; page += NOVOS_PAGE_SIZE)
        {
            if (!paging_map_4k(page, page, PagingFlags{true, false, true, true, true}))
            {
                debug_str("[KERNEL] FRAMEBUFFER MMIO MAP FAILED\\n");
                halt();
            }
        }

        volatile UINT32* fb = reinterpret_cast<volatile UINT32*>(bootInfo->framebuffer_base);
        const UINT32 pitchPixels = bootInfo->framebuffer_pitch / 4;
        const UINT32 background = bootInfo->framebuffer_pixel_format == 0 ? 0x00101820U : 0x00201810U;
        fill_rect(fb, pitchPixels, bootInfo->framebuffer_width, bootInfo->framebuffer_height, 0, 0, bootInfo->framebuffer_width, bootInfo->framebuffer_height, background);
        fill_rect(fb, pitchPixels, bootInfo->framebuffer_width, bootInfo->framebuffer_height, 48, 48, 640, 96, 0x00FFFFFFU);
        fill_rect(fb, pitchPixels, bootInfo->framebuffer_width, bootInfo->framebuffer_height, 60, 60, 616, 72, background);
    }

    halt();
}
