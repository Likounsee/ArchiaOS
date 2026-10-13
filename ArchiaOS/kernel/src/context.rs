//! Minimal x86-64 cooperative CPU context primitive (P2).
//!
//! Saves/restores the System V callee-saved register set and RSP. This is the
//! low-level primitive for a later scheduler integration; it does not itself
//! choose tasks, enable interrupts, or provide synchronization.

use core::arch::global_asm;

pub const TASK_STACK_SIZE: usize = 16 * 1024;

#[repr(C, align(16))]
pub struct TaskStack([u8; TASK_STACK_SIZE]);

impl TaskStack {
    pub const fn new() -> Self {
        Self([0; TASK_STACK_SIZE])
    }

    /// Prepare a never-run task to begin at `entry` after the first context switch.
    pub fn initialize(&mut self, context: &mut CpuContext, entry: extern "C" fn() -> !) {
        // The stack type guarantees a 16-byte-aligned top. The extra final
        // slot preserves the SysV function-entry RSP alignment after `ret`.
        let top = self.0.as_mut_ptr().wrapping_add(TASK_STACK_SIZE) as usize;
        let frame = (top - 8 * core::mem::size_of::<u64>()) as *mut u64;
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

#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct CpuContext {
    rsp: u64,
}

impl CpuContext {
    pub const fn empty() -> Self {
        Self { rsp: 0 }
    }

    pub fn is_initialized(&self) -> bool {
        self.rsp != 0 && self.rsp % 16 == 0
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
    unsafe { archiaos_context_switch(&mut old.rsp, &new.rsp) };
}

extern "C" {
    fn archiaos_context_switch(old_rsp: *mut u64, new_rsp: *const u64);
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

        if !(&*core::ptr::addr_of!(TEST_TASK_CONTEXT)).is_initialized() {
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
    fn context_starts_uninitialized() {
        assert!(!CpuContext::empty().is_initialized());
    }
}
