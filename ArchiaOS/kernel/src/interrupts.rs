//! Minimal x86-64 exception IDT for early-kernel diagnostics (P2).
//!
//! All architectural exception vectors receive interrupt gates. Error-code
//! exceptions are normalized by their assembly stubs so the common dispatcher
//! always receives (vector, error_code). Handlers are fatal until recovery and
//! per-task exception delivery are designed.

use core::arch::{asm, global_asm};

const EXCEPTION_COUNT: usize = 32;
const IDT_ENTRY_COUNT: usize = 256;
const INTERRUPT_GATE: u8 = 0x8e;

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct IdtEntry {
    offset_low: u16,
    selector: u16,
    ist: u8,
    attributes: u8,
    offset_middle: u16,
    offset_high: u32,
    reserved: u32,
}

impl IdtEntry {
    const MISSING: Self = Self {
        offset_low: 0,
        selector: 0,
        ist: 0,
        attributes: 0,
        offset_middle: 0,
        offset_high: 0,
        reserved: 0,
    };

    fn interrupt_gate(handler: u64, selector: u16) -> Self {
        Self {
            offset_low: handler as u16,
            selector,
            ist: 0,
            attributes: INTERRUPT_GATE,
            offset_middle: (handler >> 16) as u16,
            offset_high: (handler >> 32) as u32,
            reserved: 0,
        }
    }
}

#[repr(C, packed)]
struct Idtr {
    limit: u16,
    base: u64,
}

static mut IDT: [IdtEntry; IDT_ENTRY_COUNT] = [IdtEntry::MISSING; IDT_ENTRY_COUNT];

extern "C" {
    static isr_stub_table: [u64; EXCEPTION_COUNT];
    fn isr_default_stub();
}

/// Install all 256 IDT entries, preserving the active code-segment selector.
/// CPU exceptions get dedicated stubs; every other vector gets a fatal default
/// stub so an unexpected vector cannot immediately fault through an absent gate.
/// Interrupts remain disabled; this does not enable timer IRQs or preemption.
pub unsafe fn install() {
    let selector: u16;
    // SAFETY: reading CS is an architectural, side-effect-free operation.
    unsafe { asm!("mov {0:x}, cs", out(reg) selector, options(nomem, nostack, preserves_flags)) };

    let default_handler = isr_default_stub as usize as u64;
    for vector in 0..IDT_ENTRY_COUNT {
        let entry = IdtEntry::interrupt_gate(default_handler, selector);
        // SAFETY: bounded index; IDT is initialized before it is loaded.
        unsafe { core::ptr::addr_of_mut!(IDT).cast::<IdtEntry>().add(vector).write(entry) };
    }

    for vector in 0..EXCEPTION_COUNT {
        // SAFETY: this runs during single-threaded early boot before IDT use.
        let handler = unsafe { core::ptr::addr_of!(isr_stub_table).cast::<u64>().add(vector).read() };
        let entry = IdtEntry::interrupt_gate(handler, selector);
        // SAFETY: the index is bounded by EXCEPTION_COUNT and no other CPU is using the IDT yet.
        unsafe { core::ptr::addr_of_mut!(IDT).cast::<IdtEntry>().add(vector).write(entry) };
    }

    let descriptor = Idtr {
        limit: (core::mem::size_of::<IdtEntry>() * IDT_ENTRY_COUNT - 1) as u16,
        base: core::ptr::addr_of!(IDT) as u64,
    };
    // SAFETY: IDT and descriptor remain valid for the kernel lifetime; the descriptor
    // covers all 256 initialized gates (exceptions plus fatal defaults).
    unsafe { asm!("lidt [{0}]", in(reg) &descriptor, options(readonly, nostack, preserves_flags)) };
}

