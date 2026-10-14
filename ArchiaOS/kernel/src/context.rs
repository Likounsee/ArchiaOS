//! Minimal x86-64 cooperative CPU context primitive (P2).
//!
//! Saves/restores the System V callee-saved register set, RSP, and legacy
//! x87/SSE state. The scheduler selects contexts; this primitive does not
//! enable interrupts or provide synchronization.

use core::arch::global_asm;

pub const TASK_STACK_SIZE: usize = 16 * 1024;
const STACK_GUARD_WORDS: usize = 8;
const STACK_GUARD_PATTERN: u64 = 0xA11C_57AC_C0DE_F00D;

/// Statically allocated task stack with canaries on both sides of the usable area.
/// The guards detect many over/underflows during diagnostics; they are not MMU guard pages.
#[repr(C, align(16))]
pub struct TaskStack {
    lower_guard: [u64; STACK_GUARD_WORDS],
    bytes: [u8; TASK_STACK_SIZE],
    upper_guard: [u64; STACK_GUARD_WORDS],
}

impl TaskStack {
    pub const fn new() -> Self {
        Self {
            lower_guard: [STACK_GUARD_PATTERN; STACK_GUARD_WORDS],
            bytes: [0; TASK_STACK_SIZE],
            upper_guard: [STACK_GUARD_PATTERN; STACK_GUARD_WORDS],
        }
    }

    /// Check the stack canaries without changing the task stack.
    pub fn guards_intact(&self) -> bool {
        self.lower_guard.iter().all(|word| *word == STACK_GUARD_PATTERN)
            && self.upper_guard.iter().all(|word| *word == STACK_GUARD_PATTERN)
    }

    /// Check whether a saved CPU context points into this stack's usable bytes.
    ///
    /// A non-zero RSP alone is not sufficient: a corrupted context could point
    /// into another task's stack or into the guard canaries and crash on restore.
    pub fn contains_context(&self, context: &CpuContext) -> bool {
        let start = self.bytes.as_ptr() as usize;
        let end = start + TASK_STACK_SIZE;
        let rsp = context.rsp as usize;
        rsp >= start && rsp < end
    }

    /// Prepare a never-run task to begin at `entry` after the first context switch.
    pub fn initialize(&mut self, context: &mut CpuContext, entry: extern "C" fn() -> !) {
        // The stack type guarantees a 16-byte-aligned top. The extra final
        // slot preserves the SysV function-entry RSP alignment after `ret`.
        let top = self.bytes.as_mut_ptr().wrapping_add(TASK_STACK_SIZE) as usize;
        let frame = (top - 8 * core::mem::size_of::<u64>()) as *mut u64;
        // Seed a first-run task with the current x87/SSE state. Every
        // subsequent switch saves and restores this area independently.
        unsafe { capture_fx_state(&mut context.simd) };
        unsafe {
            for index in 0..6 {
                frame.add(index).write(0);
            }
            frame.add(6).write(entry as usize as u64);
            frame.add(7).write(0);
            context.rsp = frame as u64;
        }
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(C, align(16))]
struct FxState([u8; 512]);

impl FxState {
    const fn empty() -> Self {
        Self([0; 512])
    }
}

impl Default for FxState {
    fn default() -> Self {
        Self::empty()
    }
}

#[derive(Clone, Copy, Debug, Default)]
#[repr(C, align(16))]
pub struct CpuContext {
    rsp: u64,
    // Keep the FXSAVE64 area at offset 16, aligned to its architectural
    // 16-byte requirement. AVX upper state is deliberately not saved yet.
    _reserved: u64,
    simd: FxState,
}

impl CpuContext {
    pub const fn empty() -> Self {
        Self { rsp: 0, _reserved: 0, simd: FxState::empty() }
    }

