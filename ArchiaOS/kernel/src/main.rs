#![no_std]
#![no_main]

mod memory;
mod cpu;
mod cpu_features;
mod pci;
mod scheduler;
mod context;
mod interrupts;

use core::arch::{asm, global_asm};
use core::fmt::{self, Write};
use core::panic::PanicInfo;
use core::ptr;

// Keep the first load segment non-empty with an ArchiaOS-owned marker.
#[used]
#[link_section = ".archiaos_image_marker"]
static ARCHIAOS_UEFI_IMAGE_MARKER: [u64; 1] = [0x4152_4348_4941_5545];

#[cfg(not(feature = "uefi-boot"))]
compile_error!("ArchiaOS production kernel requires the uefi-boot feature");

#[cfg(feature = "uefi-boot")]
global_asm!(
    ".section .text.boot, \"ax\"",
    ".global _start",
    ".type _start, @function",
    "_start:",
    "cli",
    "cld",
    // Bring up COM1 in assembly before touching the new stack. These single-byte
    // stage markers distinguish reaching the ELF entry from a stack/Rust failure.
    // They use no UEFI services and remain available after ExitBootServices.
    "mov dx, 0x3f9",
    "xor al, al",
    "out dx, al",
    "mov dx, 0x3fb",
    "mov al, 0x80",
    "out dx, al",
    "mov dx, 0x3f8",
    "mov al, 0x03",
    "out dx, al",
    "mov dx, 0x3f9",
    "xor al, al",
    "out dx, al",
    "mov dx, 0x3fb",
    "mov al, 0x03",
    "out dx, al",
    "mov dx, 0x3fa",
    "mov al, 0xc7",
    "out dx, al",
    "mov dx, 0x3fc",
    "mov al, 0x0b",
    "out dx, al",
    // 'E' means the ELF entry was reached after the UEFI handoff/page-table switch.
    "mov dx, 0x3f8",
    "mov al, 0x45",
    "out dx, al",
    "mov al, 0x0d",
    "out dx, al",
    "mov al, 0x0a",
    "out dx, al",
    // Do not depend on the EFI application's inherited stack after handoff.
    "lea rsp, [rip + __boot_stack_top]",
    "and rsp, -16",
    // 'S' means the kernel-owned boot stack is installed and aligned.
    "mov dx, 0x3f8",
    "mov al, 0x53",
    "out dx, al",
    "mov al, 0x0d",
    "out dx, al",
    "mov al, 0x0a",
    "out dx, al",
    // Bootloader contract: BootInfo arrives in RDI and is preserved into Rust.
    "call rust_kernel_main",
    ".Lhalt_uefi:",
    "cli",
    "hlt",
    "jmp .Lhalt_uefi",
    ".section .bss.stack, \"aw\", @nobits",
    ".balign 4096",
    ".global __boot_stack_bottom",
    "__boot_stack_bottom:",
    ".skip 65536",
    ".global __boot_stack_top",
    "__boot_stack_top:",
);

const COM1: u16 = 0x3F8;
struct Serial;

impl Serial {
    fn init(&mut self) {
        unsafe {
            port_out(COM1 + 1, 0x00);
            port_out(COM1 + 3, 0x80);
            port_out(COM1, 0x03);
            port_out(COM1 + 1, 0x00);
            port_out(COM1 + 3, 0x03);
            port_out(COM1 + 2, 0xC7);
            port_out(COM1 + 4, 0x0B);
        }
    }
    fn write_byte(&mut self, byte: u8) {
        for _ in 0..100_000 {
            if unsafe { port_in(COM1 + 5) } & 0x20 != 0 {
                unsafe { port_out(COM1, byte) };
                return;
            }
            core::hint::spin_loop();
        }
    }
}
impl Write for Serial {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        for byte in text.bytes() {
            if byte == b'\n' { self.write_byte(b'\r'); }
            self.write_byte(byte);
        }
        Ok(())
    }
}

unsafe fn port_out(port: u16, value: u8) {
    // SAFETY: privileged x86 port I/O; called only by early kernel diagnostics.
    unsafe { asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags)) };
}
unsafe fn port_in(port: u16) -> u8 {
    let value: u8;
    // SAFETY: privileged x86 port I/O; caller must execute at kernel privilege.
    unsafe { asm!("in al, dx", in("dx") port, out("al") value, options(nomem, nostack, preserves_flags)) };
    value
}
fn halt_forever() -> ! {
    loop { unsafe { asm!("cli; hlt", options(nomem, nostack)) }; }
}

