//! Allocation-free cooperative scheduler bookkeeping (P2 bring-up).
//!
//! This module deliberately does not perform a CPU context switch yet. It
//! establishes and tests the task-state/round-robin contract before assembly
//! context switching, per-task stacks, interrupts, or preemption are added.

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
}
