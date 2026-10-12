#![no_std]
#![no_main]

use core::arch::{asm, global_asm};
use core::panic::PanicInfo;
use core::ptr::{read_volatile, write_volatile};
use archiaos_bootloader::bootinfo::{BootInfo, BOOT_INFO_MAGIC, BOOT_INFO_VERSION};
use archiaos_bootloader::device_tree::{validate_tree as validate_fdt_tree, FDT_MAGIC, HEADER_SIZE as FDT_HEADER_SIZE, MAX_TOTAL_SIZE as MAX_FDT_SIZE};
use archiaos_bootloader::memory_map::UefiMemoryMap;

const MIN_MEMORY_DESCRIPTOR_SIZE: u64 = 40;
const MAX_MEMORY_MAP_BYTES: u64 = 1024 * 1024;

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

unsafe fn validate_fdt_at_address(address: u64) -> bool {
    if address == 0 || address > usize::MAX as u64 {
        return false;
    }
    let base = address as usize as *const u8;
    // Firmware/QEMU supplies a valid pointer to an FDT object. Read total_size
    // from the header, cap the slice, then invoke the shared bounds-checked parser.
    let total = unsafe { u32::from_be(read_volatile(base.add(4).cast::<u32>())) as usize };
    if total < FDT_HEADER_SIZE || total > MAX_FDT_SIZE {
        return false;
    }
    let image = unsafe { core::slice::from_raw_parts(base, total) };
    validate_fdt_tree(image).is_ok()
}

fn validate_boot_info(info: *const BootInfo, serial: &mut Serial) {
    if info.is_null() {
        serial.write_str("ArchiaOS AArch64: no UEFI BootInfo (direct QEMU boot)\\n");
        return;
    }

    // QEMU's direct -kernel path supplies a Device Tree pointer in x0, whereas
    // our UEFI loader supplies the versioned ARCHIABI structure.
    let magic = unsafe { read_volatile(info.cast::<u64>()) };
    if magic != BOOT_INFO_MAGIC {
        let fdt_magic = unsafe { u32::from_be(read_volatile(info.cast::<u32>())) };
        if fdt_magic == FDT_MAGIC {
            serial.write_str("ArchiaOS AArch64: direct QEMU Device Tree detected\\n");
            if unsafe { validate_fdt_at_address(info as usize as u64) } {
                serial.write_str("ArchiaOS AArch64: direct QEMU Device Tree header validated\\n");
            } else {
                serial.write_str("ArchiaOS AArch64: direct QEMU Device Tree header rejected\\n");
                halt_forever();
            }
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
    if !unsafe { validate_fdt_at_address(b.device_tree) } {
        serial.write_str("ArchiaOS AArch64: Device Tree header rejected\\n");
        halt_forever();
    }

    // The shared parser validates every descriptor's stride, physical alignment,
    // non-zero page range and checked physical end before the kernel trusts the map.
    let map_bytes = unsafe {
        core::slice::from_raw_parts(b.memory_map, b.memory_map_size as usize)
    };
    let memory_map = match UefiMemoryMap::parse(map_bytes, stride) {
        Ok(map) if map.len() == count => map,
        _ => {
            serial.write_str("ArchiaOS AArch64: UEFI memory map rejected\\n");
            halt_forever();
        }
    };
    if memory_map.is_empty() {
        serial.write_str("ArchiaOS AArch64: UEFI memory map is empty\\n");
        halt_forever();
    }

    serial.write_str("ArchiaOS AArch64: BootInfo validated\\n");
    serial.write_str("ArchiaOS AArch64: Device Tree header validated\\n");
    serial.write_str("ArchiaOS AArch64: UEFI memory-map descriptors validated\\n");
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