// Early visual diagnostics use colored framebuffer bands until a text renderer
// and GOP geometry are part of the boot ABI. Each band is a persistent stage
// marker; a red band means the corresponding operation failed.
const FB_BACKGROUND: u32 = 0x0010_1828;
const FB_ACTIVE: u32 = 0x0050_D8_F0;
const FB_BOOTINFO_OK: u32 = 0x0000_C050;
const FB_MEMORY_OK: u32 = 0x00D0_A020;
const FB_PMM_OK: u32 = 0x0000_80E0;
const FB_CONTEXT_OK: u32 = 0x00D0_40C0;
const FB_SCHEDULER_OK: u32 = 0x00E0_E0_E0;
const FB_FAIL: u32 = 0x0000_3030;
const FB_PIXELS_PER_STATUS_LINE: usize = 32_768;

fn framebuffer_pixels(info: &archiaos_bootloader::bootinfo::BootInfo) -> Option<(*mut u32, usize)> {
    const MAX_LOW_CANONICAL_ADDRESS: u64 = 1u64 << 39;
    if info.framebuffer_base == 0 || info.framebuffer_size < 4 || info.framebuffer_base % 4 != 0 {
        return None;
    }
    let end = info.framebuffer_base.checked_add(info.framebuffer_size)?;
    if end > MAX_LOW_CANONICAL_ADDRESS {
        return None;
    }
    let pixels = usize::try_from(info.framebuffer_size / 4).ok()?;
    if pixels == 0 { return None; }
    Some((info.framebuffer_base as *mut u32, pixels))
}

fn paint_status_line(info: &archiaos_bootloader::bootinfo::BootInfo, line: usize, color: u32) {
    let Some((framebuffer, pixels)) = framebuffer_pixels(info) else { return; };
    let Some(start) = line.checked_mul(FB_PIXELS_PER_STATUS_LINE) else { return; };
    if start >= pixels { return; }
    let end = start.saturating_add(FB_PIXELS_PER_STATUS_LINE).min(pixels);
    for index in start..end {
        // SAFETY: the index is bounded by the firmware-reported framebuffer range.
        unsafe { ptr::write_volatile(framebuffer.add(index), color) };
    }
}

fn show_framebuffer_boot_status(info: &archiaos_bootloader::bootinfo::BootInfo) -> u8 {
    let Some((framebuffer, pixels)) = framebuffer_pixels(info) else {
        return 1;
    };
    for index in 0..pixels {
        // SAFETY: the index is bounded by the firmware-reported framebuffer range.
        unsafe { ptr::write_volatile(framebuffer.add(index), FB_BACKGROUND) };
    }
    paint_status_line(info, 0, FB_ACTIVE);
    0
}

