#![no_std]
#![no_main]

use core::arch::{asm, global_asm};
use core::panic::PanicInfo;
use core::ptr::{read_volatile, write_volatile};

const BOOT_INFO_MAGIC: u64 = 0x4942_4149_4843_5241;
const BOOT_INFO_VERSION: u32 = 1;
const MIN_MEMORY_DESCRIPTOR_SIZE: u64 = 40;
const MAX_MEMORY_MAP_BYTES: u64 = 1024 * 1024;
const FDT_MAGIC: u32 = 0xd00d_feed;

#[repr(C)]
struct BootInfo {
    magic: u64,
    version: u32,
    struct_size: u32,
    kernel_entry: u64,
    memory_map: *const u8,
    memory_map_size: u64,
    memory_map_descriptor_size: u64,
    memory_map_descriptor_version: u32,
    flags: u32,
    acpi_rsdp: u64,
    device_tree: u64,
    framebuffer_base: u64,
    framebuffer_size: u64,
}

global_asm!(
    ".section .text.boot, \"ax\"",
    ".global _start",
    ".type _start, %function",
    "_start:",
    "msr daifset, #0xf",
    // Preserve the first-argument register while switching to our private stack.
    "mov x19, x0",
    "adrp x1, __stack_top",
    "add x1, x1, :lo12:__stack_top",
    "mov sp, x1",
    "mov x0, x19",
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
            if byte == b'\\n' {
                self.write_byte(b'\\r');
            }
            self.write_byte(byte);
        }
    }
}

fn validate_boot_info(info: *const BootInfo, serial: &mut Serial) {
    if info.is_null() {
        serial.write_str("ArchiaOS AArch64: no UEFI BootInfo (direct QEMU boot)\\n");
        return;
    }

    // QEMU's direct -kernel path supplies a Device Tree pointer in x0, whereas
    // our UEFI loader supplies the versioned ARCHIABI structure. Read only the
    // first word before distinguishing those two entry contracts.
    let magic = unsafe { read_volatile(info.cast::<u64>()) };
    if magic != BOOT_INFO_MAGIC {
        let fdt_magic = unsafe { u32::from_be(read_volatile(info.cast::<u32>())) };
        if fdt_magic == FDT_MAGIC {
            serial.write_str("ArchiaOS AArch64: direct QEMU Device Tree detected\\n");
        } else {
            serial.write_str("ArchiaOS AArch64: no UEFI BootInfo (direct QEMU boot)\\n");
        }
        return;
    }

    let b = unsafe { &*info };
    if b.version != BOOT_INFO_VERSION
        || b.struct_size != core::mem::size_of::<BootInfo>() as u32
        || b.kernel_entry == 0
        || b.kernel_entry % 4 != 0
        || b.memory_map.is_null()
        || b.memory_map_size == 0
        || b.memory_map_size > MAX_MEMORY_MAP_BYTES
        || b.memory_map_descriptor_size < MIN_MEMORY_DESCRIPTOR_SIZE
        || b.memory_map_descriptor_size > 4096
        || b.memory_map_size % b.memory_map_descriptor_size != 0
        || b.device_tree == 0
    {
        serial.write_str("ArchiaOS AArch64: BootInfo validation failed\\n");
        halt_forever();
    }

    let count = (b.memory_map_size / b.memory_map_descriptor_size) as usize;
    let stride = b.memory_map_descriptor_size as usize;
    if count == 0 || count > 65_536 {
        serial.write_str("ArchiaOS AArch64: memory-map entry count rejected\\n");
        halt_forever();
    }

    // UEFI publishes the flattened Device Tree via EFI_DTB_TABLE_GUID. Its
    // header is big-endian, unlike the native-endian BootInfo fields.
    let fdt_base = b.device_tree as usize as *const u8;
    let fdt_magic = unsafe { u32::from_be(read_volatile(fdt_base.cast::<u32>())) };
    let fdt_size = unsafe { u32::from_be(read_volatile(fdt_base.add(4).cast::<u32>())) } as u64;
    if fdt_magic != FDT_MAGIC || !(40..=16 * 1024 * 1024).contains(&fdt_size) {
        serial.write_str("ArchiaOS AArch64: Device Tree header rejected\\n");
        halt_forever();
    }

    // Touch every descriptor's type field to verify the handoff map is readable.
    for i in 0..count {
        let descriptor = unsafe { b.memory_map.add(i * stride) };
        let _memory_type = unsafe { read_volatile(descriptor.cast::<u32>()) };
    }

    serial.write_str("ArchiaOS AArch64: BootInfo validated\\n");
    serial.write_str("ArchiaOS AArch64: Device Tree header validated\\n");
    serial.write_str("ArchiaOS AArch64: UEFI memory-map descriptors readable\\n");
}

#[no_mangle]
pub extern "C" fn rust_kernel_main(info: *const BootInfo) -> ! {
    let mut serial = Serial;
    serial.init();
    serial.write_str("ArchiaOS AArch64: kernel entry reached\\n");
    serial.write_str("ArchiaOS AArch64: PL011 diagnostics initialized\\n");
    validate_boot_info(info, &mut serial);
    serial.write_str("ArchiaOS AArch64: bring-up prototype; MMU and exceptions pending\\n");
    halt_forever();
}

fn halt_forever() -> ! {
    loop {
        unsafe { asm!("msr daifset, #0xf", "wfe", options(nomem, nostack, preserves_flags)) };
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    let mut serial = Serial;
    serial.init();
    serial.write_str("ArchiaOS AArch64: PANIC\\n");
    halt_forever();
}
