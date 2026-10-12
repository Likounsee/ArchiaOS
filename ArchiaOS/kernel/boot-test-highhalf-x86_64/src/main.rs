#![no_std]
#![no_main]

use archiaos_bootloader::bootinfo::{BootInfo, BOOT_INFO_MAGIC, BOOT_INFO_VERSION};
use archiaos_bootloader::memory_map::UefiMemoryMap;
use core::arch::{asm, global_asm};
use core::fmt::{self, Write};
use core::panic::PanicInfo;

const COM1: u16 = 0x3f8;
const MIN_DESC: u64 = 40;
const HIGH_HALF_BASE: u64 = 0xffff_ffff_8000_0000;

global_asm!(
    ".section .text.boot, \"ax\"",
    ".global _start",
    ".type _start, @function",
    "_start:",
    "cli",
    "cld",
    "lea rsp, [rip + __stack_top]",
    "and rsp, -16",
    "call rust_kernel_main",
    "1:",
    "cli",
    "hlt",
    "jmp 1b",
    ".section .bss.stack, \"aw\", @nobits",
    ".balign 4096",
    ".global __stack_bottom",
    "__stack_bottom:",
    ".skip 65536",
    ".global __stack_top",
    "__stack_top:",
);

struct Serial;

impl Serial {
    fn init(&mut self) {
        unsafe {
            port_out(COM1 + 1, 0);
            port_out(COM1 + 3, 0x80);
            port_out(COM1, 3);
            port_out(COM1 + 1, 0);
            port_out(COM1 + 3, 3);
            port_out(COM1 + 2, 0xc7);
            port_out(COM1 + 4, 0x0b);
        }
    }

    fn byte(&mut self, byte: u8) {
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
                self.byte(b'\r');
            }
            self.byte(byte);
        }
        Ok(())
    }
}

unsafe fn port_out(port: u16, value: u8) {
    unsafe {
        asm!(
            "out dx, al",
            in("dx") port,
            in("al") value,
            options(nomem, nostack, preserves_flags)
        );
    }
}

unsafe fn port_in(port: u16) -> u8 {
    let value: u8;
    unsafe {
        asm!(
            "in al, dx",
            in("dx") port,
            out("al") value,
            options(nomem, nostack, preserves_flags)
        );
    }
    value
}

#[no_mangle]
pub extern "C" fn rust_kernel_main(info: *const BootInfo) -> ! {
    let mut serial = Serial;
    serial.init();
    let _ = writeln!(serial, "ArchiaOS high-half test kernel: entry reached");

    let current_rip: u64;
    unsafe {
        asm!("lea {}, [rip]", out(reg) current_rip, options(nomem, nostack, preserves_flags));
    }
    if current_rip < HIGH_HALF_BASE {
        let _ = writeln!(serial, "ArchiaOS high-half test kernel: instruction pointer is not high-half");
        halt();
    }
    if info.is_null() {
        let _ = writeln!(serial, "ArchiaOS high-half test kernel: BootInfo pointer missing");
        halt();
    }
    let boot_info = unsafe { &*info };
    if boot_info.magic != BOOT_INFO_MAGIC
        || boot_info.version != BOOT_INFO_VERSION
        || boot_info.struct_size != core::mem::size_of::<BootInfo>() as u32
        || boot_info.kernel_entry < HIGH_HALF_BASE
        || boot_info.memory_map.is_null()
        || boot_info.memory_map_size == 0
        || boot_info.memory_map_descriptor_size < MIN_DESC
        || boot_info.memory_map_descriptor_size > 4096
        || boot_info.memory_map_size % boot_info.memory_map_descriptor_size != 0
        || boot_info.memory_map_size > 1024 * 1024
    {
        let _ = writeln!(serial, "ArchiaOS high-half test kernel: BootInfo validation failed");
        halt();
    }

    let descriptor_size = boot_info.memory_map_descriptor_size as usize;
    let descriptor_count = (boot_info.memory_map_size / boot_info.memory_map_descriptor_size) as usize;
    if descriptor_count == 0 || descriptor_count > 65_536 {
        let _ = writeln!(serial, "ArchiaOS high-half test kernel: descriptor count rejected");
        halt();
    }

    let map_bytes = unsafe {
        core::slice::from_raw_parts(
            boot_info.memory_map,
            boot_info.memory_map_size as usize,
        )
    };
    let memory_map = match UefiMemoryMap::parse(map_bytes, descriptor_size) {
        Ok(map) if map.len() == descriptor_count => map,
        _ => {
            let _ = writeln!(serial, "ArchiaOS high-half test kernel: memory map rejected");
            halt();
        }
    };
    if memory_map.conventional_page_count().map(|pages| pages > 0) != Ok(true) {
        let _ = writeln!(serial, "ArchiaOS high-half test kernel: no conventional pages found");
        halt();
    }

    let _ = writeln!(serial, "ArchiaOS high-half test kernel: executing at {:#018x}", current_rip);
    let _ = writeln!(serial, "ArchiaOS high-half test kernel: BootInfo and identity map validated");
    let _ = writeln!(serial, "ArchiaOS high-half test kernel: high-half page-table handoff passed");
    halt()
}

fn halt() -> ! {
    loop {
        unsafe { asm!("cli; hlt", options(nomem, nostack)) };
    }
}

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    let mut serial = Serial;
    serial.init();
    let _ = writeln!(serial, "ArchiaOS high-half test kernel: PANIC");
    halt()
}