#[no_mangle]
pub extern "C" fn rust_kernel_main(
    boot_info: *const archiaos_bootloader::bootinfo::BootInfo,
) -> ! {
    let mut serial = Serial;
    serial.init();
    // Install fatal exception diagnostics before touching firmware-provided data.
    // SAFETY: early boot is single-threaded and maskable interrupts remain disabled.
    unsafe { interrupts::install() };
    let _ = writeln!(serial, "ArchiaOS: x86-64 exception IDT installed (vectors 0-31)");
    let _ = writeln!(serial, "ArchiaOS: custom UEFI bootloader entered production kernel");
    let _ = writeln!(serial, "ArchiaOS: validating ARCHIABI BootInfo and UEFI memory map");

    // Validate the fixed BootInfo header before touching framebuffer metadata.
    // Paint the early status immediately after this bounded validation, before
    // CPUID/PCI discovery and PMM setup, so a later initialization failure still
    // leaves a visible indication on the physical monitor.
    if boot_info.is_null()
        || (boot_info as usize) % core::mem::align_of::<archiaos_bootloader::bootinfo::BootInfo>() != 0
    {
        let _ = writeln!(serial, "ArchiaOS: boot-stage=bootinfo-pointer-invalid");
        halt_forever();
    }
    let info = unsafe { &*boot_info };
    if archiaos_bootloader::bootinfo::validate_header(info).is_err() {
        let _ = writeln!(serial, "ArchiaOS: boot-stage=bootinfo-header-invalid");
        halt_forever();
    }
    let _ = writeln!(serial, "ArchiaOS: boot-stage=bootinfo-header-valid");
    let _ = writeln!(
        serial,
        "ArchiaOS: framebuffer candidate base={:#x} size={} bytes",
        info.framebuffer_base,
        info.framebuffer_size
    );
    let framebuffer_probe = show_framebuffer_boot_status(info);
    match framebuffer_probe {
        0 => { let _ = writeln!(serial, "ArchiaOS: framebuffer probe=written"); }
        1 => { let _ = writeln!(serial, "ArchiaOS: framebuffer probe=unavailable (GOP base is zero)"); }
        2 => { let _ = writeln!(serial, "ArchiaOS: framebuffer probe=rejected (range too small)"); }
        3 => { let _ = writeln!(serial, "ArchiaOS: framebuffer probe=rejected (range overflow)"); }
        4 => { let _ = writeln!(serial, "ArchiaOS: framebuffer probe=rejected (above low-canonical 512 GiB)"); }
        5 => { let _ = writeln!(serial, "ArchiaOS: framebuffer probe=rejected (unaligned base)"); }
        _ => { let _ = writeln!(serial, "ArchiaOS: framebuffer probe=rejected (size conversion failed)"); }
    }
    paint_status_line(info, 1, FB_BOOTINFO_OK);
    let _ = writeln!(serial, "ArchiaOS: boot-stage=early-framebuffer-probe-complete");

    // Read-only CPU inventory is the first hardware-telemetry primitive. Keep
    // it allocation-free and independent of sensor or device-driver support.
    let cpu_info = cpu::detect();
    if !cpu_info.has_sse41 {
        paint_status_line(info, 1, FB_FAIL);
        let _ = writeln!(serial, "ArchiaOS: unsupported CPU: SSE4.1 is required; refusing to continue");
        halt_forever();
    }
    let vendor = cpu::printable_field(&cpu_info.vendor);
    let brand = cpu::printable_field(&cpu_info.brand);
    let _ = writeln!(serial, "ArchiaOS: CPU vendor: {}", vendor);
    if !brand.is_empty() {
        let _ = writeln!(serial, "ArchiaOS: CPU brand: {}", brand);
    }
    let _ = writeln!(
        serial,
        "ArchiaOS: CPU family {:04x}, model {:04x}, stepping {}",
        cpu_info.family,
        cpu_info.model,
        cpu_info.stepping
    );
    let _ = writeln!(
        serial,
        "ArchiaOS: CPU features TSC={} SSE2={} SSE4.1={} SSE4.2={} AES={} XSAVE={} OSXSAVE={} AVX(hw/usable)={}/{} AVX2(hw/usable)={}/{} AVX512F(hw/usable)={}/{} FMA(hw/usable)={}/{} BMI1={} BMI2={}",
        cpu_info.has_tsc as u8,
        cpu_info.has_sse2 as u8,
        cpu_info.has_sse41 as u8,
        cpu_info.has_sse42 as u8,
        cpu_info.has_aesni as u8,
        cpu_info.has_xsave as u8,
        cpu_info.has_osxsave as u8,
        cpu_info.has_avx as u8,
        cpu_info.avx_state_usable as u8,
        cpu_info.has_avx2 as u8,
        cpu_info.avx2_usable as u8,
        cpu_info.has_avx512f as u8,
        cpu_info.avx512f_usable as u8,
        cpu_info.has_fma as u8,
        cpu_info.fma_usable as u8,
        cpu_info.has_bmi1 as u8,
        cpu_info.has_bmi2 as u8
    );
    let _ = writeln!(
        serial,
        "ArchiaOS: CPUID maximum basic={:#x}, extended={:#x}",
        cpu_info.max_basic_leaf,
        cpu_info.max_extended_leaf
    );

    // Read-only legacy PCI configuration discovery (bus 0 only). This does not
    // write to PCI device configuration registers; ECAM/ACPI extends coverage later.
    pci::scan_bus0(&mut serial);

    // The custom loader maps the handoff allocation and provides this pointer in
    // RDI. The initializer verifies alignment, version, structure size, bounds,
    // descriptor stride and every memory range before seeding the PMM bitmap.
    let stats = match unsafe { memory::initialize_from_bootinfo(&mut serial, boot_info) } {
        Ok(stats) => stats,
        Err(error) => {
            paint_status_line(info, 2, FB_FAIL);
            let _ = writeln!(serial, "ArchiaOS: UEFI memory initialization failed: {:?}", error);
            halt_forever();
        }
    };
    paint_status_line(info, 2, FB_MEMORY_OK);
    let _ = writeln!(serial, "ArchiaOS: UEFI BootInfo validated");
    let info = unsafe { &*boot_info };
    if info.framebuffer_size == 0 {
        let _ = writeln!(serial, "ArchiaOS: UEFI GOP framebuffer unavailable (headless mode)");
    } else {
        let _ = writeln!(
            serial,
            "ArchiaOS: UEFI GOP framebuffer base={:#x} size={} bytes",
            info.framebuffer_base,
            info.framebuffer_size
        );
    }
    let _ = writeln!(serial, "ArchiaOS: UEFI memory map entries: {}", stats.map_entries);
    let _ = writeln!(serial, "ArchiaOS: conventional memory ranges: {}, bytes: {}", stats.usable_ranges, stats.usable_bytes);
    let _ = writeln!(serial, "ArchiaOS: free physical pages below 4 GiB: {}", stats.free_pages);

    if let Err(error) = memory::self_test(&mut serial) {
        paint_status_line(info, 3, FB_FAIL);
        let _ = writeln!(serial, "ArchiaOS: UEFI PMM self-test failed: {:?}", error);
        halt_forever();
    }
    paint_status_line(info, 3, FB_PMM_OK);
    let _ = writeln!(serial, "ArchiaOS: UEFI PMM allocation/free self-test passed");

    match context::self_test() {
        Ok(()) => {
            paint_status_line(info, 4, FB_CONTEXT_OK);
            let _ = writeln!(serial, "ArchiaOS: x86-64 task context and initial stack self-test passed");
        }
        Err(()) => {
            paint_status_line(info, 4, FB_FAIL);
            let _ = writeln!(serial, "ArchiaOS: x86-64 task context self-test failed");
            halt_forever();
        }
    }

    // Validate the state model and then perform a real two-task round-robin
    // context-switch test. Each task has a separate static stack and CPU context;
    // both must yield, resume after suspension, and exit back to the boot stack.
    match scheduler::Scheduler::self_test() {
        Ok(()) => {
            let _ = writeln!(serial, "ArchiaOS: cooperative scheduler state self-test passed");
        }
        Err(error) => {
            paint_status_line(info, 5, FB_FAIL);
            let _ = writeln!(serial, "ArchiaOS: scheduler state self-test failed: {:?}", error);
            halt_forever();
        }
    }
    match scheduler::runtime_self_test() {
        Ok(()) => {
            paint_status_line(info, 5, FB_SCHEDULER_OK);
            let _ = writeln!(serial, "ArchiaOS: two-task cooperative context-switch self-test passed");
        }
        Err(error) => {
            paint_status_line(info, 5, FB_FAIL);
            let _ = writeln!(serial, "ArchiaOS: two-task scheduler runtime self-test failed: {:?}", error);
            halt_forever();
        }
    }
    let _ = writeln!(serial, "ArchiaOS: UEFI kernel entry validated");
    halt_forever();
}

