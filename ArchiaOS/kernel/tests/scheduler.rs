//! Host-runnable regression tests for the scheduler state model.
//!
//! This includes the production module directly so the fixed-capacity scheduler
//! can be tested on the host without trying to link the freestanding kernel.
#[path = "../src/scheduler.rs"]
mod scheduler;

#[path = "../src/context.rs"]
mod context;

use scheduler::{Scheduler, SchedulerError, TaskState};

#[test]
fn x86_64_context_switch_enters_task_stack_and_returns() {
    assert_eq!(context::self_test(), Ok(()));
}

#[test]
fn three_runtime_tasks_block_wake_yield_and_exit_on_distinct_stacks() {
    assert_eq!(scheduler::runtime_self_test(), Ok(()));
}

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
fn blocking_last_runnable_task_returns_waiting_idle_decision() {
    let mut scheduler = Scheduler::new();
    let task = scheduler.create_task().expect("task slot");

    assert_eq!(scheduler.schedule(), Ok(task));
    assert_eq!(
        scheduler.block_current_or_idle(),
        Ok(scheduler::DispatchDecision::Idle(scheduler::IdleReason::WaitingForWakeup))
    );
    assert_eq!(scheduler.current(), None);
    assert_eq!(scheduler.state(task), Ok(TaskState::Blocked));
    assert_eq!(scheduler.check_invariants(), Ok(()));

    scheduler.wake(task).expect("blocked task should wake");
    assert_eq!(scheduler.dispatch_or_idle(), Ok(scheduler::DispatchDecision::Run(task)));
    assert_eq!(scheduler.current(), Some(task));
}

#[test]
fn exiting_last_task_returns_all_tasks_exited_idle_decision() {
    let mut scheduler = Scheduler::new();
    let task = scheduler.create_task().expect("task slot");

    assert_eq!(scheduler.schedule(), Ok(task));
    assert_eq!(
        scheduler.exit_current_or_idle(),
        Ok(scheduler::DispatchDecision::Idle(scheduler::IdleReason::AllTasksExited))
    );
    assert_eq!(scheduler.current(), None);
    assert_eq!(scheduler.state(task), Ok(TaskState::Exited));
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

#[test]
fn round_robin_wraps_across_capacity_and_never_runs_two_tasks() {
    let mut scheduler = Scheduler::new();
    let mut ids = Vec::new();
    for _ in 0..scheduler::MAX_TASKS {
        ids.push(scheduler.create_task().expect("available task slot"));
    }

    let first = scheduler.schedule().expect("first dispatch");
    assert_eq!(first, ids[0]);
    for expected in ids.iter().skip(1).chain(ids.iter().take(1)) {
        assert_eq!(scheduler.yield_current(), Ok(*expected));
        assert_eq!(scheduler.check_invariants(), Ok(()));
        assert_eq!(scheduler.current(), Some(*expected));
    }
}

#[test]
fn invalid_wake_does_not_mutate_task_state() {
    let mut scheduler = Scheduler::new();
    let runnable = scheduler.create_task().expect("task slot");
    assert_eq!(scheduler.wake(runnable), Err(SchedulerError::TaskNotBlocked));
    assert_eq!(scheduler.state(runnable), Ok(TaskState::Runnable));

    assert_eq!(scheduler.schedule(), Ok(runnable));
    assert_eq!(scheduler.wake(runnable), Err(SchedulerError::TaskNotBlocked));
    assert_eq!(scheduler.state(runnable), Ok(TaskState::Running));
    assert_eq!(scheduler.check_invariants(), Ok(()));
}

#[test]
fn blocked_task_requires_one_wake_and_then_resumes_by_round_robin() {
    let mut scheduler = Scheduler::new();
    let blocked = scheduler.create_task().expect("first task slot");
    let other = scheduler.create_task().expect("second task slot");

    assert_eq!(scheduler.schedule(), Ok(blocked));
    assert_eq!(scheduler.block_current(), Ok(other));
    assert_eq!(scheduler.state(blocked), Ok(TaskState::Blocked));
    assert_eq!(scheduler.current(), Some(other));

    assert_eq!(scheduler.wake(blocked), Ok(()));
    assert_eq!(scheduler.wake(blocked), Err(SchedulerError::TaskNotBlocked));
    assert_eq!(scheduler.state(blocked), Ok(TaskState::Runnable));
    assert_eq!(scheduler.yield_current(), Ok(blocked));
    assert_eq!(scheduler.current(), Some(blocked));
    assert_eq!(scheduler.check_invariants(), Ok(()));
}

#[test]
fn exited_slots_are_not_reused_for_new_task_ids() {
    let mut scheduler = Scheduler::new();
    let exited = scheduler.create_task().expect("first task slot");
    assert_eq!(scheduler.schedule(), Ok(exited));
    assert_eq!(scheduler.exit_current(), Err(SchedulerError::NoRunnableTask));
    assert_eq!(scheduler.state(exited), Ok(TaskState::Exited));

    let later = scheduler.create_task().expect("next empty slot");
    assert_ne!(later, exited);
    assert_eq!(scheduler.state(exited), Ok(TaskState::Exited));
    assert_eq!(scheduler.state(later), Ok(TaskState::Runnable));
    assert_eq!(scheduler.check_invariants(), Ok(()));
}

#[test]
fn yielding_with_one_task_keeps_the_same_task_running() {
    let mut scheduler = Scheduler::new();
    let only = scheduler.create_task().expect("task slot");

    assert_eq!(scheduler.schedule(), Ok(only));
    for _ in 0..(scheduler::MAX_TASKS * 3) {
        assert_eq!(scheduler.yield_current(), Ok(only));
        assert_eq!(scheduler.current(), Some(only));
        assert_eq!(scheduler.state(only), Ok(TaskState::Running));
        assert_eq!(scheduler.check_invariants(), Ok(()));
    }
}

#[test]
fn idle_dispatch_is_stable_until_a_blocked_task_is_woken() {
    let mut scheduler = Scheduler::new();
    let first = scheduler.create_task().expect("first task slot");
    let second = scheduler.create_task().expect("second task slot");

    assert_eq!(scheduler.schedule(), Ok(first));
    assert_eq!(scheduler.block_current(), Ok(second));
    assert_eq!(scheduler.block_current(), Err(SchedulerError::NoRunnableTask));
    assert_eq!(scheduler.dispatch_or_idle(), Ok(scheduler::DispatchDecision::Idle(scheduler::IdleReason::WaitingForWakeup)));
    assert_eq!(scheduler.dispatch_or_idle(), Ok(scheduler::DispatchDecision::Idle(scheduler::IdleReason::WaitingForWakeup)));
    assert_eq!(scheduler.current(), None);
    assert_eq!(scheduler.check_invariants(), Ok(()));

    scheduler.wake(first).expect("first task wakes");
    assert_eq!(scheduler.dispatch_or_idle(), Ok(scheduler::DispatchDecision::Run(first)));
    assert_eq!(scheduler.current(), Some(first));
    assert_eq!(scheduler.check_invariants(), Ok(()));
}
