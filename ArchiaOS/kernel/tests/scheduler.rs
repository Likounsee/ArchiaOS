//! Host-runnable regression tests for the scheduler state model.
//!
//! This includes the production module directly so the fixed-capacity scheduler
//! can be tested on the host without trying to link the freestanding kernel.
#[path = "../src/scheduler.rs"]
mod scheduler;

use scheduler::{Scheduler, SchedulerError, TaskState};

#[test]
fn public_dispatch_starts_and_refuses_to_replace_running_task() {
    let mut scheduler = Scheduler::new();
    let first = scheduler.create_task().expect("first task slot");
    let second = scheduler.create_task().expect("second task slot");

    assert_eq!(scheduler.schedule(), Ok(first));
    assert_eq!(scheduler.schedule(), Err(SchedulerError::InvariantViolation));
    assert_eq!(scheduler.current(), Some(first));
    assert_eq!(scheduler.state(first), Ok(TaskState::Running));
    assert_eq!(scheduler.state(second), Ok(TaskState::Runnable));
    assert_eq!(scheduler.check_invariants(), Ok(()));
}

#[test]
fn scheduler_can_restart_after_every_task_is_blocked() {
    let mut scheduler = Scheduler::new();
    let task = scheduler.create_task().expect("task slot");

    assert_eq!(scheduler.schedule(), Ok(task));
    assert_eq!(scheduler.block_current(), Err(SchedulerError::NoRunnableTask));
    assert_eq!(scheduler.current(), None);
    assert_eq!(scheduler.state(task), Ok(TaskState::Blocked));

    scheduler.wake(task).expect("blocked task should wake");
    assert_eq!(scheduler.schedule(), Ok(task));
    assert_eq!(scheduler.check_invariants(), Ok(()));
}

#[test]
fn exited_task_is_not_dispatchable_or_wakeable() {
    let mut scheduler = Scheduler::new();
    let task = scheduler.create_task().expect("task slot");

    assert_eq!(scheduler.schedule(), Ok(task));
    assert_eq!(scheduler.exit_current(), Err(SchedulerError::NoRunnableTask));
    assert_eq!(scheduler.state(task), Ok(TaskState::Exited));
    assert_eq!(scheduler.wake(task), Err(SchedulerError::TaskNotBlocked));
    assert_eq!(scheduler.schedule(), Err(SchedulerError::NoRunnableTask));
    assert_eq!(scheduler.check_invariants(), Ok(()));
}