/// Fatal early-kernel exception dispatcher called by the assembly stubs.
/// The stubs normalize the stack to pass the vector and CPU error code.
#[no_mangle]
pub extern "C" fn archiaos_exception_dispatch(vector: u64, error_code: u64) -> ! {
    let mut serial = Serial;
    serial.init();
    let name = match vector {
        0 => "divide error", 1 => "debug", 2 => "non-maskable interrupt",
        3 => "breakpoint", 4 => "overflow", 5 => "bound range",
        6 => "invalid opcode", 7 => "device not available", 8 => "double fault",
        9 => "coprocessor segment overrun", 10 => "invalid TSS",
        11 => "segment not present", 12 => "stack-segment fault",
        13 => "general protection fault", 14 => "page fault",
        16 => "x87 floating-point exception", 17 => "alignment check",
        18 => "machine check", 19 => "SIMD floating-point exception",
        20 => "virtualization exception", 21 => "control protection exception",
        29 => "VMM communication exception", 30 => "security exception",
        _ => "reserved or unclassified exception",
    };
    let _ = writeln!(serial, "ArchiaOS: FATAL x86-64 exception vector={} ({})", vector, name);
    let _ = writeln!(serial, "ArchiaOS: exception error_code={:#018x}", error_code);
    if vector == 14 {
        let fault_address: u64;
        // SAFETY: CR2 is read only in the page-fault diagnostic path at kernel privilege.
        unsafe { asm!("mov {}, cr2", out(reg) fault_address, options(nomem, nostack, preserves_flags)) };
        let _ = writeln!(serial, "ArchiaOS: page-fault address={:#018x}", fault_address);
    }
    halt_forever()
}

#[panic_handler]
fn panic(info: &PanicInfo<'_>) -> ! {
    let mut serial = Serial;
    serial.init();
    let _ = writeln!(serial, "ArchiaOS: PANIC: {}", info);
    halt_forever();
}