global_asm!(
    ".section .text",
    ".global isr_stub_0",
    "isr_stub_0:",
    "pushq $0",
    "pushq $0",
    "jmp isr_common",
    ".global isr_stub_1",
    "isr_stub_1:",
    "pushq $0",
    "pushq $1",
    "jmp isr_common",
    ".global isr_stub_2",
    "isr_stub_2:",
    "pushq $0",
    "pushq $2",
    "jmp isr_common",
    ".global isr_stub_3",
    "isr_stub_3:",
    "pushq $0",
    "pushq $3",
    "jmp isr_common",
    ".global isr_stub_4",
    "isr_stub_4:",
    "pushq $0",
    "pushq $4",
    "jmp isr_common",
    ".global isr_stub_5",
    "isr_stub_5:",
    "pushq $0",
    "pushq $5",
    "jmp isr_common",
    ".global isr_stub_6",
    "isr_stub_6:",
    "pushq $0",
    "pushq $6",
    "jmp isr_common",
    ".global isr_stub_7",
    "isr_stub_7:",
    "pushq $0",
    "pushq $7",
    "jmp isr_common",
    ".global isr_stub_8",
    "isr_stub_8:",
    "pushq $8",
    "jmp isr_common",
    ".global isr_stub_9",
    "isr_stub_9:",
    "pushq $0",
    "pushq $9",
    "jmp isr_common",
    ".global isr_stub_10",
    "isr_stub_10:",
    "pushq $10",
    "jmp isr_common",
    ".global isr_stub_11",
    "isr_stub_11:",
    "pushq $11",
    "jmp isr_common",
    ".global isr_stub_12",
    "isr_stub_12:",
    "pushq $12",
    "jmp isr_common",
    ".global isr_stub_13",
    "isr_stub_13:",
    "pushq $13",
    "jmp isr_common",
    ".global isr_stub_14",
    "isr_stub_14:",
    "pushq $14",
    "jmp isr_common",
    ".global isr_stub_15",
    "isr_stub_15:",
    "pushq $0",
    "pushq $15",
    "jmp isr_common",
    ".global isr_stub_16",
    "isr_stub_16:",
    "pushq $0",
    "pushq $16",
    "jmp isr_common",
    ".global isr_stub_17",
    "isr_stub_17:",
    "pushq $17",
    "jmp isr_common",
    ".global isr_stub_18",
    "isr_stub_18:",
    "pushq $0",
    "pushq $18",
    "jmp isr_common",
    ".global isr_stub_19",
    "isr_stub_19:",
    "pushq $0",
    "pushq $19",
    "jmp isr_common",
    ".global isr_stub_20",
    "isr_stub_20:",
    "pushq $0",
    "pushq $20",
    "jmp isr_common",
    ".global isr_stub_21",
    "isr_stub_21:",
    "pushq $21",
    "jmp isr_common",
    ".global isr_stub_22",
    "isr_stub_22:",
    "pushq $0",
    "pushq $22",
    "jmp isr_common",
    ".global isr_stub_23",
    "isr_stub_23:",
    "pushq $0",
    "pushq $23",
    "jmp isr_common",
    ".global isr_stub_24",
    "isr_stub_24:",
    "pushq $0",
    "pushq $24",
    "jmp isr_common",
    ".global isr_stub_25",
    "isr_stub_25:",
    "pushq $0",
    "pushq $25",
    "jmp isr_common",
    ".global isr_stub_26",
    "isr_stub_26:",
    "pushq $0",
    "pushq $26",
    "jmp isr_common",
    ".global isr_stub_27",
    "isr_stub_27:",
    "pushq $0",
    "pushq $27",
    "jmp isr_common",
    ".global isr_stub_28",
    "isr_stub_28:",
    "pushq $0",
    "pushq $28",
    "jmp isr_common",
    ".global isr_stub_29",
    "isr_stub_29:",
    "pushq $29",
    "jmp isr_common",
    ".global isr_stub_30",
    "isr_stub_30:",
    "pushq $30",
    "jmp isr_common",
    ".global isr_stub_31",
    "isr_stub_31:",
    "pushq $0",
    "pushq $31",
    "jmp isr_common",
    ".global isr_default_stub",
    "isr_default_stub:",
    "pushq $0",
    "pushq $255",
    "jmp isr_common",
    ".global isr_common",
    "isr_common:",
    "movq (%rsp), %rdi",
    "movq 8(%rsp), %rsi",
    "movq 16(%rsp), %rdx",
    "andq $-16, %rsp",
    "call archiaos_exception_dispatch",
    ".Larchiaos_exception_halt:",
    "cli",
    "hlt",
    "jmp .Larchiaos_exception_halt",
    ".section .rodata",
    ".balign 8",
    ".global isr_stub_table",
    "isr_stub_table:",
    ".quad isr_stub_0, isr_stub_1, isr_stub_2, isr_stub_3",
    ".quad isr_stub_4, isr_stub_5, isr_stub_6, isr_stub_7",
    ".quad isr_stub_8, isr_stub_9, isr_stub_10, isr_stub_11",
    ".quad isr_stub_12, isr_stub_13, isr_stub_14, isr_stub_15",
    ".quad isr_stub_16, isr_stub_17, isr_stub_18, isr_stub_19",
    ".quad isr_stub_20, isr_stub_21, isr_stub_22, isr_stub_23",
    ".quad isr_stub_24, isr_stub_25, isr_stub_26, isr_stub_27",
    ".quad isr_stub_28, isr_stub_29, isr_stub_30, isr_stub_31",
    options(att_syntax)
);
