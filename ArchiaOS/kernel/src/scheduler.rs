//! Allocation-free cooperative scheduler and x86-64 context integration (P2).
//!
//! The state model remains independently testable. A bounded boot-time runtime
//! test also binds task IDs to dedicated static stacks/CPU contexts and performs
//! real round-robin context switches. Timer preemption and SMP are out of scope.

pub const MAX_TASKS: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TaskId(usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskState {
    Empty,
    Runnable,
    Running,
    Blocked,
    Exited,
}

/// Snapshot of scheduler occupancy for diagnostics and idle-path decisions.
/// Counts are observational only; they never reserve slots or change task state.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TaskCounts {
    pub empty: usize,
    pub runnable: usize,
    pub running: usize,
    pub blocked: usize,
    pub exited: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchedulerError {
    NoTaskSlot,
    InvalidTask,
    NoRunnableTask,
    NoCurrentTask,
    TaskNotBlocked,
    StackCorrupted,
    InvariantViolation,
}

/// Why the scheduler has no runnable task. The caller can wait for a wakeup,
/// stop scheduling permanently, or handle an as-yet empty task table differently.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdleReason {
    /// At least one task is blocked and may become runnable after a wakeup.
    WaitingForWakeup,
    /// At least one task existed, and every created task has exited.
    AllTasksExited,
    /// No task has ever been created in this scheduler instance.
    NoTasks,
}

/// Result of asking the scheduler for work when no task is currently running.
/// Idle decisions describe scheduler state; they do not halt the CPU themselves.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DispatchDecision {
    Run(TaskId),
    Idle(IdleReason),
}

#[derive(Clone, Copy)]
struct Task {
    state: TaskState,
}

impl Task {
    const EMPTY: Self = Self { state: TaskState::Empty };
}

/// Fixed-capacity scheduler model; no heap allocation or implicit task reuse.
pub struct Scheduler {
    tasks: [Task; MAX_TASKS],
    current: Option<usize>,
    cursor: usize,
}

impl Scheduler {
    pub const fn new() -> Self {
        Self {
            tasks: [Task::EMPTY; MAX_TASKS],
            current: None,
            cursor: MAX_TASKS - 1,
        }
    }

    pub fn create_task(&mut self) -> Result<TaskId, SchedulerError> {
        // Keep task creation transactional: never allocate a slot on top of a
        // scheduler whose current-task/cursor bookkeeping is already invalid.
        self.check_invariants()?;
        let index = self.tasks.iter().position(|task| task.state == TaskState::Empty)
            .ok_or(SchedulerError::NoTaskSlot)?;
        self.tasks[index].state = TaskState::Runnable;
        Ok(TaskId(index))
    }

    pub fn state(&self, id: TaskId) -> Result<TaskState, SchedulerError> {
        self.tasks.get(id.0).map(|task| task.state).ok_or(SchedulerError::InvalidTask)
    }

    pub fn current(&self) -> Option<TaskId> {
        self.current.map(TaskId)
    }

    /// Return a read-only count of tasks in each lifecycle state.
    ///
    /// This is intended for diagnostics and tests; callers must still validate
    /// scheduler consistency separately when required.
    pub fn task_counts(&self) -> TaskCounts {
        let mut counts = TaskCounts::default();
        for task in &self.tasks {
            match task.state {
                TaskState::Empty => counts.empty += 1,
                TaskState::Runnable => counts.runnable += 1,
                TaskState::Running => counts.running += 1,
                TaskState::Blocked => counts.blocked += 1,
                TaskState::Exited => counts.exited += 1,
            }
        }
        counts
    }

    /// Start scheduling when no task is currently running.
    ///
    /// This is the public entry point for the first dispatch and for restarting
    /// after all runnable tasks were blocked. It deliberately refuses to replace
    /// an active task; real CPU context switching is implemented separately.
    pub fn schedule(&mut self) -> Result<TaskId, SchedulerError> {
        self.check_invariants()?;
        if self.current.is_some() {
            return Err(SchedulerError::InvariantViolation);
        }
        self.select_next()
    }

    /// Describe the idle state without selecting a task or changing scheduler state.
    ///
    /// Returns `None` while a task is running or any task is runnable. Callers
    /// can use this to decide whether to wait for a wakeup or finish shutdown,
    /// while still keeping CPU-specific idle behavior outside the state model.
    pub fn idle_reason(&self) -> Result<Option<IdleReason>, SchedulerError> {
        self.check_invariants()?;
        if self.current.is_some()
            || self.tasks.iter().any(|task| task.state == TaskState::Runnable)
        {
            return Ok(None);
        }
        let has_blocked = self.tasks.iter().any(|task| task.state == TaskState::Blocked);
        let has_created = self.tasks.iter().any(|task| task.state != TaskState::Empty);
        let reason = if has_blocked {
            IdleReason::WaitingForWakeup
        } else if has_created {
            IdleReason::AllTasksExited
        } else {
            IdleReason::NoTasks
        };
        Ok(Some(reason))
    }

    /// Dispatch a runnable task, or explicitly tell the caller to enter its idle path.
    ///
    /// Unlike `schedule`, an empty runnable set is not an error for this API:
    /// all tasks may be blocked or exited. The caller must still provide the
    /// architecture-specific idle loop/context; this method does not halt the CPU.
    pub fn dispatch_or_idle(&mut self) -> Result<DispatchDecision, SchedulerError> {
        self.check_invariants()?;
        if self.current.is_some() {
            return Err(SchedulerError::InvariantViolation);
        }
        match self.select_next() {
            Ok(id) => Ok(DispatchDecision::Run(id)),
            Err(SchedulerError::NoRunnableTask) => {
                match self.idle_reason()? {
                    Some(reason) => Ok(DispatchDecision::Idle(reason)),
                    None => Err(SchedulerError::InvariantViolation),
                }
            }
            Err(error) => Err(error),
        }
    }

    /// Select the next runnable task in deterministic round-robin order.
    /// The outgoing running task becomes runnable (cooperative yield).
    pub fn yield_current(&mut self) -> Result<TaskId, SchedulerError> {
        self.check_invariants()?;
        let previous = self.current.ok_or(SchedulerError::NoCurrentTask)?;
        if self.tasks[previous].state != TaskState::Running {
            return Err(SchedulerError::InvariantViolation);
        }
        self.tasks[previous].state = TaskState::Runnable;
        self.current = None;
        self.select_next()
    }

    pub fn block_current(&mut self) -> Result<TaskId, SchedulerError> {
        self.check_invariants()?;
        let previous = self.current.ok_or(SchedulerError::NoCurrentTask)?;
        if self.tasks[previous].state != TaskState::Running {
            return Err(SchedulerError::InvariantViolation);
        }
        self.tasks[previous].state = TaskState::Blocked;
        self.current = None;
        self.select_next()
    }

