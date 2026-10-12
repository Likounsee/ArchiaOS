#![no_std]
#![no_main]

mod memory;

use core::arch::{asm, global_asm};
use core::panic::PanicInfo;
use core::ptr::{read_volatile, write_volatile};
use archiaos_bootloader::acpi::{parse_rsdp, parse_sdt_header, parse_spcr_uart_base, RsdpInfo, MAX_RSDP_SIZE, MAX_SDT_SIZE, SDT_HEADER_SIZE};
use archiaos_bootloader::bootinfo::{BootInfo, BOOT_INFO_MAGIC, BOOT_INFO_VERSION};
use archiaos_bootloader::device_tree::{find_compatible_reg, validate_tree as validate_fdt_tree, FDT_MAGIC, HEADER_SIZE as FDT_HEADER_SIZE, MAX_TOTAL_SIZE as MAX_FDT_SIZE};
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
    ".section .text.vectors, \"ax\"",
    ".balign 2048",
    ".global __exception_vectors",
    ".type __exception_vectors, %function",
    "__exception_vectors:",
    "mov x0, #0", "b aarch64_exception_vector_handler", ".space 120",
    "mov x0, #1", "b aarch64_exception_vector_handler", ".space 120",
    "mov x0, #2", "b aarch64_exception_vector_handler", ".space 120",
    "mov x0, #3", "b aarch64_exception_vector_handler", ".space 120",
    "mov x0, #4", "b aarch64_exception_vector_handler", ".space 120",
    "mov x0, #5", "b aarch64_exception_vector_handler", ".space 120",
    "mov x0, #6", "b aarch64_exception_vector_handler", ".space 120",
    "mov x0, #7", "b aarch64_exception_vector_handler", ".space 120",
    "mov x0, #8", "b aarch64_exception_vector_handler", ".space 120",
    "mov x0, #9", "b aarch64_exception_vector_handler", ".space 120",
    "mov x0, #10", "b aarch64_exception_vector_handler", ".space 120",
    "mov x0, #11", "b aarch64_exception_vector_handler", ".space 120",
    "mov x0, #12", "b aarch64_exception_vector_handler", ".space 120",
    "mov x0, #13", "b aarch64_exception_vector_handler", ".space 120",
    "mov x0, #14", "b aarch64_exception_vector_handler", ".space 120",
    "mov x0, #15", "b aarch64_exception_vector_handler", ".space 120",
    ".section .text, \"ax\"",
    ".global aarch64_exception_vector_handler",
    "aarch64_exception_vector_handler:",
    "bl rust_exception_handler",
    "1:",
    "wfe",
    "b 1b",
);

const FALLBACK_PL011_BASE: usize = 0x0900_0000;
const UART_DR_OFFSET: usize = 0x00;
const UART_FR_OFFSET: usize = 0x18;
const UART_IBRD_OFFSET: usize = 0x24;
const UART_FBRD_OFFSET: usize = 0x28;
const UART_LCR_H_OFFSET: usize = 0x2C;
const UART_CR_OFFSET: usize = 0x30;
const UART_ICR_OFFSET: usize = 0x44;
const UART_REQUIRED_SIZE: u64 = 0x48;
const UART_FR_TXFF: u32 = 1 << 5;
static mut EARLY_SERIAL_BASE: usize = FALLBACK_PL011_BASE;

extern "C" {
    static __exception_vectors: u8;
}

struct Serial {
    base: usize,
}

impl Serial {
    const fn new(base: usize) -> Self {
        Self { base }
    }

    fn register(&self, offset: usize) -> *mut u32 {
        (self.base + offset) as *mut u32
    }

    fn init(&mut self) {
        // Use a compatible/reg pair from the validated Device Tree when present.
        // The QEMU virt address remains an early-console fallback only.
        unsafe {
            write_volatile(self.register(UART_CR_OFFSET), 0);
            write_volatile(self.register(UART_ICR_OFFSET), 0x7ff);
            write_volatile(self.register(UART_IBRD_OFFSET), 13);
            write_volatile(self.register(UART_FBRD_OFFSET), 1);
            write_volatile(self.register(UART_LCR_H_OFFSET), 0x70);
            write_volatile(self.register(UART_CR_OFFSET), 0x301);
        }
    }