    pub fn is_initialized(&self) -> bool {
        // Saved contexts point at six pushed registers and naturally have
        // RSP % 16 == 8 at the assembly entry boundary. Only synthetic first-run
        // frames have the stricter 16-byte alignment checked by self_test().
        self.rsp != 0
    }
}

/// Enable legacy x87/SSE state management before any context switch.
///
/// AVX/AVX-512 execution is not enabled here: their extended state requires
/// XSAVE/XRSTOR context storage, which is intentionally a separate future step.
pub unsafe fn initialize_simd() {
    let mut cr0: u64;
    let mut cr4: u64;
    // SAFETY: this function is called only during privileged x86-64 kernel
    // initialization, before the scheduler is allowed to switch contexts.
    unsafe {
        core::arch::asm!("mov {}, cr0", out(reg) cr0, options(nomem, nostack, preserves_flags));
        cr0 &= !((1 << 2) | (1 << 3)); // EM=0, TS=0: permit eager FPU/SSE use.
        cr0 |= (1 << 1) | (1 << 5); // MP=1, NE=1.
        core::arch::asm!("mov cr0, {}", in(reg) cr0, options(nomem, nostack, preserves_flags));

        core::arch::asm!("mov {}, cr4", out(reg) cr4, options(nomem, nostack, preserves_flags));
        cr4 |= (1 << 9) | (1 << 10); // OSFXSR and OSXMMEXCPT.
        core::arch::asm!("mov cr4, {}", in(reg) cr4, options(nomem, nostack, preserves_flags));
        core::arch::asm!("fninit", options(nomem, nostack, preserves_flags));
        let mxcsr: u32 = 0x1F80;
        core::arch::asm!("ldmxcsr [{}]", in(reg) &mxcsr, options(readonly, nostack, preserves_flags));
    }
}

unsafe fn capture_fx_state(state: &mut FxState) {
    // SAFETY: the destination is 16-byte aligned and exactly 512 bytes.
    unsafe {
        core::arch::asm!(
            "fxsave64 [{}]",
            in(reg) state.0.as_mut_ptr(),
            options(nostack, preserves_flags)
        );
    }
}

/// Save the current callee-saved registers and switch to another saved stack.
///
/// # Safety
/// Both contexts must be valid and uniquely owned for the duration of this
/// operation. The target context must be initialized by this module or be a
/// previously saved context. The caller must ensure both stacks remain mapped
/// and alive, and must not switch while holding locks that require this task.
pub unsafe fn switch(old: &mut CpuContext, new: &CpuContext) {
    // SAFETY: the caller guarantees both context pointers and stacks are valid.
    unsafe { archiaos_context_switch(old as *mut CpuContext, new as *const CpuContext) };
}

extern "C" {
    fn archiaos_context_switch(old_context: *mut CpuContext, new_context: *const CpuContext);
}

global_asm!(
    ".section .text",
    ".global archiaos_context_switch",
    ".type archiaos_context_switch, @function",
    "archiaos_context_switch:",
    "pushq %rbp",
    "pushq %rbx",
    "pushq %r12",
    "pushq %r13",
    "pushq %r14",
    "pushq %r15",
    // FXSAVE preserves x87/MMX/MXCSR and XMM registers for each task.
    // The SIMD area is fixed at offset 16 by CpuContext's C layout.
    "fxsave64 16(%rdi)",
    "fxrstor64 16(%rsi)",
    "movq %rsp, (%rdi)",
    "movq (%rsi), %rsp",
    "popq %r15",
    "popq %r14",
    "popq %r13",
    "popq %r12",
    "popq %rbx",
    "popq %rbp",
    "ret",
    options(att_syntax)
);

// These single-use test fixtures are only touched by the boot-time self-test,
// before normal task scheduling starts and with interrupts still disabled.
static mut TEST_BOOT_CONTEXT: CpuContext = CpuContext::empty();
static mut TEST_TASK_CONTEXT: CpuContext = CpuContext::empty();
static mut TEST_TASK_STACK: TaskStack = TaskStack::new();
static mut TEST_STAGE: usize = 0;

extern "C" fn context_test_entry() -> ! {
    unsafe {
        *core::ptr::addr_of_mut!(TEST_STAGE) = 1;
        // Return control to the boot stack. This suspends this task at the
        // switch call; it must never return from the entry point.
        switch(
            &mut *core::ptr::addr_of_mut!(TEST_TASK_CONTEXT),
            &*core::ptr::addr_of!(TEST_BOOT_CONTEXT),
        );
    }
    loop {
        core::hint::spin_loop();
    }
}

/// Validate the synthetic frame and perform a real first dispatch/return.
///
/// This is deliberately a one-shot boot test: it enters a separate stack,
/// records that the task entry ran, and switches back to the boot stack.
pub fn self_test() -> Result<(), ()> {
    unsafe {
        *core::ptr::addr_of_mut!(TEST_BOOT_CONTEXT) = CpuContext::empty();
        *core::ptr::addr_of_mut!(TEST_TASK_CONTEXT) = CpuContext::empty();
        *core::ptr::addr_of_mut!(TEST_STAGE) = 0;

        (&mut *core::ptr::addr_of_mut!(TEST_TASK_STACK)).initialize(
            &mut *core::ptr::addr_of_mut!(TEST_TASK_CONTEXT),
            context_test_entry,
        );

        if !(&*core::ptr::addr_of!(TEST_TASK_CONTEXT)).is_initialized()
            || (&*core::ptr::addr_of!(TEST_TASK_CONTEXT)).rsp % 16 != 0
            || !(&*core::ptr::addr_of!(TEST_TASK_STACK)).guards_intact()
        {
            return Err(());
        }

        let frame = (&*core::ptr::addr_of!(TEST_TASK_CONTEXT)).rsp as *const u64;
        // SAFETY: initialize() wrote eight u64 words within TEST_TASK_STACK.
        let (entry, padding) = unsafe { (*frame.add(6), *frame.add(7)) };
        if entry != context_test_entry as usize as u64 || padding != 0 {
            return Err(());
        }

        // SAFETY: both contexts and the task stack are static, valid, and
        // exclusively used by this one-shot test with interrupts disabled.
        switch(
            &mut *core::ptr::addr_of_mut!(TEST_BOOT_CONTEXT),
            &*core::ptr::addr_of!(TEST_TASK_CONTEXT),
        );

        if *core::ptr::addr_of!(TEST_STAGE) != 1
            || !(&*core::ptr::addr_of!(TEST_TASK_STACK)).guards_intact()
            || !(&*core::ptr::addr_of!(TEST_BOOT_CONTEXT)).is_initialized()
            || !(&*core::ptr::addr_of!(TEST_TASK_CONTEXT)).is_initialized()
        {
            return Err(());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_run_stack_frame_is_aligned_and_performs_context_switch() {
        assert_eq!(self_test(), Ok(()));
    }

    #[test]
    fn stack_canaries_detect_corruption_on_both_sides() {
        let mut stack = TaskStack::new();
        assert!(stack.guards_intact());
        stack.lower_guard[0] ^= 1;
        assert!(!stack.guards_intact());

        let mut stack = TaskStack::new();
        stack.upper_guard[STACK_GUARD_WORDS - 1] ^= 1;
        assert!(!stack.guards_intact());
    }

    #[test]
    fn context_starts_uninitialized() {
        assert!(!CpuContext::empty().is_initialized());
    }

    extern "C" fn never_run() -> ! {
        loop { core::hint::spin_loop(); }
    }

    #[test]
    fn saved_context_must_belong_to_its_own_stack() {
        let mut stack = TaskStack::new();
        let mut other_stack = TaskStack::new();
        let mut context = CpuContext::empty();
        stack.initialize(&mut context, never_run);

        assert!(context.is_initialized());
        assert!(stack.contains_context(&context));
        assert!(!other_stack.contains_context(&context));
        assert!(!stack.contains_context(&CpuContext::empty()));
    }

    #[test]
    fn fxsave_area_is_512_bytes_and_16_byte_aligned_at_abi_offset() {
        let context = CpuContext::empty();
        let base = &context as *const CpuContext as usize;
        let area = &context.simd as *const FxState as usize;
        assert_eq!(core::mem::size_of::<FxState>(), 512);
        assert_eq!(base % 16, 0);
        assert_eq!(area % 16, 0);
        assert_eq!(area - base, 16);
    }

    #[test]
    fn context_layout_matches_offsets_used_by_switch_assembly() {
        let context = CpuContext::empty();
        let base = &context as *const CpuContext as usize;
        let rsp = &context.rsp as *const u64 as usize;
        let reserved = &context._reserved as *const u64 as usize;
        let simd = &context.simd as *const FxState as usize;

        // archiaos_context_switch uses (%rdi), (%rsi), and 16(%rdi/rsi).
        assert_eq!(core::mem::size_of::<CpuContext>(), 528);
        assert_eq!(core::mem::align_of::<CpuContext>(), 16);
        assert_eq!(rsp - base, 0);
        assert_eq!(reserved - base, 8);
        assert_eq!(simd - base, 16);
        assert_eq!(core::mem::size_of::<TaskStack>(), TASK_STACK_SIZE + 2 * 8 * core::mem::size_of::<u64>());
        assert!(TaskStack::new().guards_intact());
        assert_eq!(core::mem::align_of::<TaskStack>(), 16);
    }
}