    /// Block the current task and return either the next task or an explicit idle reason.
    ///
    /// Unlike `block_current`, this does not report `NoRunnableTask` as an error
    /// when every remaining task is blocked or exited. The caller must still
    /// provide a safe architecture-level idle context before using this for
    /// real CPU blocking.
    pub fn block_current_or_idle(&mut self) -> Result<DispatchDecision, SchedulerError> {
        self.check_invariants()?;
        let previous = self.current.ok_or(SchedulerError::NoCurrentTask)?;
        if self.tasks[previous].state != TaskState::Running {
            return Err(SchedulerError::InvariantViolation);
        }
        self.tasks[previous].state = TaskState::Blocked;
        self.current = None;
        self.dispatch_or_idle()
    }

    pub fn wake(&mut self, id: TaskId) -> Result<(), SchedulerError> {
        // Never mutate task state on top of a scheduler that is already corrupt.
        self.check_invariants()?;
        let task = self.tasks.get_mut(id.0).ok_or(SchedulerError::InvalidTask)?;
        if task.state != TaskState::Blocked {
            return Err(SchedulerError::TaskNotBlocked);
        }
        task.state = TaskState::Runnable;
        Ok(())
    }

    pub fn exit_current(&mut self) -> Result<TaskId, SchedulerError> {
        self.check_invariants()?;
        let previous = self.current.ok_or(SchedulerError::NoCurrentTask)?;
        if self.tasks[previous].state != TaskState::Running {
            return Err(SchedulerError::InvariantViolation);
        }
        self.tasks[previous].state = TaskState::Exited;
        self.current = None;
        self.select_next()
    }

    /// Exit the current task and return a dispatch or terminal/idle decision.
    ///
    /// An all-exited result is distinct from a temporary wait for blocked tasks.
    /// This is scheduler-state handling only; it does not switch CPU contexts.
    pub fn exit_current_or_idle(&mut self) -> Result<DispatchDecision, SchedulerError> {
        self.check_invariants()?;
        let previous = self.current.ok_or(SchedulerError::NoCurrentTask)?;
        if self.tasks[previous].state != TaskState::Running {
            return Err(SchedulerError::InvariantViolation);
        }
        self.tasks[previous].state = TaskState::Exited;
        self.current = None;
        self.dispatch_or_idle()
    }

    fn select_next(&mut self) -> Result<TaskId, SchedulerError> {
        // Validate before modulo/index arithmetic. A damaged cursor must return
        // a controlled scheduler error rather than panic on overflow or select
        // an unintended slot.
        if self.cursor >= MAX_TASKS {
            return Err(SchedulerError::InvariantViolation);
        }

        for offset in 1..=MAX_TASKS {
            let index = (self.cursor + offset) % MAX_TASKS;
            if self.tasks[index].state == TaskState::Runnable {
                self.tasks[index].state = TaskState::Running;
                self.current = Some(index);
                self.cursor = index;
                return Ok(TaskId(index));
            }
        }
        self.current = None;
        Err(SchedulerError::NoRunnableTask)
    }

    pub fn check_invariants(&self) -> Result<(), SchedulerError> {
        // The round-robin cursor participates in modulo-based slot selection.
        // Treat an out-of-range cursor as corruption instead of silently
        // normalizing it and potentially masking a scheduler state bug.
        if self.cursor >= MAX_TASKS {
            return Err(SchedulerError::InvariantViolation);
        }

        let mut running = 0usize;
        for (index, task) in self.tasks.iter().enumerate() {
            if task.state == TaskState::Running {
                running += 1;
                if self.current != Some(index) {
                    return Err(SchedulerError::InvariantViolation);
                }
            }
        }
        if running > 1 || (self.current.is_some() != (running == 1)) {
            return Err(SchedulerError::InvariantViolation);
        }
        // While a task is running, the round-robin cursor must identify that
        // same task. A valid-but-stale cursor is just as dangerous as an
        // out-of-range cursor: the next transition could skip fairness order
        // or conceal a broken current-task handoff.
        if let Some(current) = self.current {
            if self.cursor != current {
                return Err(SchedulerError::InvariantViolation);
            }
        }
        Ok(())
    }

    /// Deterministic boot-time regression test for task transitions and fairness.
    pub fn self_test() -> Result<(), SchedulerError> {
        let mut scheduler = Self::new();
        let first = scheduler.create_task()?;
        let second = scheduler.create_task()?;
        if scheduler.yield_current() != Err(SchedulerError::NoCurrentTask) {
            return Err(SchedulerError::InvariantViolation);
        }
        if scheduler.select_next()? != first {
            return Err(SchedulerError::InvariantViolation);
        }
        scheduler.check_invariants()?;
        if scheduler.yield_current()? != second {
            return Err(SchedulerError::InvariantViolation);
        }
        if scheduler.block_current()? != first {
            return Err(SchedulerError::InvariantViolation);
        }
        if scheduler.state(second)? != TaskState::Blocked {
            return Err(SchedulerError::InvariantViolation);
        }
        scheduler.wake(second)?;
        if scheduler.yield_current()? != second {
            return Err(SchedulerError::InvariantViolation);
        }
        if scheduler.exit_current()? != first {
            return Err(SchedulerError::InvariantViolation);
        }
        if scheduler.state(second)? != TaskState::Exited {
            return Err(SchedulerError::InvariantViolation);
        }
        scheduler.check_invariants()?;

        let mut full = Self::new();
        for _ in 0..MAX_TASKS {
            full.create_task()?;
        }
        if full.create_task() != Err(SchedulerError::NoTaskSlot) {
            return Err(SchedulerError::InvariantViolation);
        }
        Ok(())
    }
}


// ---- Real three-task runtime integration (x86-64, single CPU) ----
//
// Stacks are static rather than embedded in Scheduler, so a scheduler object
// cannot accidentally consume >100 KiB of the early boot stack. The runtime
// integration exercises three tasks; the state model remains MAX_TASKS-capable
// and can be extended once allocation/lifecycle policy lands.
const RUNTIME_TASKS: usize = 3;

static mut RUNTIME_SCHEDULER: Scheduler = Scheduler::new();
static mut RUNTIME_CONTEXTS: [crate::context::CpuContext; RUNTIME_TASKS] = [
    crate::context::CpuContext::empty(),
    crate::context::CpuContext::empty(),
    crate::context::CpuContext::empty(),
];
static mut RUNTIME_STACKS: [crate::context::TaskStack; RUNTIME_TASKS] = [
    crate::context::TaskStack::new(),
    crate::context::TaskStack::new(),
    crate::context::TaskStack::new(),
];
static mut RUNTIME_BOOT_CONTEXT: crate::context::CpuContext =
    crate::context::CpuContext::empty();
