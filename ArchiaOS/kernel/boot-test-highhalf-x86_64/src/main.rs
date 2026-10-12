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
const PAGE_PRESENT: u64 = 1;
const PAGE_WRITABLE: u64 = 1 << 1;
const PAGE_LARGE: u64 = 1 << 7;
const PAGE_NO_EXECUTE: u64 = 1 << 63;

#[used]
#[link_section = ".data"]
#[no_mangle]
static mut PAGE_PERMISSION_TEST_DATA: u64 = 0x4152_4348_4941_4f53;

#[used]
#[link_section = ".data"]
#[no_mangle]
static mut ARCHIAOS_WRITE_FAULT_SEEN: u64 = 0;

#[used]
#[link_section = ".data"]
#[no_mangle]
static mut ARCHIAOS_NX_FAULT_SEEN: u64 = 0;

#[used]
#[link_section = ".data"]
#[no_mangle]
static mut ARCHIAOS_PAGE_FAULT_TEST_FAILURE: u64 = 0;

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct IdtEntry {
    offset_low: u16,
    selector: u16,
    ist: u8,
    attributes: u8,
    offset_mid: u16,
    offset_high: u32,
    reserved: u32,
}

impl IdtEntry {
    const EMPTY: Self = Self {
        offset_low: 0,
        selector: 0,
        ist: 0,
        attributes: 0,
        offset_mid: 0,
        offset_high: 0,
        reserved: 0,
    };

    const fn interrupt_gate(handler: u64, selector: u16) -> Self {
        Self {
            offset_low: handler as u16,
            selector,
            ist: 0,
            attributes: 0x8e,
            offset_mid: (handler >> 16) as u16,
            offset_high: (handler >> 32) as u32,
            reserved: 0,
        }
    }
}

#[repr(align(16))]
struct AlignedIdt([IdtEntry; 256]);

static mut ARCHIAOS_TEST_IDT: AlignedIdt = AlignedIdt([IdtEntry::EMPTY; 256]);

#[repr(C, packed)]
struct Idtr {
    limit: u16,
    base: u64,
}

extern "C" {
    fn archiaos_page_fault_entry();
    fn archiaos_test_write_readonly(address: *mut u8);
    fn archiaos_test_execute_nx(address: *const u8);
    fn archiaos_test_protected_text_marker();
}

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

