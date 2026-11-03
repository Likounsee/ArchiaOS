#![no_std]
#![no_main]

mod memory_ranges;
mod boot_status;
mod framebuffer;
mod memory;
mod vmm;
mod heap;
mod address_space;
mod cpu;
mod cpu_features;
mod simd;
mod pci;
mod scheduler;
mod scheduler_topology;
mod context;
mod interrupts;
mod interrupt_vectors;
mod acpi_madt;
mod apic;
mod sync;

/// Legacy dual 8259 PIC setup. All lines stay masked until matching IDT
/// handlers and an interrupt-safe wakeup path are implemented.
mod pic;

use core::arch::{asm, global_asm};
use core::fmt::{self, Write};
use core::panic::PanicInfo;

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

mod console;
use console::Serial;

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

mod apic_bringup;

fn halt_forever() -> ! {
    loop { unsafe { asm!("cli; hlt", options(nomem, nostack)) }; }
}

mod boot;

/// Fatal early-kernel exception dispatcher called by the assembly stubs.
/// The stubs normalize the stack to pass the vector and CPU error code.
#[no_mangle]
pub extern "C" fn archiaos_exception_dispatch(vector: u64, error_code: u64, instruction_pointer: u64) -> ! {
    let mut serial = Serial;
    serial.init();

    // Until a returning IRQ dispatcher exists, classify legacy PIC vectors
    // accurately and stop without sending an EOI from this fatal path.
    if vector <= u8::MAX as u64 {
        if let interrupts::InterruptVectorClass::LegacyPicIrq(irq) =
            interrupts::classify_interrupt_vector(vector as u8)
        {
            let _ = writeln!(
                serial,
                "ArchiaOS: FATAL unhandled legacy PIC IRQ vector={} irq={} (no EOI; interrupts remain disabled)",
                vector,
                irq
            );
            let _ = writeln!(serial, "ArchiaOS: interrupted RIP={:#018x}", instruction_pointer);
            halt_forever();
        }
    }

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
    let _ = writeln!(serial, "ArchiaOS: exception RIP={:#018x}", instruction_pointer);
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
