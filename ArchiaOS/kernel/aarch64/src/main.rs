#![no_std]
#![no_main]

use core::arch::{asm, global_asm};
use core::panic::PanicInfo;
use core::ptr::{read_volatile, write_volatile};

global_asm!(
    ".section .text.boot, \"ax\"",
    ".global _start",
    ".type _start, %function",
    "_start:",
    "msr daifset, #0xf",
    "adrp x0, __stack_top",
    "add x0, x0, :lo12:__stack_top",
    "mov sp, x0",
    "bl rust_kernel_main",
    "1:",
    "wfe",
    "b 1b",
    ".section .bss.stack, \"aw\", %nobits",
    ".align 12",
    ".global __stack_bottom",
    "__stack_bottom:",
    ".skip 65536",
    ".global __stack_top",
    "__stack_top:",
);

const PL011_BASE: usize = 0x0900_0000;
const UART_DR: usize = PL011_BASE;
const UART_FR: usize = PL011_BASE + 0x18;
const UART_IBRD: usize = PL011_BASE + 0x24;
const UART_FBRD: usize = PL011_BASE + 0x28;
const UART_LCR_H: usize = PL011_BASE + 0x2C;
const UART_CR: usize = PL011_BASE + 0x30;
const UART_ICR: usize = PL011_BASE + 0x44;
const UART_FR_TXFF: u32 = 1 << 5;

struct Serial;

impl Serial {
    fn init(&mut self) {
        // QEMU virt exposes a PL011 UART at 0x09000000 with a 24 MHz clock.
        // This early prototype is platform-specific and is not a generic ARM64
        // console driver.
        unsafe {
            write_volatile(UART_CR as *mut u32, 0);
            write_volatile(UART_ICR as *mut u32, 0x7ff);
            write_volatile(UART_IBRD as *mut u32, 13);
            write_volatile(UART_FBRD as *mut u32, 1);
            write_volatile(UART_LCR_H as *mut u32, 0x70);
            write_volatile(UART_CR as *mut u32, 0x301);
        }
    }

    fn write_byte(&mut self, byte: u8) {
        // Bound the wait so a missing or misconfigured UART cannot hang forever.
        for _ in 0..1_000_000 {
            if unsafe { read_volatile(UART_FR as *const u32) } & UART_FR_TXFF == 0 {
                unsafe { write_volatile(UART_DR as *mut u32, byte as u32) };
                return;
            }
            core::hint::spin_loop();
        }
    }

    fn write_str(&mut self, text: &str) {
        for byte in text.bytes() {
            if byte == b'\n' {
                self.write_byte(b'\r');
            }
            self.write_byte(byte);
        }
    }
}

#[no_mangle]
pub extern "C" fn rust_kernel_main() -> ! {
    let mut serial = Serial;
    serial.init();
    serial.write_str("ArchiaOS AArch64: kernel entry reached\n");
    serial.write_str("ArchiaOS AArch64: PL011 diagnostics initialized\n");
    serial.write_str("ArchiaOS AArch64: bring-up prototype; MMU and exceptions pending\n");
    halt_forever();
}

fn halt_forever() -> ! {
    loop {
        unsafe { asm!("wfe", options(nomem, nostack, preserves_flags)) };
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    let mut serial = Serial;
    serial.init();
    serial.write_str("ArchiaOS AArch64: PANIC\n");
    halt_forever();
}