global_asm!(
    ".section .text, \"ax\"",
    ".global archiaos_page_fault_entry",
    ".type archiaos_page_fault_entry, @function",
    "archiaos_page_fault_entry:",
    "cli",
    // On a same-CPL #PF, the CPU pushes error code, RIP, CS and RFLAGS.
    // Preserve every scratch register used by this assembly-only handler.
    "push rax",
    "push rcx",
    "push rdx",
    "push rsi",
    "push rdi",
    "push r8",
    "push r9",
    "push r10",
    "push r11",
    "mov rax, [rsp + 72]",
    "mov rdx, [rsp + 80]",
    "lea rcx, [rip + archiaos_test_write_fault_instruction]",
    "cmp rdx, rcx",
    "je .Larchiaos_write_page_fault",
    "lea rcx, [rip + PAGE_PERMISSION_TEST_DATA]",
    "cmp rdx, rcx",
    "je .Larchiaos_nx_page_fault",
    "jmp .Larchiaos_unexpected_page_fault",
    ".Larchiaos_write_page_fault:",
    "mov rcx, rax",
    "and rcx, 0x1f",
    "cmp rcx, 0x3",
    "jne .Larchiaos_write_fault_bad_code",
    "mov qword ptr [rip + ARCHIAOS_WRITE_FAULT_SEEN], 1",
    "jmp .Larchiaos_write_fault_resume",
    ".Larchiaos_write_fault_bad_code:",
    "mov qword ptr [rip + ARCHIAOS_PAGE_FAULT_TEST_FAILURE], 1",
    ".Larchiaos_write_fault_resume:",
    "lea rcx, [rip + archiaos_test_write_recovery]",
    "mov [rsp + 80], rcx",
    "jmp .Larchiaos_restore_and_iret",
    ".Larchiaos_nx_page_fault:",
    "mov rcx, rax",
    "and rcx, 0x1f",
    "cmp rcx, 0x11",
    "jne .Larchiaos_nx_fault_bad_code",
    "mov qword ptr [rip + ARCHIAOS_NX_FAULT_SEEN], 1",
    "jmp .Larchiaos_nx_fault_resume",
    ".Larchiaos_nx_fault_bad_code:",
    "mov qword ptr [rip + ARCHIAOS_PAGE_FAULT_TEST_FAILURE], 1",
    ".Larchiaos_nx_fault_resume:",
    "lea rcx, [rip + archiaos_test_exec_recovery]",
    "mov [rsp + 80], rcx",
    ".Larchiaos_restore_and_iret:",
    "pop r11",
    "pop r10",
    "pop r9",
    "pop r8",
    "pop rdi",
    "pop rsi",
    "pop rdx",
    "pop rcx",
    "pop rax",
    "add rsp, 8",
    "iretq",
    ".Larchiaos_unexpected_page_fault:",
    "cli",
    "1:",
    "hlt",
    "jmp 1b",
    ".global archiaos_test_write_readonly",
    ".type archiaos_test_write_readonly, @function",
    "archiaos_test_write_readonly:",
    ".global archiaos_test_write_fault_instruction",
    "archiaos_test_write_fault_instruction:",
    "mov byte ptr [rdi], 0x90",
    ".global archiaos_test_write_recovery",
    "archiaos_test_write_recovery:",
    "ret",
    ".global archiaos_test_execute_nx",
    ".type archiaos_test_execute_nx, @function",
    "archiaos_test_execute_nx:",
    "jmp rdi",
    ".global archiaos_test_exec_recovery",
    "archiaos_test_exec_recovery:",
    "ret",
    ".global archiaos_test_protected_text_marker",
    "archiaos_test_protected_text_marker:",
    "nop",
    "ret",
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

fn install_page_fault_test_handler() -> bool {
    let code_selector: u16;
    unsafe {
        asm!(
            "mov {selector:x}, cs",
            selector = out(reg) code_selector,
            options(nomem, nostack, preserves_flags)
        );
    }
    let handler = archiaos_page_fault_entry as usize as u64;
    // SAFETY: writes one valid interrupt gate to the private, writable IDT.
    unsafe {
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!(ARCHIAOS_TEST_IDT.0)
                .cast::<IdtEntry>()
                .add(14),
            IdtEntry::interrupt_gate(handler, code_selector),
        );
    }
    let idtr = Idtr {
        limit: (core::mem::size_of::<[IdtEntry; 256]>() - 1) as u16,
        // SAFETY: taking the raw address of mutable static storage is required
        // for LIDT; no Rust reference to the table is created or retained.
        base: unsafe { core::ptr::addr_of!(ARCHIAOS_TEST_IDT.0) as u64 },
    };
    // SAFETY: IDT storage remains mapped by the active high-half data mapping.
    unsafe {
        asm!(
            "lidt [{pointer}]",
            pointer = in(reg) core::ptr::addr_of!(idtr),
            options(nostack, preserves_flags)
        );
    }
    true
}