    fn write_byte(&mut self, byte: u8) {
        for _ in 0..1_000_000 {
            if unsafe { read_volatile(self.register(UART_FR_OFFSET)) } & UART_FR_TXFF == 0 {
                unsafe { write_volatile(self.register(UART_DR_OFFSET), byte as u32) };
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

    fn write_hex(&mut self, value: u64) {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        self.write_str("0x");
        for shift in (0..16).rev() {
            self.write_byte(HEX[((value >> (shift * 4)) & 0xf) as usize]);
        }
    }
}

unsafe fn boot_info_device_tree(info: *const BootInfo) -> u64 {
    if info.is_null() {
        return 0;
    }
    let first_word = unsafe { read_volatile(info.cast::<u64>()) };
    if first_word == BOOT_INFO_MAGIC {
        let boot_info = unsafe { &*info };
        if boot_info.version == BOOT_INFO_VERSION
            && boot_info.struct_size == core::mem::size_of::<BootInfo>() as u32
        {
            return boot_info.device_tree;
        }
        return 0;
    }

    let first_u32 = unsafe { u32::from_be(read_volatile(info.cast::<u32>())) };
    if first_u32 == FDT_MAGIC {
        info as usize as u64
    } else {
        0
    }
}

unsafe fn discover_pl011_base(address: u64) -> Option<usize> {
    if address == 0 || address > usize::MAX as u64 {
        return None;
    }
    let base = address as usize as *const u8;
    // The header pointer comes from the boot contract; cap it before slicing.
    let total = unsafe { u32::from_be(read_volatile(base.add(4).cast::<u32>())) as usize };
    if total < FDT_HEADER_SIZE || total > MAX_FDT_SIZE {
        return None;
    }
    let image = unsafe { core::slice::from_raw_parts(base, total) };
    validate_fdt_tree(image).ok()?;
    let reg = match find_compatible_reg(image, b"arm,pl011") {
        Ok(Some(reg)) => reg,
        Ok(None) | Err(_) => return None,
    };
    if reg.address > usize::MAX as u64
        || reg.address % core::mem::align_of::<u32>() as u64 != 0
        || reg.size < UART_REQUIRED_SIZE
        || reg.address.checked_add(UART_REQUIRED_SIZE).is_none()
    {
        return None;
    }
    Some(reg.address as usize)
}

unsafe fn boot_info_acpi_rsdp(info: *const BootInfo) -> u64 {
    if info.is_null() || unsafe { read_volatile(info.cast::<u64>()) } != BOOT_INFO_MAGIC {
        return 0;
    }
    let boot_info = unsafe { &*info };
    if boot_info.version == BOOT_INFO_VERSION
        && boot_info.struct_size == core::mem::size_of::<BootInfo>() as u32
    {
        boot_info.acpi_rsdp
    } else {
        0
    }
}

unsafe fn read_rsdp_info(address: u64) -> Option<RsdpInfo> {
    if address == 0 || address > usize::MAX as u64 {
        return None;
    }
    let base = address as usize as *const u8;
    // SAFETY: the address is supplied by the UEFI ACPI configuration table.
    let legacy = unsafe { core::slice::from_raw_parts(base, 20) };
    if legacy[15] < 2 {
        return parse_rsdp(legacy).ok();
    }
    // The extended structure includes a length field at offset 20.
    // SAFETY: revision >= 2 specifies the 36-byte extended RSDP header.
    let fixed = unsafe { core::slice::from_raw_parts(base, 36) };
    let length = u32::from_le_bytes([fixed[20], fixed[21], fixed[22], fixed[23]]) as usize;
    if !(36..=MAX_RSDP_SIZE).contains(&length) {
        return None;
    }
    // SAFETY: the length is bounded by the shared parser's maximum RSDP size.
    let complete = unsafe { core::slice::from_raw_parts(base, length) };
    parse_rsdp(complete).ok()
}

unsafe fn acpi_sdt_at(
    address: u64,
    expected_signature: Option<&[u8; 4]>,
) -> Option<&'static [u8]> {
    if address == 0 || address > usize::MAX as u64 || address % 4 != 0 {
        return None;
    }
    let base = address as usize as *const u8;
    // SAFETY: table addresses originate from the validated firmware RSDP/table root.
    let header = unsafe { core::slice::from_raw_parts(base, SDT_HEADER_SIZE) };
    let length = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
    if !(SDT_HEADER_SIZE..=MAX_SDT_SIZE).contains(&length) {
        return None;
    }
    // SAFETY: table length is capped before creating the complete table view.
    let table = unsafe { core::slice::from_raw_parts(base, length) };
    let parsed = parse_sdt_header(table).ok()?;
    if parsed.length != length {
        return None;
    }
    if let Some(expected) = expected_signature {
        if &parsed.signature != expected {
            return None;
        }
    }
    Some(table)
}

unsafe fn discover_pl011_base_from_acpi(rsdp_address: u64) -> Option<usize> {
    let rsdp = unsafe { read_rsdp_info(rsdp_address) }?;
    let (root_address, root_signature, entry_size) = if rsdp.xsdt_address != 0 {
        (rsdp.xsdt_address, b"XSDT" as &[u8; 4], 8usize)
    } else if rsdp.rsdt_address != 0 {
        (rsdp.rsdt_address as u64, b"RSDT" as &[u8; 4], 4usize)
    } else {
        return None;
    };
    let root = unsafe { acpi_sdt_at(root_address, Some(root_signature)) }?;
    let entries = root.get(SDT_HEADER_SIZE..)?;
    if entries.len() % entry_size != 0 || entries.len() / entry_size > 65_536 {
        return None;
    }

    for entry in entries.chunks_exact(entry_size) {
        let address = if entry_size == 8 {
            u64::from_le_bytes([
                entry[0], entry[1], entry[2], entry[3],
                entry[4], entry[5], entry[6], entry[7],
            ])
        } else {
            u32::from_le_bytes([entry[0], entry[1], entry[2], entry[3]]) as u64
        };
        if address == 0 {
            continue;
        }
        // Do not dereference arbitrary signatures: first validate the child SDT
        // bounds and checksum before passing it to the SPCR-specific parser.
        let table = match unsafe { acpi_sdt_at(address, None) } {
            Some(table) => table,
            None => continue,
        };
        if table.get(0..4) != Some(&b"SPCR"[..]) {
            continue;
        }
        let base = match parse_spcr_uart_base(table) {
            Ok(base) => base,
            Err(_) => continue,
        };
        if base > usize::MAX as u64
            || base % core::mem::align_of::<u32>() as u64 != 0
            || base.checked_add(UART_REQUIRED_SIZE).is_none()
        {
            continue;
        }
        return Some(base as usize);
    }
    None
}

fn install_exception_vectors() -> bool {
    // The vector table contains sixteen 128-byte slots and must be 2 KiB aligned.
    let base = core::ptr::addr_of!(__exception_vectors) as u64;
    if base & 0x7ff != 0 {
        return false;
    }

    let current_el: u64;
    unsafe {
        asm!(
            "mrs {level}, CurrentEL",
            level = out(reg) current_el,
            options(nomem, nostack, preserves_flags)
        );
    }
    match current_el & 0x0c {
        0x04 => unsafe {
            asm!("msr VBAR_EL1, {base}", base = in(reg) base, options(nostack, preserves_flags));
        },
        0x08 => unsafe {
            asm!("msr VBAR_EL2, {base}", base = in(reg) base, options(nostack, preserves_flags));
        },
        0x0c => unsafe {
            asm!("msr VBAR_EL3, {base}", base = in(reg) base, options(nostack, preserves_flags));
        },
        _ => return false,
    }
    unsafe { asm!("dsb sy", "isb", options(nostack, preserves_flags)); }
    true
}

fn exception_registers() -> Option<(u64, u64, u64)> {
    let current_el: u64;
    unsafe {
        asm!(
            "mrs {level}, CurrentEL",
            level = out(reg) current_el,
            options(nomem, nostack, preserves_flags)
        );
    }

    let esr: u64;
    let elr: u64;
    let far: u64;
    match current_el & 0x0c {
        0x04 => unsafe {
            asm!(
                "mrs {esr}, ESR_EL1",
                "mrs {elr}, ELR_EL1",
                "mrs {far}, FAR_EL1",
                esr = out(reg) esr,
                elr = out(reg) elr,
                far = out(reg) far,
                options(nomem, nostack, preserves_flags)
            );
        },
        0x08 => unsafe {
            asm!(
                "mrs {esr}, ESR_EL2",
                "mrs {elr}, ELR_EL2",
                "mrs {far}, FAR_EL2",
                esr = out(reg) esr,
                elr = out(reg) elr,
                far = out(reg) far,
                options(nomem, nostack, preserves_flags)
            );
        },
        0x0c => unsafe {
            asm!(
                "mrs {esr}, ESR_EL3",
                "mrs {elr}, ELR_EL3",
                "mrs {far}, FAR_EL3",
                esr = out(reg) esr,
                elr = out(reg) elr,
                far = out(reg) far,
                options(nomem, nostack, preserves_flags)
            );
        },
        _ => return None,
    }
    Some((esr, elr, far))
}

#[no_mangle]
pub extern "C" fn rust_exception_handler(vector: u64) -> ! {
    let base = unsafe { read_volatile(core::ptr::addr_of!(EARLY_SERIAL_BASE)) };
    let mut serial = Serial::new(base);
    serial.write_str("ArchiaOS AArch64: exception vector entered\n");
    serial.write_str("ArchiaOS AArch64: vector=");
    serial.write_hex(vector);
    serial.write_str("\n");

    if let Some((esr, elr, far)) = exception_registers() {
        serial.write_str("ArchiaOS AArch64: ESR=");
        serial.write_hex(esr);
        serial.write_str(" ELR=");
        serial.write_hex(elr);
        serial.write_str(" FAR=");
        serial.write_hex(far);
        serial.write_str("\n");
    } else {
        serial.write_str("ArchiaOS AArch64: unknown exception level\n");
    }

    // The bring-up test executes BRK while running with SPx, which is vector 4.
    if vector == 4 {
        serial.write_str("ArchiaOS AArch64: expected synchronous exception trapped\n");
    }
    halt_forever()
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
        serial.write_str("ArchiaOS AArch64: no UEFI BootInfo (direct QEMU boot)\n");
        return;
    }

    // QEMU's direct -kernel path supplies a Device Tree pointer in x0, whereas
    // our UEFI loader supplies the versioned ARCHIABI structure.
    let magic = unsafe { read_volatile(info.cast::<u64>()) };
    if magic != BOOT_INFO_MAGIC {
        let fdt_magic = unsafe { u32::from_be(read_volatile(info.cast::<u32>())) };
        if fdt_magic == FDT_MAGIC {
            serial.write_str("ArchiaOS AArch64: direct QEMU Device Tree detected\n");
            if unsafe { validate_fdt_at_address(info as usize as u64) } {
                serial.write_str("ArchiaOS AArch64: direct QEMU Device Tree header validated\n");
            } else {
                serial.write_str("ArchiaOS AArch64: direct QEMU Device Tree header rejected\n");
                halt_forever();
            }
        } else {
            serial.write_str("ArchiaOS AArch64: no UEFI BootInfo (direct QEMU boot)\n");
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
        || (b.device_tree == 0 && b.acpi_rsdp == 0)
    {
        serial.write_str("ArchiaOS AArch64: BootInfo validation failed\n");
        halt_forever();
    }

    let count = (b.memory_map_size / b.memory_map_descriptor_size) as usize;
    let stride = b.memory_map_descriptor_size as usize;
    if count == 0 || count > 65_536 {
        serial.write_str("ArchiaOS AArch64: memory-map entry count rejected\n");
        halt_forever();
    }

    // Some UEFI platforms publish a Device Tree, while ACPI-based ARM64
    // platforms publish an RSDP instead. Accept either validated contract.
    if b.device_tree != 0 {
        if !unsafe { validate_fdt_at_address(b.device_tree) } {
            serial.write_str("ArchiaOS AArch64: Device Tree header rejected\n");
            halt_forever();
        }
        serial.write_str("ArchiaOS AArch64: Device Tree header validated\n");
    }
    if b.acpi_rsdp != 0 {
        if unsafe { read_rsdp_info(b.acpi_rsdp) }.is_none() {
            serial.write_str("ArchiaOS AArch64: ACPI RSDP rejected\n");
            halt_forever();
        }
        serial.write_str("ArchiaOS AArch64: ACPI RSDP validated\n");
    }

    // The shared parser validates every descriptor's stride, physical alignment,
    // non-zero page range and checked physical end before the kernel trusts the map.
    let map_bytes = unsafe {
        core::slice::from_raw_parts(b.memory_map, b.memory_map_size as usize)
    };
    let memory_map = match UefiMemoryMap::parse(map_bytes, stride) {
        Ok(map) if map.len() == count => map,
        _ => {
            serial.write_str("ArchiaOS AArch64: UEFI memory map rejected\n");
            halt_forever();
        }
    };
    if memory_map.is_empty() {
        serial.write_str("ArchiaOS AArch64: UEFI memory map is empty\n");
        halt_forever();
    }
    let conventional_pages = match memory_map.conventional_page_count() {
        Ok(pages) if pages > 0 => pages,
        _ => {
            serial.write_str("ArchiaOS AArch64: conventional memory validation failed\n");
            halt_forever();
        }
    };

    serial.write_str("ArchiaOS AArch64: BootInfo validated\n");
    serial.write_str("ArchiaOS AArch64: UEFI memory-map descriptors validated\n");
    if conventional_pages > 0 {
        serial.write_str("ArchiaOS AArch64: conventional memory pages validated\n");
    }

    match memory::initialize(&memory_map) {
        Ok(stats) if stats.free_pages > 0 && stats.conventional_pages_below_4g > 0 => {
            serial.write_str("ArchiaOS AArch64: physical page allocator initialized\n");
        }
        _ => {
            serial.write_str("ArchiaOS AArch64: physical page allocator initialization failed\n");
            halt_forever();
        }
    }
    match memory::self_test() {
        Ok(()) => serial.write_str("ArchiaOS AArch64: PMM allocation/free self-test passed\n"),
        Err(_) => {
            serial.write_str("ArchiaOS AArch64: PMM allocation/free self-test failed\n");
            halt_forever();
        }
    }
}

#[no_mangle]
pub extern "C" fn rust_kernel_main(info: *const BootInfo) -> ! {
    let device_tree = unsafe { boot_info_device_tree(info) };
    let acpi_rsdp = unsafe { boot_info_acpi_rsdp(info) };
    let dt_uart = unsafe { discover_pl011_base(device_tree) };
    let acpi_uart = if dt_uart.is_none() {
        unsafe { discover_pl011_base_from_acpi(acpi_rsdp) }
    } else {
        None
    };
    let discovered_base = dt_uart.or(acpi_uart);
    let serial_base = discovered_base.unwrap_or(FALLBACK_PL011_BASE);
    unsafe { write_volatile(core::ptr::addr_of_mut!(EARLY_SERIAL_BASE), serial_base); }
    let mut serial = Serial::new(serial_base);
    serial.init();
    serial.write_str("ArchiaOS AArch64: kernel entry reached\n");
    serial.write_str("ArchiaOS AArch64: PL011 diagnostics initialized\n");
    if dt_uart.is_some() {
        serial.write_str("ArchiaOS AArch64: PL011 base discovered from Device Tree\n");
    } else if acpi_uart.is_some() {
        serial.write_str("ArchiaOS AArch64: PL011 base discovered from ACPI SPCR\n");
    } else {
        serial.write_str("ArchiaOS AArch64: using QEMU virt PL011 fallback\n");
    }
    if install_exception_vectors() {
        serial.write_str("ArchiaOS AArch64: exception vectors installed\n");
    } else {
        serial.write_str("ArchiaOS AArch64: exception vector installation failed\n");
        halt_forever();
    }
    validate_boot_info(info, &mut serial);
    serial.write_str("ArchiaOS AArch64: executing synchronous exception self-test\n");
    unsafe { asm!("brk #0", options(nostack)); }
    halt_forever();
}

fn halt_forever() -> ! {
    loop {
        unsafe { asm!("msr daifset, #0xf", "wfe", options(nomem, nostack, preserves_flags)) };
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    let base = unsafe { read_volatile(core::ptr::addr_of!(EARLY_SERIAL_BASE)) };
    let mut serial = Serial::new(base);
    serial.init();
    serial.write_str("ArchiaOS AArch64: PANIC\n");
    halt_forever();
}
