#![no_std]
#![no_main]

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
#[link_section = ".limine_requests"]
static mut LIMINE_BASE_REVISION: [u64; 3] = [
    0xf956_2b2d_5c95_a6c8,
    0x6a7b_3849_4453_6bdc,
    6,
];

#[repr(C)]
struct LimineMemmapEntry {
    base: u64,
    length: u64,
    kind: u64,
}

#[repr(C)]
struct LimineMemmapResponse {
    revision: u64,
    entry_count: u64,
    entries: *mut *mut LimineMemmapEntry,
}

#[repr(C)]
struct LimineMemmapRequest {
    id: [u64; 4],
    revision: u64,
    response: *mut LimineMemmapResponse,
}

#[used]
#[link_section = ".limine_requests"]
static mut LIMINE_MEMMAP_REQUEST: LimineMemmapRequest = LimineMemmapRequest {
    id: [
        0xc7b1_dd30_df4c_8b88,
        0x0a82_e883_a194_f07b,
        0x67cf_3d9d_378a_806f,
        0xe304_acdf_c50c_3c62,
    ],
    revision: 0,
    response: core::ptr::null_mut(),
};

#[used]
#[link_section = ".limine_requests_end"]
static LIMINE_REQUESTS_END_MARKER: [u64; 2] = [
    0xadc0_e053_1bb1_0d03,
    0x9572_709f_3176_4c62,
];

// Limine transfers control to _start without a normal Rust call. Align the
// bootloader-provided stack and call Rust with the x86-64 SysV ABI alignment.
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
const MAX_MEMMAP_ENTRIES: u64 = 4096;
const LIMINE_MEMMAP_USABLE: u64 = 0;

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
        // Bound the poll so an absent serial device cannot stall early boot forever.
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
            if byte == b'\n' {
                self.write_byte(b'\r');
            }
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
    loop {
        // SAFETY: this is a terminal kernel error path with interrupts disabled.
        unsafe { asm!("cli; hlt", options(nomem, nostack)) };
    }
}

fn inspect_memory_map(serial: &mut Serial) -> bool {
    // Limine writes this request's response pointer before entering the kernel.
    let base_revision_supported = unsafe {
        core::ptr::read_volatile(core::ptr::addr_of!(LIMINE_BASE_REVISION[2])) == 0
    };
    if !base_revision_supported {
        let _ = writeln!(serial, "ArchiaOS: unsupported Limine base revision");
        return false;
    }

    let response_ptr = unsafe {
        core::ptr::read_volatile(core::ptr::addr_of!(LIMINE_MEMMAP_REQUEST.response))
    };
    if response_ptr.is_null() {
        let _ = writeln!(serial, "ArchiaOS: missing Limine memory map response");
        return false;
    }

    // SAFETY: response_ptr is supplied by Limine; validate its count and array pointer
    // before reading any entries. The bootloader response remains live during boot.
    let response = unsafe { &*response_ptr };
    if response.entry_count == 0
        || response.entry_count > MAX_MEMMAP_ENTRIES
        || response.entries.is_null()
    {
        let _ = writeln!(serial, "ArchiaOS: invalid Limine memory map metadata");
        return false;
    }

    let mut usable_ranges = 0u64;
    let mut usable_bytes = 0u64;
    for index in 0..response.entry_count {
        // SAFETY: the protocol defines entries as a contiguous pointer array. The
        // count is bounded above; individual pointers and ranges are checked below.
        let entry_ptr = unsafe { core::ptr::read_volatile(response.entries.add(index as usize)) };
        if entry_ptr.is_null() {
            let _ = writeln!(serial, "ArchiaOS: null memory map entry");
            return false;
        }
        // SAFETY: the entry pointer came from Limine's response array and is non-null.
        let entry = unsafe { core::ptr::read_volatile(entry_ptr) };
        if entry.length == 0 || entry.base.checked_add(entry.length).is_none() {
            let _ = writeln!(serial, "ArchiaOS: invalid memory map range");
            return false;
        }
        if entry.kind == LIMINE_MEMMAP_USABLE {
            usable_ranges += 1;
            usable_bytes = match usable_bytes.checked_add(entry.length) {
                Some(total) => total,
                None => {
                    let _ = writeln!(serial, "ArchiaOS: usable memory size overflow");
                    return false;
                }
            };
        }
    }

    let _ = writeln!(serial, "ArchiaOS: memory map entries validated: {}", response.entry_count);
    let _ = writeln!(serial, "ArchiaOS: usable memory ranges: {}, bytes: {}", usable_ranges, usable_bytes);
    true
}

#[no_mangle]
pub extern "C" fn rust_kernel_main() -> ! {
    let mut serial = Serial;
    serial.init();
    let _ = writeln!(serial, "ArchiaOS: Limine entered x86_64 kernel");
    let _ = writeln!(serial, "ArchiaOS: serial diagnostics are online");

    if !inspect_memory_map(&mut serial) {
        halt_forever();
    }

    let _ = writeln!(serial, "ArchiaOS: physical allocator and interrupts are not initialized yet");
    halt_forever();
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    let mut serial = Serial;
    serial.init();
    let _ = writeln!(serial, "ArchiaOS: PANIC");
    halt_forever();
}