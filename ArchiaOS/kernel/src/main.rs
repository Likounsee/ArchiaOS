#![no_std]
#![no_main]

mod memory;
mod cpu;
mod pci;

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
    // Do not depend on the EFI application's inherited stack after handoff.
    "lea rsp, [rip + __boot_stack_top]",
    "and rsp, -16",
    // 'S' means the kernel-owned boot stack is installed and aligned.
    "mov dx, 0x3f8",
    "mov al, 0x53",
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

// Temporary bring-up display: until the text renderer is ready, paint a
// dark background and a bright cyan band so a successful kernel handoff is
// visible on the physical monitor (kernel diagnostics continue on COM1).
fn show_framebuffer_boot_status(info: &archiaos_bootloader::bootinfo::BootInfo) {
    const MAX_PHYSICAL_ADDRESS: u64 = 1u64 << 32;
    if info.framebuffer_base == 0 || info.framebuffer_size < 4 {
        return;
    }
    let Some(end) = info.framebuffer_base.checked_add(info.framebuffer_size) else {
        return;
    };
    if end > MAX_PHYSICAL_ADDRESS || info.framebuffer_base % 4 != 0 {
        return;
    }
    let pixels = (info.framebuffer_size / 4) as usize;
    let framebuffer = info.framebuffer_base as *mut u32;
    let band_pixels = pixels.min(32_768);
    for index in 0..pixels {
        // GOP pixel modes use 32-bit pixels. These colors remain visible in
        // both common RGB/BGR channel orderings.
        let color = if index < band_pixels { 0x00_50_D8_F0 } else { 0x00_10_18_28 };
        unsafe { ptr::write_volatile(framebuffer.add(index), color) };
    }
}

#[no_mangle]
pub extern "C" fn rust_kernel_main(
    boot_info: *const archiaos_bootloader::bootinfo::BootInfo,
) -> ! {
    let mut serial = Serial;
    serial.init();
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
    show_framebuffer_boot_status(info);
    let _ = writeln!(serial, "ArchiaOS: boot-stage=early-framebuffer-probe-complete");

    // Read-only CPU inventory is the first hardware-telemetry primitive. Keep
    // it allocation-free and independent of sensor or device-driver support.
    let cpu_info = cpu::detect();
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
        "ArchiaOS: CPU features TSC={} SSE2={} AES={} XSAVE={} AVX={} AVX2={} (hardware flags only)",
        cpu_info.has_tsc as u8,
        cpu_info.has_sse2 as u8,
        cpu_info.has_aesni as u8,
        cpu_info.has_xsave as u8,
        cpu_info.has_avx as u8,
        cpu_info.has_avx2 as u8
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
            let _ = writeln!(serial, "ArchiaOS: UEFI memory initialization failed: {:?}", error);
            halt_forever();
        }
    };
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
        let _ = writeln!(serial, "ArchiaOS: UEFI PMM self-test failed: {:?}", error);
        halt_forever();
    }
    let _ = writeln!(serial, "ArchiaOS: UEFI PMM allocation/free self-test passed");
    let _ = writeln!(serial, "ArchiaOS: UEFI kernel entry validated");
    halt_forever();
}

#[panic_handler]
fn panic(info: &PanicInfo<'_>) -> ! {
    let mut serial = Serial;
    serial.init();
    let _ = writeln!(serial, "ArchiaOS: PANIC: {}", info);
    halt_forever();
}
