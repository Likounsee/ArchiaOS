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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchedulerError {
    NoTaskSlot,
    InvalidTask,
    NoRunnableTask,
    NoCurrentTask,
    TaskNotBlocked,
    InvariantViolation,
}

/// Result of asking the scheduler for work when no task is currently running.
/// `Idle` is a decision for the caller to enter its idle path; it is not a task.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DispatchDecision {
    Run(TaskId),
    Idle,
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

    /// Start scheduling when no task is currently running.
    ///
    /// This is the public entry point for the first dispatch and for restarting
    /// after all runnable tasks were blocked. It deliberately refuses to replace
    /// an active task; real CPU context switching is implemented separately.
    pub fn schedule(&mut self) -> Result<TaskId, SchedulerError> {
        if self.current.is_some() {
            return Err(SchedulerError::InvariantViolation);
        }
        self.select_next()
    }

    /// Dispatch a runnable task, or explicitly tell the caller to enter its idle path.
    ///
    /// Unlike `schedule`, an empty runnable set is not an error for this API:
    /// all tasks may be blocked or exited. The caller must still provide the
    /// architecture-specific idle loop/context; this method does not halt the CPU.
    pub fn dispatch_or_idle(&mut self) -> Result<DispatchDecision, SchedulerError> {
        if self.current.is_some() {
            return Err(SchedulerError::InvariantViolation);
        }
        match self.select_next() {
            Ok(id) => Ok(DispatchDecision::Run(id)),
            Err(SchedulerError::NoRunnableTask) => Ok(DispatchDecision::Idle),
            Err(error) => Err(error),
        }
    }

    /// Select the next runnable task in deterministic round-robin order.
    /// The outgoing running task becomes runnable (cooperative yield).
    pub fn yield_current(&mut self) -> Result<TaskId, SchedulerError> {
        let previous = self.current.ok_or(SchedulerError::NoCurrentTask)?;
        if self.tasks[previous].state != TaskState::Running {
            return Err(SchedulerError::InvariantViolation);
        }
        self.tasks[previous].state = TaskState::Runnable;
        self.current = None;
        self.select_next()
    }

    pub fn block_current(&mut self) -> Result<TaskId, SchedulerError> {
        let previous = self.current.ok_or(SchedulerError::NoCurrentTask)?;
        if self.tasks[previous].state != TaskState::Running {
            return Err(SchedulerError::InvariantViolation);
        }
        self.tasks[previous].state = TaskState::Blocked;
        self.current = None;
        self.select_next()
    }

    pub fn wake(&mut self, id: TaskId) -> Result<(), SchedulerError> {
        let task = self.tasks.get_mut(id.0).ok_or(SchedulerError::InvalidTask)?;
        if task.state != TaskState::Blocked {
            return Err(SchedulerError::TaskNotBlocked);
        }
        task.state = TaskState::Runnable;
        Ok(())
    }

    pub fn exit_current(&mut self) -> Result<TaskId, SchedulerError> {
        let previous = self.current.ok_or(SchedulerError::NoCurrentTask)?;
        if self.tasks[previous].state != TaskState::Running {
            return Err(SchedulerError::InvariantViolation);
        }
        self.tasks[previous].state = TaskState::Exited;
        self.current = None;
        self.select_next()
    }

    fn select_next(&mut self) -> Result<TaskId, SchedulerError> {
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


// ---- First real two-task runtime integration (x86-64, single CPU) ----
//
// Stacks are static rather than embedded in Scheduler, so a scheduler object
// cannot accidentally consume >100 KiB of the early boot stack. The first
// runtime slice intentionally supports two tasks; the state model remains
// MAX_TASKS-capable and can be extended once allocation/lifecycle policy lands.
const RUNTIME_TASKS: usize = 2;

static mut RUNTIME_SCHEDULER: Scheduler = Scheduler::new();
static mut RUNTIME_CONTEXTS: [crate::context::CpuContext; RUNTIME_TASKS] = [
    crate::context::CpuContext::empty(),
    crate::context::CpuContext::empty(),
];
static mut RUNTIME_STACKS: [crate::context::TaskStack; RUNTIME_TASKS] = [
    crate::context::TaskStack::new(),
    crate::context::TaskStack::new(),
];
static mut RUNTIME_BOOT_CONTEXT: crate::context::CpuContext =
    crate::context::CpuContext::empty();
static mut RUNTIME_STAGES: [usize; RUNTIME_TASKS] = [0; RUNTIME_TASKS];

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

fn runtime_halt() -> ! {
    loop {
        core::hint::spin_loop();
    }
}

/// Yield from the currently running runtime task and resume when it is selected
/// again. Call only on the single boot CPU with maskable interrupts disabled.
fn runtime_yield() -> Result<(), SchedulerError> {
    unsafe {
        let scheduler = core::ptr::addr_of_mut!(RUNTIME_SCHEDULER);
        let previous = (*scheduler).current.ok_or(SchedulerError::NoCurrentTask)?;
        let next = (&mut *scheduler).yield_current()?.0;
        if previous == next {
            return Ok(());
        }
        let contexts = core::ptr::addr_of_mut!(RUNTIME_CONTEXTS).cast::<crate::context::CpuContext>();
        // SAFETY: task IDs are bounded by the scheduler; the two distinct
        // contexts are static, and their stacks remain alive for this test.
        crate::context::switch(&mut *contexts.add(previous), &*contexts.add(next));
    }
    Ok(())
}

/// Block the current runtime task and switch away until another task wakes it.
fn runtime_block_current() -> Result<(), SchedulerError> {
    unsafe {
        let scheduler = core::ptr::addr_of_mut!(RUNTIME_SCHEDULER);
        let previous = (*scheduler).current.ok_or(SchedulerError::NoCurrentTask)?;
        let next = (&mut *scheduler).block_current()?.0;
        let contexts = core::ptr::addr_of_mut!(RUNTIME_CONTEXTS).cast::<crate::context::CpuContext>();
        // SAFETY: scheduler selected a runnable task; both contexts and stacks
        // are static and interrupts are disabled for this single-CPU test.
        crate::context::switch(&mut *contexts.add(previous), &*contexts.add(next));
    }
    Ok(())
}

/// Wake a blocked task in the boot-time runtime.
fn runtime_wake(id: TaskId) -> Result<(), SchedulerError> {
    unsafe { (&mut *core::ptr::addr_of_mut!(RUNTIME_SCHEDULER)).wake(id) }
}

/// Exit a runtime task permanently. An exited task is never dispatched again.
/// When no runnable task remains, return to the boot context rather than
/// continuing on a task stack that is logically dead.
fn runtime_exit() -> ! {
    unsafe {
        let scheduler = core::ptr::addr_of_mut!(RUNTIME_SCHEDULER);
        let previous = match (*scheduler).current {
            Some(index) => index,
            None => runtime_halt(),
        };
        match (&mut *scheduler).exit_current() {
            Ok(next_id) => {
                let contexts = core::ptr::addr_of_mut!(RUNTIME_CONTEXTS).cast::<crate::context::CpuContext>();
                // SAFETY: scheduler returned a valid distinct runnable task.
                crate::context::switch(&mut *contexts.add(previous), &*contexts.add(next_id.0));
            }
            Err(SchedulerError::NoRunnableTask) => {
                let contexts = core::ptr::addr_of_mut!(RUNTIME_CONTEXTS).cast::<crate::context::CpuContext>();
                // SAFETY: boot context was saved by the initial dispatch.
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

/// Execute a real block/wake scenario on separate stacks:
/// A blocks -> B wakes A and yields -> A resumes and exits -> B resumes and exits
/// -> boot. This checks blocked-task wakeup and the real saved contexts.
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
        if first.0 != 0 || second.0 != 1 {
            return Err(SchedulerError::InvariantViolation);
        }

        let stacks = core::ptr::addr_of_mut!(RUNTIME_STACKS).cast::<crate::context::TaskStack>();
        let contexts = core::ptr::addr_of_mut!(RUNTIME_CONTEXTS).cast::<crate::context::CpuContext>();
        (&mut *stacks.add(first.0)).initialize(&mut *contexts.add(first.0), runtime_task_a);
        (&mut *stacks.add(second.0)).initialize(&mut *contexts.add(second.0), runtime_task_b);

        let initial = (&mut *scheduler).schedule()?;
        if initial != first {
            return Err(SchedulerError::InvariantViolation);
        }
        // SAFETY: boot/task contexts and both dedicated stacks are static,
        // initialized, and exclusively used by this single-threaded self-test.
        crate::context::switch(
            &mut *core::ptr::addr_of_mut!(RUNTIME_BOOT_CONTEXT),
            &*contexts.add(initial.0),
        );

        let stages = core::ptr::addr_of!(RUNTIME_STAGES).cast::<usize>();
        if core::ptr::read_volatile(stages) != 2
            || core::ptr::read_volatile(stages.add(1)) != 2
            || (&*scheduler).state(first)? != TaskState::Exited
            || (&*scheduler).state(second)? != TaskState::Exited
            || (&*scheduler).current().is_some()
            || (&*scheduler).check_invariants().is_err()
        {
            return Err(SchedulerError::InvariantViolation);
        }
        (&*scheduler).check_invariants()?;
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
        assert_eq!(scheduler.dispatch_or_idle(), Ok(DispatchDecision::Idle));
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
        assert_eq!(scheduler.dispatch_or_idle(), Ok(DispatchDecision::Idle));
        assert_eq!(scheduler.state(task), Ok(TaskState::Exited));
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