/// Walk a four-level paging structure. The bootloader currently builds only
/// 4 KiB leaf mappings, so large-page entries are rejected instead of misread.
fn lookup_leaf_pte(virtual_address: u64) -> Option<u64> {
    let root: u64;
    unsafe {
        asm!(
            "mov {}, cr3",
            out(reg) root,
            options(nomem, nostack, preserves_flags)
        );
    }
    let mut table = (root & !0xfff) as *const u64;
    for shift in [39u32, 30, 21] {
        let index = ((virtual_address >> shift) & 0x1ff) as usize;
        let entry = unsafe { core::ptr::read_volatile(table.add(index)) };
        if entry & PAGE_PRESENT == 0 || entry & PAGE_LARGE != 0 {
            return None;
        }
        table = (entry & 0x000f_ffff_ffff_f000) as *const u64;
    }
    let index = ((virtual_address >> 12) & 0x1ff) as usize;
    let leaf = unsafe { core::ptr::read_volatile(table.add(index)) };
    if leaf & PAGE_PRESENT == 0 {
        None
    } else {
        Some(leaf)
    }
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

    // Read back the protection controls used by the loader's CR3 transition.
    let cr0: u64;
    unsafe {
        asm!(
            "mov {}, cr0",
            out(reg) cr0,
            options(nomem, nostack, preserves_flags)
        );
    }
    if cr0 & (1 << 16) == 0 {
        let _ = writeln!(serial, "ArchiaOS high-half test kernel: CR0.WP is not enabled");
        halt();
    }

    let efer_low: u32;
    let efer_high: u32;
    unsafe {
        asm!(
            "rdmsr",
            in("ecx") 0xC000_0080u32,
            out("eax") efer_low,
            out("edx") efer_high,
            options(nomem, nostack, preserves_flags)
        );
    }
    let efer = ((efer_high as u64) << 32) | efer_low as u64;
    if efer & (1 << 11) == 0 {
        let _ = writeln!(serial, "ArchiaOS high-half test kernel: EFER.NXE is not enabled");
        halt();
    }
    let _ = writeln!(serial, "ArchiaOS high-half test kernel: CR0.WP and EFER.NXE confirmed");

    // The custom page tables must be active, not merely prepared. Verify the
    // executable text leaf, a writable/non-executable data leaf, and the NX
    // physical identity alias of the currently executing text page.
    let text_pte = match lookup_leaf_pte(current_rip) {
        Some(entry) => entry,
        None => {
            let _ = writeln!(serial, "ArchiaOS high-half test kernel: text page is unmapped");
            halt();
        }
    };
    if text_pte & PAGE_WRITABLE != 0 || text_pte & PAGE_NO_EXECUTE != 0 {
        let _ = writeln!(serial, "ArchiaOS high-half test kernel: text is not read-only executable");
        halt();
    }

    let data_virtual = core::ptr::addr_of!(PAGE_PERMISSION_TEST_DATA) as u64;
    let data_pte = match lookup_leaf_pte(data_virtual) {
        Some(entry) => entry,
        None => {
            let _ = writeln!(serial, "ArchiaOS high-half test kernel: data page is unmapped");
            halt();
        }
    };
    if data_pte & PAGE_WRITABLE == 0 || data_pte & PAGE_NO_EXECUTE == 0 {
        let _ = writeln!(serial, "ArchiaOS high-half test kernel: data page is not writable/NX");
        halt();
    }

    // The test linker defines p_paddr = p_vaddr - HIGH_HALF_BASE, so the
    // identity alias uses the same high-half offset as the linked virtual image.
    let text_physical = current_rip
        .checked_sub(HIGH_HALF_BASE)
        .expect("linker physical address calculation underflow");
    let identity_pte = match lookup_leaf_pte(text_physical) {
        Some(entry) => entry,
        None => {
            let _ = writeln!(serial, "ArchiaOS high-half test kernel: text identity alias is unmapped");
            halt();
        }
    };
    if identity_pte & PAGE_NO_EXECUTE == 0 || identity_pte & PAGE_WRITABLE != 0 {
        let _ = writeln!(serial, "ArchiaOS high-half test kernel: executable identity alias is not read-only/NX");
        halt();
    }

    if !install_page_fault_test_handler() {
        let _ = writeln!(serial, "ArchiaOS high-half test kernel: failed to install page-fault test IDT");
        halt();
    }

    // Trigger real CPU faults, not just page-table inspection. The handler only
    // recovers the two expected faults; any other #PF halts the VM for CI.
    unsafe {
        archiaos_test_write_readonly(archiaos_test_protected_text_marker as usize as *mut u8);
    }
    let write_fault_seen = unsafe {
        core::ptr::read_volatile(core::ptr::addr_of!(ARCHIAOS_WRITE_FAULT_SEEN))
    };
    unsafe {
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!(PAGE_PERMISSION_TEST_DATA).cast::<u8>(),
            0xc3,
        );
        archiaos_test_execute_nx(core::ptr::addr_of!(PAGE_PERMISSION_TEST_DATA).cast::<u8>());
    }
    let nx_fault_seen = unsafe {
        core::ptr::read_volatile(core::ptr::addr_of!(ARCHIAOS_NX_FAULT_SEEN))
    };
    let page_fault_failure = unsafe {
        core::ptr::read_volatile(core::ptr::addr_of!(ARCHIAOS_PAGE_FAULT_TEST_FAILURE))
    };
    if write_fault_seen != 1 || nx_fault_seen != 1 || page_fault_failure != 0 {
        let _ = writeln!(
            serial,
            "ArchiaOS high-half test kernel: W^X/NX hardware fault test failed (write={}, nx={}, error={})",
            write_fault_seen,
            nx_fault_seen,
            page_fault_failure
        );
        halt();
    }
    let _ = writeln!(serial, "ArchiaOS high-half test kernel: read-only write fault trapped");
    let _ = writeln!(serial, "ArchiaOS high-half test kernel: NX instruction-fetch fault trapped");
    let _ = writeln!(serial, "ArchiaOS high-half test kernel: hardware W^X/NX enforcement passed");
    let _ = writeln!(serial, "ArchiaOS high-half test kernel: executing at {:#018x}", current_rip);
    let _ = writeln!(serial, "ArchiaOS high-half test kernel: BootInfo and identity map validated");
    let _ = writeln!(serial, "ArchiaOS high-half test kernel: high-half W^X/NX permissions passed");
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
