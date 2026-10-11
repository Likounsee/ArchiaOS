#![no_std]
#![no_main]

mod memory;

use core::arch::{asm, global_asm};
use core::fmt::{self, Write};
use core::panic::PanicInfo;

#[used]
#[link_section = ".limine_requests_start"]
static LIMINE_REQUESTS_START_MARKER: [u64; 4] = [
    0xf6b8_f4b3_9de7_d1ae,
    0xfab9_1a69_40fc_b9cf,
    0x785c_6ed0_15d3_e316,
    0x181e_920a_7852_b9d9,
];

#[used]
#[link_section = ".limine_requests_end"]
static LIMINE_REQUESTS_END_MARKER: [u64; 2] = [
    0xadc0_e053_1bb1_0d03,
    0x9572_709f_3176_4c62,
];

global_asm!(
    ".section .text.boot, \"ax\"",
    ".global _start",
    ".type _start, @function",
    "_start:",
    "cli",
    "cld",
    "and rsp, -16",
    "call rust_kernel_main",
    ".Lhalt:",
    "cli",
    "hlt",
    "jmp .Lhalt",
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

#[no_mangle]
pub extern "C" fn rust_kernel_main() -> ! {
    let mut serial = Serial;
    serial.init();
    let _ = writeln!(serial, "ArchiaOS: Limine entered x86_64 kernel");
    let _ = writeln!(serial, "ArchiaOS: serial diagnostics are online");

    let _ = writeln!(serial, "ArchiaOS: initializing physical page allocator");
    let stats = match memory::initialize(&mut serial) {
        Ok(stats) => stats,
        Err(error) => {
            let _ = writeln!(serial, "ArchiaOS: memory initialization failed: {:?}", error);
            halt_forever();
        }
    };
    let _ = writeln!(serial, "ArchiaOS: memory map entries validated: {}", stats.map_entries);
    let _ = writeln!(serial, "ArchiaOS: usable memory ranges: {}, bytes: {}", stats.usable_ranges, stats.usable_bytes);
    let _ = writeln!(serial, "ArchiaOS: free physical pages below 4 GiB: {}", stats.free_pages);

    if let Err(error) = memory::self_test(&mut serial) {
        let _ = writeln!(serial, "ArchiaOS: PMM self-test failed: {:?}", error);
        halt_forever();
    }
    let _ = writeln!(serial, "ArchiaOS: PMM allocation/free self-test passed");
    let _ = writeln!(serial, "ArchiaOS: interrupts and scheduling are not initialized yet");
    halt_forever();
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    let mut serial = Serial;
    serial.init();
    let _ = writeln!(serial, "ArchiaOS: PANIC");
    halt_forever();
}