// A dedicated, persistent idle context is separate from ordinary task slots.
// The boot-time test idle entry returns to the harness; it intentionally does
// not execute HLT because this self-test runs before the interrupt/wakeup path.
static mut RUNTIME_IDLE_CONTEXT: crate::context::CpuContext =
    crate::context::CpuContext::empty();
static mut RUNTIME_IDLE_STACK: crate::context::TaskStack = crate::context::TaskStack::new();
static mut RUNTIME_STAGES: [usize; RUNTIME_TASKS] = [0; RUNTIME_TASKS];

extern "C" fn runtime_idle_context_entry() -> ! {
    // The idle stack is live here, so validate its canaries and the scheduler
    // state before handing control back to the boot-time wakeup harness.
    if runtime_validate_boundary().is_err() {
        runtime_halt();
    }
    unsafe {
        crate::context::switch(
            &mut *core::ptr::addr_of_mut!(RUNTIME_IDLE_CONTEXT),
            &*core::ptr::addr_of!(RUNTIME_BOOT_CONTEXT),
        );
    }
    // The harness may later resume this saved idle context, but this test
    // never schedules it again. Do not return through a synthetic entry frame.
    runtime_halt()
}

extern "C" fn runtime_task_a() -> ! {
    // A records entry and blocks. Task B must wake it explicitly before it can
    // resume; this exercises the real context-switch path, not just state data.
    unsafe { core::ptr::write_volatile(core::ptr::addr_of_mut!(RUNTIME_STAGES).cast::<usize>(), 1) };
    if runtime_block_current().is_err() {
        runtime_halt();
    }
    unsafe { core::ptr::write_volatile(core::ptr::addr_of_mut!(RUNTIME_STAGES).cast::<usize>(), 2) };
    runtime_exit()
}

extern "C" fn runtime_task_b() -> ! {
    unsafe {
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!(RUNTIME_STAGES).cast::<usize>().add(1),
            1,
        )
    };
    if runtime_wake(TaskId(0)).is_err() || runtime_yield().is_err() {
        runtime_halt();
    }
    unsafe {
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!(RUNTIME_STAGES).cast::<usize>().add(1),
            2,
        )
    };
    runtime_exit()
}

extern "C" fn runtime_idle_task() -> ! {
    unsafe { core::ptr::write_volatile(core::ptr::addr_of_mut!(RUNTIME_STAGES).cast::<usize>(), 1) };
    if runtime_block_current_to_boot().is_err() {
        runtime_halt();
    }
    unsafe { core::ptr::write_volatile(core::ptr::addr_of_mut!(RUNTIME_STAGES).cast::<usize>(), 2) };
    runtime_exit()
}

extern "C" fn runtime_task_c() -> ! {
    // Task C must run after A exits and before B resumes, proving round-robin
    // dispatch continues correctly across three independently saved contexts.
    unsafe {
        core::ptr::write_volatile(
            core::ptr::addr_of_mut!(RUNTIME_STAGES).cast::<usize>().add(2),
            1,
        )
    };
    runtime_exit()
}

fn runtime_halt() -> ! {
    loop {
        core::hint::spin_loop();
    }
}

/// Validate every statically allocated runtime stack before crossing a
/// context boundary. Canaries are diagnostic checks, not memory protection.
fn runtime_stack_guards_intact() -> bool {
    unsafe {
        let stacks = core::ptr::addr_of!(RUNTIME_STACKS).cast::<crate::context::TaskStack>();
        for index in 0..RUNTIME_TASKS {
            if !(&*stacks.add(index)).guards_intact() {
                return false;
            }
        }
        (&*core::ptr::addr_of!(RUNTIME_IDLE_STACK)).guards_intact()
    }
}

/// Validate the scheduler bookkeeping and every runtime stack at a
/// context boundary. Keep the checks together so new transition paths cannot
/// accidentally validate only one half of the runtime invariants.
fn runtime_validate_boundary() -> Result<(), SchedulerError> {
    if !runtime_stack_guards_intact() {
        return Err(SchedulerError::StackCorrupted);
    }

    unsafe {
        let scheduler = &*core::ptr::addr_of!(RUNTIME_SCHEDULER);
        scheduler.check_invariants()?;

        // Every allocated runtime task must have a valid saved context before
        // it can be blocked, woken, or dispatched. Catch lifecycle/context
        // mismatches here rather than letting assembly restore an empty RSP.
        let contexts = core::ptr::addr_of!(RUNTIME_CONTEXTS).cast::<crate::context::CpuContext>();
        let stacks = core::ptr::addr_of!(RUNTIME_STACKS).cast::<crate::context::TaskStack>();
        for index in 0..RUNTIME_TASKS {
            if scheduler.state(TaskId(index))? != TaskState::Empty {
                let context = &*contexts.add(index);
                let stack = &*stacks.add(index);
                if !context.is_initialized() || !stack.contains_context(context) {
                    return Err(SchedulerError::InvariantViolation);
                }
            }
        }
    }
    Ok(())
}

/// Yield from the currently running runtime task and resume when it is selected
/// again. Call only on the single boot CPU with maskable interrupts disabled.
fn runtime_yield() -> Result<(), SchedulerError> {
    runtime_validate_boundary()?;
    unsafe {
        let scheduler = core::ptr::addr_of_mut!(RUNTIME_SCHEDULER);
        let previous = (*scheduler).current.ok_or(SchedulerError::NoCurrentTask)?;
        let next = (&mut *scheduler).yield_current()?.0;
        if previous == next {
            return Ok(());
        }
        let contexts = core::ptr::addr_of_mut!(RUNTIME_CONTEXTS).cast::<crate::context::CpuContext>();
        let stacks = core::ptr::addr_of!(RUNTIME_STACKS).cast::<crate::context::TaskStack>();
        // SAFETY: task IDs are bounded by the scheduler; the destination
        // context is checked against its persistent stack before assembly runs.
        crate::context::switch_checked(
            &mut *contexts.add(previous),
            &*contexts.add(next),
            &*stacks.add(next),
        ).map_err(|_| SchedulerError::StackCorrupted)?;
    }
    runtime_validate_boundary()?;
    Ok(())
}

