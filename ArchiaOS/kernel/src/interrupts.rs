//! Minimal x86-64 exception IDT for early-kernel diagnostics (P2).
//!
//! All architectural exception vectors receive interrupt gates. Error-code
//! exceptions are normalized by their assembly stubs so the common dispatcher
//! always receives (vector, error_code). Handlers are fatal until recovery and
//! per-task exception delivery are designed.

use core::arch::asm;

const EXCEPTION_COUNT: usize = 32;
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

static mut IDT: [IdtEntry; EXCEPTION_COUNT] = [IdtEntry::MISSING; EXCEPTION_COUNT];

extern "C" {
    static isr_stub_table: [u64; EXCEPTION_COUNT];
}

/// Install exception vectors 0-31, preserving the active code-segment selector.
/// Interrupts remain disabled; this does not enable timer IRQs or preemption.
pub unsafe fn install() {
    let selector: u16;
    // SAFETY: reading CS is an architectural, side-effect-free operation.
    unsafe { asm!("mov {0:x}, cs", out(reg) selector, options(nomem, nostack, preserves_flags)) };

    for vector in 0..EXCEPTION_COUNT {
        // SAFETY: this runs during single-threaded early boot before IDT use.
        let handler = unsafe { core::ptr::addr_of!(isr_stub_table).cast::<u64>().add(vector).read() };
        let entry = IdtEntry::interrupt_gate(handler, selector);
        // SAFETY: the index is bounded by EXCEPTION_COUNT and no other CPU is using the IDT yet.
        unsafe { core::ptr::addr_of_mut!(IDT).cast::<IdtEntry>().add(vector).write(entry) };
    }

    let descriptor = Idtr {
        limit: (core::mem::size_of::<IdtEntry>() * EXCEPTION_COUNT - 1) as u16,
        base: core::ptr::addr_of!(IDT) as u64,
    };
    // SAFETY: IDT and descriptor remain valid for the kernel lifetime; the descriptor
    // covers exactly the 32 initialized exception gates.
    unsafe { asm!("lidt [{0}]", in(reg) &descriptor, options(readonly, nostack, preserves_flags)) };
}

global_asm!(
    ".section .text.archiaos_isr, \\"ax\\"",
    ".macro ISR_NOERR num",
    ".global isr_stub_\\\\num",
    "isr_stub_\\\\num:",
    "pushq $0",
    "pushq $\\\\num",
    "jmp isr_common",
    ".endm",
    ".macro ISR_ERR num",
    ".global isr_stub_\\\\num",
    "isr_stub_\\\\num:",
    "pushq $\\\\num",
    "jmp isr_common",
    ".endm",
    "ISR_NOERR 0", "ISR_NOERR 1", "ISR_NOERR 2", "ISR_NOERR 3",
    "ISR_NOERR 4", "ISR_NOERR 5", "ISR_NOERR 6", "ISR_NOERR 7",
    "ISR_ERR 8",
    "ISR_NOERR 9",
    "ISR_ERR 10", "ISR_ERR 11", "ISR_ERR 12", "ISR_ERR 13", "ISR_ERR 14",
    "ISR_NOERR 15", "ISR_NOERR 16", "ISR_ERR 17", "ISR_NOERR 18", "ISR_NOERR 19",
    "ISR_NOERR 20", "ISR_ERR 21", "ISR_NOERR 22", "ISR_NOERR 23", "ISR_NOERR 24",
    "ISR_NOERR 25", "ISR_NOERR 26", "ISR_NOERR 27", "ISR_NOERR 28", "ISR_ERR 29", "ISR_ERR 30", "ISR_NOERR 31",
    ".global isr_common",
    "isr_common:",
    "movq (%rsp), %rdi",
    "movq 8(%rsp), %rsi",
    "andq $-16, %rsp",
    "call archiaos_exception_dispatch",
    ".Larchiaos_exception_halt:",
    "cli",
    "hlt",
    "jmp .Larchiaos_exception_halt",
    ".section .rodata.archiaos_isr, \\"a\\"",
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
);