/// Block the current runtime task and switch to another runnable task.
///
/// This helper requires another runnable task. The boot-time idle test uses
/// a dedicated idle context, but production blocking still needs an interrupt
/// and wakeup path before this helper can be generalized.
fn runtime_block_current() -> Result<(), SchedulerError> {
    runtime_validate_boundary()?;
    unsafe {
        let scheduler = core::ptr::addr_of_mut!(RUNTIME_SCHEDULER);
        let previous = (*scheduler).current.ok_or(SchedulerError::NoCurrentTask)?;
        let next = (&mut *scheduler).block_current()?.0;
        let contexts = core::ptr::addr_of_mut!(RUNTIME_CONTEXTS).cast::<crate::context::CpuContext>();
        let stacks = core::ptr::addr_of!(RUNTIME_STACKS).cast::<crate::context::TaskStack>();
        // SAFETY: scheduler selected a runnable task; checked switching verifies
        // its destination frame and stack canaries before entering assembly.
        crate::context::switch_checked(
            &mut *contexts.add(previous),
            &*contexts.add(next),
            &*stacks.add(next),
        ).map_err(|_| SchedulerError::StackCorrupted)?;
    }
    runtime_validate_boundary()?;
    Ok(())
}

/// Block a task and hand control to the boot-time scheduler owner at idle.
///
/// This is a deterministic test harness, not a production idle loop: the saved
/// boot context regains control so the test can inject a wakeup without enabling
/// interrupts.
fn runtime_block_current_to_boot() -> Result<(), SchedulerError> {
    runtime_validate_boundary()?;
    unsafe {
        let scheduler = core::ptr::addr_of_mut!(RUNTIME_SCHEDULER);
        let previous = (*scheduler).current.ok_or(SchedulerError::NoCurrentTask)?;
        match (&mut *scheduler).block_current_or_idle()? {
            DispatchDecision::Run(next) => {
                let contexts = core::ptr::addr_of_mut!(RUNTIME_CONTEXTS).cast::<crate::context::CpuContext>();
                let stacks = core::ptr::addr_of!(RUNTIME_STACKS).cast::<crate::context::TaskStack>();
                crate::context::switch_checked(
                    &mut *contexts.add(previous),
                    &*contexts.add(next.0),
                    &*stacks.add(next.0),
                ).map_err(|_| SchedulerError::StackCorrupted)?;
            }
            DispatchDecision::Idle(IdleReason::WaitingForWakeup) => {
                // This test path now crosses a dedicated idle stack/context
                // before returning to the boot harness to inject a wakeup.
                // It remains a deterministic harness, not a hardware idle loop.
                crate::context::switch_checked(
                    &mut *core::ptr::addr_of_mut!(RUNTIME_CONTEXTS).cast::<crate::context::CpuContext>().add(previous),
                    &*core::ptr::addr_of!(RUNTIME_IDLE_CONTEXT),
                    &*core::ptr::addr_of!(RUNTIME_IDLE_STACK),
                ).map_err(|_| SchedulerError::StackCorrupted)?;
            }
            DispatchDecision::Idle(_) => return Err(SchedulerError::InvariantViolation),
        }
    }
    runtime_validate_boundary()?;
    Ok(())
}

/// Wake a blocked task in the boot-time runtime.
fn runtime_wake(id: TaskId) -> Result<(), SchedulerError> {
    runtime_validate_boundary()?;
    unsafe { (&mut *core::ptr::addr_of_mut!(RUNTIME_SCHEDULER)).wake(id)?; }
    runtime_validate_boundary()
}

/// Exit a runtime task permanently. An exited task is never dispatched again.
/// When no runnable task remains, return to the boot context rather than
/// continuing on a task stack that is logically dead.
fn runtime_exit() -> ! {
    if runtime_validate_boundary().is_err() {
        runtime_halt();
    }
    unsafe {
        let scheduler = core::ptr::addr_of_mut!(RUNTIME_SCHEDULER);
        let previous = match (*scheduler).current {
            Some(index) => index,
            None => runtime_halt(),
        };
        match (&mut *scheduler).exit_current_or_idle() {
            Ok(DispatchDecision::Run(next_id)) => {
                let contexts = core::ptr::addr_of_mut!(RUNTIME_CONTEXTS).cast::<crate::context::CpuContext>();
                // SAFETY: scheduler returned a valid distinct runnable task.
                let stacks = core::ptr::addr_of!(RUNTIME_STACKS).cast::<crate::context::TaskStack>();
                if crate::context::switch_checked(
                    &mut *contexts.add(previous),
                    &*contexts.add(next_id.0),
                    &*stacks.add(next_id.0),
                ).is_err() {
                    runtime_halt();
                }
            }
            Ok(DispatchDecision::Idle(IdleReason::AllTasksExited))
            | Ok(DispatchDecision::Idle(IdleReason::WaitingForWakeup))
            | Ok(DispatchDecision::Idle(IdleReason::NoTasks)) => {
                // This runtime is only a boot-time harness; production idle
                // handling must switch to a persistent idle context instead.
                let contexts = core::ptr::addr_of_mut!(RUNTIME_CONTEXTS).cast::<crate::context::CpuContext>();
                crate::context::switch(
                    &mut *contexts.add(previous),
                    &*core::ptr::addr_of!(RUNTIME_BOOT_CONTEXT),
                );
            }
            Err(_) => runtime_halt(),
        }
    }
    // A task which was marked Exited must never be resumed.
    runtime_halt()
}

/// Execute a real three-task scenario on separate stacks:
/// A blocks -> B wakes A and yields -> C exits -> A resumes and exits
/// -> B resumes and exits -> boot. This checks wakeup, round-robin order and
/// three saved contexts.
pub fn runtime_self_test() -> Result<(), SchedulerError> {
    unsafe {
        core::ptr::write(core::ptr::addr_of_mut!(RUNTIME_SCHEDULER), Scheduler::new());
        core::ptr::write(
            core::ptr::addr_of_mut!(RUNTIME_CONTEXTS),
            [crate::context::CpuContext::empty(); RUNTIME_TASKS],
        );
        core::ptr::write(core::ptr::addr_of_mut!(RUNTIME_BOOT_CONTEXT), crate::context::CpuContext::empty());
        core::ptr::write(core::ptr::addr_of_mut!(RUNTIME_STAGES), [0; RUNTIME_TASKS]);

        let scheduler = core::ptr::addr_of_mut!(RUNTIME_SCHEDULER);
        let first = (&mut *scheduler).create_task()?;
        let second = (&mut *scheduler).create_task()?;
        let third = (&mut *scheduler).create_task()?;
        if first.0 != 0 || second.0 != 1 || third.0 != 2 {
            return Err(SchedulerError::InvariantViolation);
        }

        let stacks = core::ptr::addr_of_mut!(RUNTIME_STACKS).cast::<crate::context::TaskStack>();
        let contexts = core::ptr::addr_of_mut!(RUNTIME_CONTEXTS).cast::<crate::context::CpuContext>();
        (&mut *stacks.add(first.0)).initialize(&mut *contexts.add(first.0), runtime_task_a);
        (&mut *stacks.add(second.0)).initialize(&mut *contexts.add(second.0), runtime_task_b);
        (&mut *stacks.add(third.0)).initialize(&mut *contexts.add(third.0), runtime_task_c);

        let initial = (&mut *scheduler).schedule()?;
        if initial != first {
            return Err(SchedulerError::InvariantViolation);
        }
        // SAFETY: boot/task contexts and both dedicated stacks are static,
        // initialized, and exclusively used by this single-threaded self-test.
        crate::context::switch_checked(
            &mut *core::ptr::addr_of_mut!(RUNTIME_BOOT_CONTEXT),
            &*contexts.add(initial.0),
            &*stacks.add(initial.0),
        ).map_err(|_| SchedulerError::StackCorrupted)?;

        let stages = core::ptr::addr_of!(RUNTIME_STAGES).cast::<usize>();
        if core::ptr::read_volatile(stages) != 2
            || core::ptr::read_volatile(stages.add(1)) != 2
            || core::ptr::read_volatile(stages.add(2)) != 1
            || (&*scheduler).state(first)? != TaskState::Exited
            || (&*scheduler).state(second)? != TaskState::Exited
            || (&*scheduler).state(third)? != TaskState::Exited
            || (&*scheduler).current().is_some()
            || !(&*contexts.add(first.0)).is_initialized()
            || !(&*contexts.add(second.0)).is_initialized()
            || !(&*contexts.add(third.0)).is_initialized()
            || !(&*stacks.add(first.0)).guards_intact()
            || !(&*stacks.add(second.0)).guards_intact()
            || !(&*stacks.add(third.0)).guards_intact()
            || !(&*core::ptr::addr_of!(RUNTIME_BOOT_CONTEXT)).is_initialized()
            || (&*scheduler).check_invariants().is_err()
        {
            return Err(SchedulerError::InvariantViolation);
        }
        (&*scheduler).check_invariants()?;
    }
    Ok(())
}

/// Exercise a real block-to-idle boundary, then wake and resume the same task.
///
/// With interrupts deliberately disabled, the boot context acts as the test
/// harness at the idle boundary; this proves context preservation and wakeup
/// without pretending that a hardware interrupt path already exists.
pub fn runtime_idle_wakeup_self_test() -> Result<(), SchedulerError> {
    unsafe {
        core::ptr::write(core::ptr::addr_of_mut!(RUNTIME_SCHEDULER), Scheduler::new());
        core::ptr::write(
            core::ptr::addr_of_mut!(RUNTIME_CONTEXTS),
            [crate::context::CpuContext::empty(); RUNTIME_TASKS],
        );
        core::ptr::write(core::ptr::addr_of_mut!(RUNTIME_BOOT_CONTEXT), crate::context::CpuContext::empty());
        core::ptr::write(core::ptr::addr_of_mut!(RUNTIME_IDLE_CONTEXT), crate::context::CpuContext::empty());
        core::ptr::write(core::ptr::addr_of_mut!(RUNTIME_IDLE_STACK), crate::context::TaskStack::new());
        core::ptr::write(core::ptr::addr_of_mut!(RUNTIME_STAGES), [0; RUNTIME_TASKS]);

        let scheduler = core::ptr::addr_of_mut!(RUNTIME_SCHEDULER);
        let task = (&mut *scheduler).create_task()?;
        let stacks = core::ptr::addr_of_mut!(RUNTIME_STACKS).cast::<crate::context::TaskStack>();
        let contexts = core::ptr::addr_of_mut!(RUNTIME_CONTEXTS).cast::<crate::context::CpuContext>();
        (&mut *stacks.add(task.0)).initialize(&mut *contexts.add(task.0), runtime_idle_task);
        (&mut *core::ptr::addr_of_mut!(RUNTIME_IDLE_STACK)).initialize(
            &mut *core::ptr::addr_of_mut!(RUNTIME_IDLE_CONTEXT),
            runtime_idle_context_entry,
        );

        if (&mut *scheduler).schedule()? != task {
            return Err(SchedulerError::InvariantViolation);
        }
        crate::context::switch_checked(
            &mut *core::ptr::addr_of_mut!(RUNTIME_BOOT_CONTEXT),
            &*contexts.add(task.0),
            &*stacks.add(task.0),
        ).map_err(|_| SchedulerError::StackCorrupted)?;

        let stage = core::ptr::read_volatile(core::ptr::addr_of!(RUNTIME_STAGES).cast::<usize>());
        if stage != 1
            || !(&*stacks.add(task.0)).guards_intact()
            || !(&*core::ptr::addr_of!(RUNTIME_IDLE_STACK)).guards_intact()
            || !(&*core::ptr::addr_of!(RUNTIME_IDLE_CONTEXT)).is_initialized()
            || (&*scheduler).current().is_some()
            || (&*scheduler).state(task)? != TaskState::Blocked
            || (&mut *scheduler).dispatch_or_idle()?
                != DispatchDecision::Idle(IdleReason::WaitingForWakeup)
            || (&*scheduler).check_invariants().is_err()
        {
            return Err(SchedulerError::InvariantViolation);
        }

        (&mut *scheduler).wake(task)?;
        match (&mut *scheduler).dispatch_or_idle()? {
            DispatchDecision::Run(next) if next == task => {}
            _ => return Err(SchedulerError::InvariantViolation),
        }
        crate::context::switch_checked(
            &mut *core::ptr::addr_of_mut!(RUNTIME_BOOT_CONTEXT),
            &*contexts.add(task.0),
            &*stacks.add(task.0),
        ).map_err(|_| SchedulerError::StackCorrupted)?;

        if core::ptr::read_volatile(core::ptr::addr_of!(RUNTIME_STAGES).cast::<usize>()) != 2
            || !(&*stacks.add(task.0)).guards_intact()
            || (&*scheduler).state(task)? != TaskState::Exited
            || (&*scheduler).current().is_some()
            || (&mut *scheduler).dispatch_or_idle()?
                != DispatchDecision::Idle(IdleReason::AllTasksExited)
            || (&*scheduler).check_invariants().is_err()
        {
            return Err(SchedulerError::InvariantViolation);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_robin_block_wake_exit_and_capacity() {
        assert_eq!(Scheduler::self_test(), Ok(()));
    }

    #[test]
    fn task_counts_track_create_block_wake_and_exit_without_mutating_state() {
        let mut scheduler = Scheduler::new();
        assert_eq!(scheduler.task_counts(), TaskCounts {
            empty: MAX_TASKS, runnable: 0, running: 0, blocked: 0, exited: 0,
        });

        let first = scheduler.create_task().unwrap();
        let second = scheduler.create_task().unwrap();
        assert_eq!(scheduler.task_counts(), TaskCounts {
            empty: MAX_TASKS - 2, runnable: 2, running: 0, blocked: 0, exited: 0,
        });

        assert_eq!(scheduler.schedule(), Ok(first));
        assert_eq!(scheduler.task_counts(), TaskCounts {
            empty: MAX_TASKS - 2, runnable: 1, running: 1, blocked: 0, exited: 0,
        });

        assert_eq!(scheduler.block_current_or_idle(), Ok(DispatchDecision::Run(second)));
        assert_eq!(scheduler.task_counts(), TaskCounts {
            empty: MAX_TASKS - 2, runnable: 0, running: 1, blocked: 1, exited: 0,
        });

        assert_eq!(scheduler.exit_current_or_idle(), Ok(DispatchDecision::Idle(IdleReason::WaitingForWakeup)));
        assert_eq!(scheduler.task_counts(), TaskCounts {
            empty: MAX_TASKS - 2, runnable: 0, running: 0, blocked: 1, exited: 1,
        });

        scheduler.wake(first).unwrap();
        assert_eq!(scheduler.task_counts(), TaskCounts {
            empty: MAX_TASKS - 2, runnable: 1, running: 0, blocked: 0, exited: 1,
        });
        assert_eq!(scheduler.dispatch_or_idle(), Ok(DispatchDecision::Run(first)));
        assert_eq!(scheduler.task_counts(), TaskCounts {
            empty: MAX_TASKS - 2, runnable: 0, running: 1, blocked: 0, exited: 1,
        });
        scheduler.check_invariants().unwrap();
    }

    #[test]
    fn rejects_waking_non_blocked_task() {
        let mut scheduler = Scheduler::new();
        let task = scheduler.create_task().unwrap();
        assert_eq!(scheduler.wake(task), Err(SchedulerError::TaskNotBlocked));
        assert_eq!(scheduler.state(TaskId(MAX_TASKS)), Err(SchedulerError::InvalidTask));
    }

    #[test]
    fn no_runnable_task_after_all_tasks_block() {
        let mut scheduler = Scheduler::new();
        let task = scheduler.create_task().unwrap();
        assert_eq!(scheduler.select_next(), Ok(task));
        assert_eq!(scheduler.block_current(), Err(SchedulerError::NoRunnableTask));
        assert_eq!(scheduler.current(), None);
        assert_eq!(scheduler.check_invariants(), Ok(()));
    }

    #[test]
    fn dispatch_returns_idle_when_every_task_is_blocked_then_resumes_after_wake() {
        let mut scheduler = Scheduler::new();
        let task = scheduler.create_task().unwrap();

        assert_eq!(scheduler.schedule(), Ok(task));
        assert_eq!(scheduler.block_current(), Err(SchedulerError::NoRunnableTask));
        assert_eq!(scheduler.dispatch_or_idle(), Ok(DispatchDecision::Idle(IdleReason::WaitingForWakeup)));
        assert_eq!(scheduler.current(), None);
        assert_eq!(scheduler.state(task), Ok(TaskState::Blocked));
        assert_eq!(scheduler.check_invariants(), Ok(()));

        scheduler.wake(task).unwrap();
        assert_eq!(scheduler.dispatch_or_idle(), Ok(DispatchDecision::Run(task)));
        assert_eq!(scheduler.current(), Some(task));
        assert_eq!(scheduler.check_invariants(), Ok(()));
    }

    #[test]
    fn dispatch_returns_idle_after_all_tasks_exit() {
        let mut scheduler = Scheduler::new();
        let task = scheduler.create_task().unwrap();

        assert_eq!(scheduler.schedule(), Ok(task));
        assert_eq!(scheduler.exit_current(), Err(SchedulerError::NoRunnableTask));
        assert_eq!(scheduler.dispatch_or_idle(), Ok(DispatchDecision::Idle(IdleReason::AllTasksExited)));
        assert_eq!(scheduler.state(task), Ok(TaskState::Exited));
        assert_eq!(scheduler.check_invariants(), Ok(()));
    }

    #[test]
    fn idle_reason_is_observational_and_tracks_scheduler_lifecycle() {
        let mut scheduler = Scheduler::new();
        assert_eq!(scheduler.idle_reason(), Ok(Some(IdleReason::NoTasks)));

        let task = scheduler.create_task().unwrap();
        assert_eq!(scheduler.idle_reason(), Ok(None));
        assert_eq!(scheduler.schedule(), Ok(task));
        assert_eq!(scheduler.idle_reason(), Ok(None));
        assert_eq!(scheduler.block_current_or_idle(), Ok(DispatchDecision::Idle(IdleReason::WaitingForWakeup)));
        assert_eq!(scheduler.idle_reason(), Ok(Some(IdleReason::WaitingForWakeup)));
        assert_eq!(scheduler.state(task), Ok(TaskState::Blocked));

        scheduler.wake(task).unwrap();
        assert_eq!(scheduler.idle_reason(), Ok(None));
        assert_eq!(scheduler.dispatch_or_idle(), Ok(DispatchDecision::Run(task)));
        assert_eq!(scheduler.exit_current_or_idle(), Ok(DispatchDecision::Idle(IdleReason::AllTasksExited)));
        assert_eq!(scheduler.idle_reason(), Ok(Some(IdleReason::AllTasksExited)));
        assert_eq!(scheduler.check_invariants(), Ok(()));
    }

    #[test]
    fn empty_scheduler_reports_no_tasks_not_a_waiting_idle_state() {
        let mut scheduler = Scheduler::new();
        assert_eq!(scheduler.dispatch_or_idle(), Ok(DispatchDecision::Idle(IdleReason::NoTasks)));
        assert_eq!(scheduler.current(), None);
        assert_eq!(scheduler.check_invariants(), Ok(()));
    }

    #[test]
    fn block_current_or_idle_reports_waiting_without_losing_blocked_tasks() {
        let mut scheduler = Scheduler::new();
        let first = scheduler.create_task().unwrap();
        let second = scheduler.create_task().unwrap();

        assert_eq!(scheduler.schedule(), Ok(first));
        assert_eq!(scheduler.block_current_or_idle(), Ok(DispatchDecision::Run(second)));
        assert_eq!(scheduler.state(first), Ok(TaskState::Blocked));
        assert_eq!(scheduler.current(), Some(second));

        assert_eq!(
            scheduler.block_current_or_idle(),
            Ok(DispatchDecision::Idle(IdleReason::WaitingForWakeup))
        );
        assert_eq!(scheduler.current(), None);
        assert_eq!(scheduler.state(first), Ok(TaskState::Blocked));
        assert_eq!(scheduler.state(second), Ok(TaskState::Blocked));
        assert_eq!(scheduler.check_invariants(), Ok(()));
    }

    #[test]
    fn exit_current_or_idle_distinguishes_remaining_work_from_completion() {
        let mut scheduler = Scheduler::new();
        let first = scheduler.create_task().unwrap();
        let second = scheduler.create_task().unwrap();

        assert_eq!(scheduler.schedule(), Ok(first));
        assert_eq!(scheduler.exit_current_or_idle(), Ok(DispatchDecision::Run(second)));
        assert_eq!(scheduler.state(first), Ok(TaskState::Exited));
        assert_eq!(scheduler.current(), Some(second));

        assert_eq!(
            scheduler.exit_current_or_idle(),
            Ok(DispatchDecision::Idle(IdleReason::AllTasksExited))
        );
        assert_eq!(scheduler.current(), None);
        assert_eq!(scheduler.state(second), Ok(TaskState::Exited));
        assert_eq!(scheduler.check_invariants(), Ok(()));
    }

    #[test]
    fn exiting_last_runnable_task_keeps_blocked_work_in_waiting_state() {
        let mut scheduler = Scheduler::new();
        let blocked = scheduler.create_task().unwrap();
        let running = scheduler.create_task().unwrap();

        assert_eq!(scheduler.schedule(), Ok(blocked));
        assert_eq!(scheduler.block_current_or_idle(), Ok(DispatchDecision::Run(running)));
        assert_eq!(scheduler.state(blocked), Ok(TaskState::Blocked));
        assert_eq!(scheduler.current(), Some(running));

        assert_eq!(
            scheduler.exit_current_or_idle(),
            Ok(DispatchDecision::Idle(IdleReason::WaitingForWakeup))
        );
        assert_eq!(scheduler.current(), None);
        assert_eq!(scheduler.state(blocked), Ok(TaskState::Blocked));
        assert_eq!(scheduler.state(running), Ok(TaskState::Exited));
        assert_eq!(scheduler.check_invariants(), Ok(()));

        scheduler.wake(blocked).unwrap();
        assert_eq!(scheduler.dispatch_or_idle(), Ok(DispatchDecision::Run(blocked)));
        assert_eq!(scheduler.state(blocked), Ok(TaskState::Running));
        assert_eq!(scheduler.check_invariants(), Ok(()));
    }

    #[test]
    fn waking_one_of_multiple_blocked_tasks_preserves_round_robin_selection() {
        let mut scheduler = Scheduler::new();
        let first = scheduler.create_task().unwrap();
        let second = scheduler.create_task().unwrap();
        let third = scheduler.create_task().unwrap();

        assert_eq!(scheduler.schedule(), Ok(first));
        assert_eq!(scheduler.block_current_or_idle(), Ok(DispatchDecision::Run(second)));
        assert_eq!(scheduler.block_current_or_idle(), Ok(DispatchDecision::Run(third)));
        assert_eq!(
            scheduler.block_current_or_idle(),
            Ok(DispatchDecision::Idle(IdleReason::WaitingForWakeup))
        );

        scheduler.wake(second).unwrap();
        assert_eq!(scheduler.dispatch_or_idle(), Ok(DispatchDecision::Run(second)));
        assert_eq!(scheduler.state(first), Ok(TaskState::Blocked));
        assert_eq!(scheduler.state(second), Ok(TaskState::Running));
        assert_eq!(scheduler.state(third), Ok(TaskState::Blocked));
        assert_eq!(scheduler.check_invariants(), Ok(()));
    }

    #[test]
    fn repeated_wake_is_rejected_without_corrupting_runnable_state() {
        let mut scheduler = Scheduler::new();
        let task = scheduler.create_task().unwrap();
        assert_eq!(scheduler.schedule(), Ok(task));
        assert_eq!(scheduler.block_current(), Err(SchedulerError::NoRunnableTask));
        assert_eq!(scheduler.wake(task), Ok(()));
        assert_eq!(scheduler.wake(task), Err(SchedulerError::TaskNotBlocked));
        assert_eq!(scheduler.state(task), Ok(TaskState::Runnable));
        assert_eq!(scheduler.dispatch_or_idle(), Ok(DispatchDecision::Run(task)));
        assert_eq!(scheduler.state(task), Ok(TaskState::Running));
        assert_eq!(scheduler.check_invariants(), Ok(()));
    }

    #[test]
    fn invariant_check_rejects_corrupted_round_robin_cursor() {
        let mut scheduler = Scheduler::new();
        scheduler.create_task().unwrap();
        scheduler.check_invariants().unwrap();

        // Unit tests can inspect private scheduler internals; production code
        // cannot write the cursor directly.
        scheduler.cursor = MAX_TASKS;
        assert_eq!(scheduler.check_invariants(), Err(SchedulerError::InvariantViolation));
        assert_eq!(scheduler.schedule(), Err(SchedulerError::InvariantViolation));
        assert_eq!(scheduler.state(TaskId(0)), Ok(TaskState::Runnable));
        assert_eq!(scheduler.current(), None);
        assert_eq!(scheduler.wake(TaskId(0)), Err(SchedulerError::InvariantViolation));
        assert_eq!(scheduler.state(TaskId(0)), Ok(TaskState::Runnable));
        assert_eq!(scheduler.create_task(), Err(SchedulerError::InvariantViolation));
        // An in-range slot is a valid index even when it is still Empty.
        assert_eq!(scheduler.state(TaskId(1)), Ok(TaskState::Empty));

        let mut active = Scheduler::new();
        let active_task = active.create_task().unwrap();
        assert_eq!(active.schedule(), Ok(active_task));
        active.cursor = MAX_TASKS;
        assert_eq!(active.yield_current(), Err(SchedulerError::InvariantViolation));
        assert_eq!(active.state(active_task), Ok(TaskState::Running));
        assert_eq!(active.current(), Some(active_task));

        // In-range cursor corruption must also be rejected before a task
        // transition mutates the currently running task.
        active.cursor = (active_task.0 + 1) % MAX_TASKS;
        assert_eq!(active.check_invariants(), Err(SchedulerError::InvariantViolation));
        assert_eq!(active.yield_current(), Err(SchedulerError::InvariantViolation));
        assert_eq!(active.state(active_task), Ok(TaskState::Running));
        assert_eq!(active.current(), Some(active_task));
    }

    #[test]
    fn invalid_wake_and_dispatch_while_running_do_not_mutate_scheduler() {
        let mut scheduler = Scheduler::new();
        let task = scheduler.create_task().unwrap();
        assert_eq!(scheduler.wake(TaskId(MAX_TASKS)), Err(SchedulerError::InvalidTask));
        assert_eq!(scheduler.schedule(), Ok(task));
        assert_eq!(scheduler.schedule(), Err(SchedulerError::InvariantViolation));
        assert_eq!(scheduler.dispatch_or_idle(), Err(SchedulerError::InvariantViolation));
        assert_eq!(scheduler.current(), Some(task));
        assert_eq!(scheduler.state(task), Ok(TaskState::Running));
        assert_eq!(scheduler.check_invariants(), Ok(()));
    }

    #[test]
    fn block_and_exit_without_current_task_fail_without_mutating_idle_state() {
        let mut scheduler = Scheduler::new();
        let task = scheduler.create_task().unwrap();

        assert_eq!(scheduler.block_current_or_idle(), Err(SchedulerError::NoCurrentTask));
        assert_eq!(scheduler.exit_current_or_idle(), Err(SchedulerError::NoCurrentTask));
        assert_eq!(scheduler.current(), None);
        assert_eq!(scheduler.state(task), Ok(TaskState::Runnable));
        assert_eq!(scheduler.check_invariants(), Ok(()));

        assert_eq!(scheduler.schedule(), Ok(task));
        assert_eq!(scheduler.block_current_or_idle(), Ok(DispatchDecision::Idle(IdleReason::WaitingForWakeup)));
        assert_eq!(scheduler.block_current_or_idle(), Err(SchedulerError::NoCurrentTask));
        assert_eq!(scheduler.current(), None);
        assert_eq!(scheduler.state(task), Ok(TaskState::Blocked));
        assert_eq!(scheduler.check_invariants(), Ok(()));
    }

    #[test]
    fn deterministic_lifecycle_stress_preserves_invariants_after_every_transition() {
        let mut scheduler = Scheduler::new();
        let mut ids = [None; MAX_TASKS];
        let mut created = 0usize;
        let mut random = 0xA5A5_1234u32;

        for step in 0..10_000usize {
            // Small deterministic PRNG: reproducible transition coverage with
            // no dependency and no flaky host randomness.
            random = random.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let current = scheduler.current();

            if current.is_none() {
                let runnable_exists = (0..created).any(|i| {
                    ids[i].and_then(|id| scheduler.state(id).ok()) == Some(TaskState::Runnable)
                });
                if runnable_exists {
                    let _ = scheduler.schedule();
                } else {
                    let blocked = (0..created).find_map(|i| {
                        let id = ids[i]?;
                        (scheduler.state(id).ok() == Some(TaskState::Blocked)).then_some(id)
                    });
                    if let Some(id) = blocked {
                        scheduler.wake(id).unwrap();
                    } else if created < MAX_TASKS {
                        ids[created] = Some(scheduler.create_task().unwrap());
                        created += 1;
                    } else {
                        // The table can be terminal when every task exited.
                        // There is intentionally no implicit slot reuse.
                        assert!(scheduler.task_counts().blocked == 0);
                        assert!(scheduler.task_counts().runnable == 0);
                    }
                }
            } else {
                match random % 3 {
                    0 => { let _ = scheduler.yield_current(); }
                    1 => { let _ = scheduler.block_current_or_idle(); }
                    _ => { let _ = scheduler.exit_current_or_idle(); }
                }
            }

            scheduler.check_invariants().unwrap_or_else(|error| {
                panic!("scheduler invariant failed at deterministic step {step}: {error:?}")
            });
            let counts = scheduler.task_counts();
            assert_eq!(counts.empty + counts.runnable + counts.running + counts.blocked + counts.exited, MAX_TASKS);
            assert!(counts.running <= 1);
            assert_eq!(counts.running, usize::from(scheduler.current().is_some()));
        }
    }

    #[test]
    fn round_robin_wraps_across_full_task_capacity() {
        let mut scheduler = Scheduler::new();
        let mut ids = [TaskId(0); MAX_TASKS];

        for slot in 0..MAX_TASKS {
            ids[slot] = scheduler.create_task().unwrap();
        }
        assert_eq!(scheduler.schedule(), Ok(ids[0]));

        for slot in 1..MAX_TASKS {
            assert_eq!(scheduler.yield_current(), Ok(ids[slot]));
            assert_eq!(scheduler.current(), Some(ids[slot]));
            scheduler.check_invariants().unwrap();
        }

        // The cursor must wrap from the final slot back to the first runnable
        // task without skipping a slot or creating two Running tasks.
        assert_eq!(scheduler.yield_current(), Ok(ids[0]));
        assert_eq!(scheduler.current(), Some(ids[0]));
        for slot in 0..MAX_TASKS {
            let expected = if slot == 0 { TaskState::Running } else { TaskState::Runnable };
            assert_eq!(scheduler.state(ids[slot]), Ok(expected));
        }
        scheduler.check_invariants().unwrap();
    }

    #[test]
    fn exited_slots_are_not_reused_implicitly() {
        let mut scheduler = Scheduler::new();
        let mut ids = [TaskId(0); MAX_TASKS];

        for slot in 0..MAX_TASKS {
            ids[slot] = scheduler.create_task().unwrap();
        }
        assert_eq!(scheduler.create_task(), Err(SchedulerError::NoTaskSlot));

        // Exiting a task frees execution, not its identity/slot. Reuse must be
        // an explicit future lifecycle policy rather than an accidental alias.
        assert_eq!(scheduler.schedule(), Ok(ids[0]));
        assert_eq!(scheduler.exit_current(), Ok(ids[1]));
        assert_eq!(scheduler.state(ids[0]), Ok(TaskState::Exited));
        assert_eq!(scheduler.create_task(), Err(SchedulerError::NoTaskSlot));
        assert_eq!(scheduler.state(ids[0]), Ok(TaskState::Exited));
        assert_eq!(scheduler.state(ids[MAX_TASKS - 1]), Ok(TaskState::Runnable));
        assert_eq!(scheduler.check_invariants(), Ok(()));
    }

    #[test]
    fn no_runnable_task_errors_leave_blocked_or_exited_state_consistent() {
        let mut blocked = Scheduler::new();
        let blocked_task = blocked.create_task().unwrap();
        assert_eq!(blocked.schedule(), Ok(blocked_task));
        assert_eq!(blocked.block_current(), Err(SchedulerError::NoRunnableTask));
        assert_eq!(blocked.current(), None);
        assert_eq!(blocked.state(blocked_task), Ok(TaskState::Blocked));
        assert_eq!(blocked.check_invariants(), Ok(()));

        let mut exited = Scheduler::new();
        let exited_task = exited.create_task().unwrap();
        assert_eq!(exited.schedule(), Ok(exited_task));
        assert_eq!(exited.exit_current(), Err(SchedulerError::NoRunnableTask));
        assert_eq!(exited.current(), None);
        assert_eq!(exited.state(exited_task), Ok(TaskState::Exited));
        assert_eq!(exited.check_invariants(), Ok(()));
    }
}
