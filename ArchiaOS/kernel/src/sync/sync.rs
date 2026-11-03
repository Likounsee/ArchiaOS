//! Minimal SMP-capable spin mutex for kernel-owned data (P2).
//!
//! This lock provides inter-CPU mutual exclusion only. It does not disable
//! local interrupts: code shared with an interrupt handler must mask local
//! interrupts before acquiring the lock, or use a separately designed
//! IRQ-safe primitive. Never sleep, allocate, or block while holding it.

use core::cell::UnsafeCell;
use core::hint::spin_loop;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};

/// A non-recursive spin mutex. Lock ordering remains the caller's responsibility.
pub struct SpinLock<T: ?Sized> {
    held: AtomicBool,
    value: UnsafeCell<T>,
}

// SAFETY: access to T is serialized by the atomic lock; moving the lock is safe
// whenever T itself can be moved to another thread.
unsafe impl<T: ?Sized + Send> Send for SpinLock<T> {}
// SAFETY: shared access is only exposed through a guard holding the lock.
unsafe impl<T: ?Sized + Send> Sync for SpinLock<T> {}

impl<T> SpinLock<T> {
    pub const fn new(value: T) -> Self {
        Self {
            held: AtomicBool::new(false),
            value: UnsafeCell::new(value),
        }
    }

    pub fn into_inner(self) -> T {
        self.value.into_inner()
    }
}

impl<T: ?Sized> SpinLock<T> {
    /// Acquire the lock, spinning until the current owner releases it.
    ///
    /// Do not call from an interrupt that can preempt a same-CPU lock owner.
    pub fn lock(&self) -> SpinLockGuard<'_, T> {
        loop {
            if self.held.compare_exchange_weak(
                false,
                true,
                Ordering::Acquire,
                Ordering::Relaxed,
            ).is_ok() {
                return SpinLockGuard { lock: self };
            }
            while self.held.load(Ordering::Relaxed) {
                spin_loop();
            }
        }
    }

    /// Attempt acquisition once without waiting.
    pub fn try_lock(&self) -> Option<SpinLockGuard<'_, T>> {
        self.held.compare_exchange(
            false,
            true,
            Ordering::Acquire,
            Ordering::Relaxed,
        ).ok().map(|_| SpinLockGuard { lock: self })
    }

    /// Whether a lock appears held at this instant; diagnostic only.
    pub fn is_locked(&self) -> bool {
        self.held.load(Ordering::Relaxed)
    }
}


/// FIFO ticket spin mutex for short critical sections that need bounded ordering
/// between contenders. It provides inter-CPU mutual exclusion only; it does not
/// disable local interrupts or park the current task.
pub struct TicketLock<T: ?Sized> {
    next_ticket: core::sync::atomic::AtomicUsize,
    serving: core::sync::atomic::AtomicUsize,
    value: UnsafeCell<T>,
}

// SAFETY: the ticket protocol grants exclusive access to T to one guard at a time.
unsafe impl<T: ?Sized + Send> Send for TicketLock<T> {}
// SAFETY: shared access is only exposed while the caller owns the serving ticket.
unsafe impl<T: ?Sized + Send> Sync for TicketLock<T> {}

impl<T> TicketLock<T> {
    pub const fn new(value: T) -> Self {
        Self {
            next_ticket: core::sync::atomic::AtomicUsize::new(0),
            serving: core::sync::atomic::AtomicUsize::new(0),
            value: UnsafeCell::new(value),
        }
    }

    pub fn into_inner(self) -> T {
        self.value.into_inner()
    }
}

impl<T: ?Sized> TicketLock<T> {
    /// Acquire a monotonically ordered ticket and wait until it is served.
    ///
    /// Never sleep, block, or enter an interrupt handler that can preempt the
    /// current owner while holding this lock. Ticket wraparound is safe as long
    /// as fewer than usize::MAX tickets can be outstanding simultaneously.
    pub fn lock(&self) -> TicketLockGuard<'_, T> {
        let ticket = self.next_ticket.fetch_add(1, Ordering::Relaxed);
        while self.serving.load(Ordering::Acquire) != ticket {
            spin_loop();
        }
        TicketLockGuard { lock: self, ticket }
    }

    /// Attempt to acquire the lock only when no owner or queued waiter exists.
    ///
    /// A failed attempt never reserves a ticket and can never jump ahead of an
    /// already queued caller. Use this for opportunistic paths that must not spin.
    pub fn try_lock(&self) -> Option<TicketLockGuard<'_, T>> {
        let serving = self.serving.load(Ordering::Acquire);
        self.next_ticket
            .compare_exchange(
                serving,
                serving.wrapping_add(1),
                Ordering::Acquire,
                Ordering::Relaxed,
            )
            .ok()
            .map(|_| TicketLockGuard { lock: self, ticket: serving })
    }

    /// Whether a ticket is currently outstanding; diagnostic only.
    pub fn is_locked(&self) -> bool {
        self.next_ticket.load(Ordering::Relaxed)
            != self.serving.load(Ordering::Relaxed)
    }
}

/// RAII guard that advances the FIFO queue when the critical section ends.
pub struct TicketLockGuard<'a, T: ?Sized> {
    lock: &'a TicketLock<T>,
    ticket: usize,
}

impl<T: ?Sized> Deref for TicketLockGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // SAFETY: this guard owns the currently served ticket.
        unsafe { &*self.lock.value.get() }
    }
}

impl<T: ?Sized> DerefMut for TicketLockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: this guard owns the currently served ticket.
        unsafe { &mut *self.lock.value.get() }
    }
}

impl<T: ?Sized> Drop for TicketLockGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.serving.store(self.ticket.wrapping_add(1), Ordering::Release);
    }
}

/// Monotonic notification sequence for wait protocols that must not lose a wakeup.
///
/// A waiter snapshots `observe()` before checking its predicate. If the
/// predicate is still false, it calls `wait_for_change(snapshot)`; a notifier
/// increments the sequence with release ordering. A notification arriving
/// between the predicate check and the wait is therefore still observable.
/// This primitive deliberately spins and does not block a scheduler task.
/// Integrating it with task parking requires an atomic park/register protocol.
pub struct EventSequence {
    generation: core::sync::atomic::AtomicU64,
}

impl EventSequence {
    pub const fn new() -> Self {
        Self { generation: core::sync::atomic::AtomicU64::new(0) }
    }

    /// Capture the current generation before checking the protected predicate.
    #[inline]
    pub fn observe(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Publish a state change and notify any waiter observing an older generation.
    #[inline]
    pub fn notify(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::Release).wrapping_add(1)
    }

    /// Return immediately if a notification happened since `observed`.
    #[inline]
    pub fn has_changed(&self, observed: u64) -> bool {
        self.generation.load(Ordering::Acquire) != observed
    }

    /// Busy-wait until a notification changes the generation.
    ///
    /// This is useful only for short low-level waits. It must not be used as a
    /// replacement for scheduler blocking or while holding a lock needed by
    /// the notifier.
    pub fn wait_for_change(&self, observed: u64) -> u64 {
        loop {
            let current = self.generation.load(Ordering::Acquire);
            if current != observed {
                return current;
            }
            spin_loop();
        }
    }
}

impl Default for EventSequence {
    fn default() -> Self { Self::new() }
}

/// IRQ-safe wrapper for contexts that share data with local interrupt handlers.
///
/// `lock_irqsave` is kernel-only: it executes CLI and therefore must never
/// be called by a host process at ring 3. Interrupts are restored only if IF was
/// set on entry. The wrapped lock still provides inter-CPU exclusion.
pub struct IrqSpinLock<T> {
    inner: SpinLock<T>,
}

impl<T> IrqSpinLock<T> {
    pub const fn new(value: T) -> Self {
        Self { inner: SpinLock::new(value) }
    }

    /// Disable local maskable interrupts before acquiring the inter-CPU lock.
    ///
    /// # Safety
    /// This method executes privileged CLI and is only valid in kernel mode on
    /// x86-64 with a valid interrupt state. Do not use from NMI contexts.
    pub unsafe fn lock_irqsave(&self) -> IrqSpinLockGuard<'_, T> {
        let flags: u64;
        // SAFETY: guaranteed by this method's kernel-mode contract.
        unsafe {
            core::arch::asm!(
                "pushfq",
                "pop {flags}",
                "cli",
                flags = out(reg) flags
            );
        }
        let guard = self.inner.lock();
        IrqSpinLockGuard { guard: Some(guard), restore_if: saved_if_was_enabled(flags) }
    }
}

/// Guard that releases the lock before restoring the caller's interrupt state.
pub struct IrqSpinLockGuard<'a, T> {
    guard: Option<SpinLockGuard<'a, T>>,
    restore_if: bool,
}

impl<T> Deref for IrqSpinLockGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T { self.guard.as_ref().expect("live IRQ lock guard").deref() }
}

impl<T> DerefMut for IrqSpinLockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T { self.guard.as_mut().expect("live IRQ lock guard").deref_mut() }
}

impl<T> Drop for IrqSpinLockGuard<'_, T> {
    fn drop(&mut self) {
        // Release inter-CPU ownership while local interrupts are still masked.
        drop(self.guard.take());
        if self.restore_if {
            // SAFETY: the lock was acquired only in kernel mode and IF was set
            // on entry; restore it only after releasing the protected data.
            unsafe { core::arch::asm!("sti", options(nostack)) };
        }
    }
}

/// Pure predicate used to test saved RFLAGS handling without privileged ops.
pub const fn saved_if_was_enabled(rflags: u64) -> bool {
    rflags & (1 << 9) != 0
}

/// RAII exclusive access; dropping the guard releases the lock.
pub struct SpinLockGuard<'a, T: ?Sized> {
    lock: &'a SpinLock<T>,
}

impl<T: ?Sized> Deref for SpinLockGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // SAFETY: this guard owns the lock, excluding all other guards.
        unsafe { &*self.lock.value.get() }
    }
}

impl<T: ?Sized> DerefMut for SpinLockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: this guard owns the lock, excluding all other references
        // created through the public API for the duration of the borrow.
        unsafe { &mut *self.lock.value.get() }
    }
}

impl<T: ?Sized> Drop for SpinLockGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.held.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::{saved_if_was_enabled, EventSequence, SpinLock, TicketLock};
    use core::sync::atomic::Ordering;
    use std::sync::Arc;
    use std::thread;


    #[test]
    fn event_sequence_detects_notification_between_observe_and_wait() {
        let event = EventSequence::new();
        let observed = event.observe();

        // This models the race window: the notification lands after the
        // waiter snapshots the sequence but before it starts waiting.
        assert_eq!(event.notify(), observed + 1);
        assert!(event.has_changed(observed));
        assert_eq!(event.wait_for_change(observed), observed + 1);
    }

    #[test]
    fn event_sequence_counts_every_notification_without_losing_wakeups() {
        const THREADS: usize = 8;
        const NOTIFICATIONS: usize = 2_000;
        let event = Arc::new(EventSequence::new());
        let workers: Vec<_> = (0..THREADS).map(|_| {
            let event = Arc::clone(&event);
            thread::spawn(move || {
                for _ in 0..NOTIFICATIONS {
                    event.notify();
                }
            })
        }).collect();

        for worker in workers {
            worker.join().expect("notifier thread panicked");
        }
        assert_eq!(event.observe(), (THREADS * NOTIFICATIONS) as u64);
    }

    #[test]
    fn event_sequence_waiter_observes_cross_thread_notification() {
        let event = Arc::new(EventSequence::new());
        let observed = event.observe();
        let notifier = Arc::clone(&event);
        let worker = thread::spawn(move || {
            notifier.notify();
        });

        let changed = event.wait_for_change(observed);
        worker.join().expect("notifier thread panicked");
        assert_ne!(changed, observed);
        assert_eq!(changed, event.observe());
    }

    #[test]
    fn event_sequence_default_starts_at_zero_and_notifications_are_monotonic() {
        let event = EventSequence::default();
        assert_eq!(event.observe(), 0);
        assert_eq!(event.notify(), 1);
        assert_eq!(event.notify(), 2);
        assert_eq!(event.observe(), 2);
    }

    #[test]
    fn event_sequence_wraparound_still_changes_the_observed_generation() {
        let event = EventSequence::new();
        event.generation.store(u64::MAX, Ordering::Relaxed);
        let observed = event.observe();
        assert_eq!(observed, u64::MAX);
        assert_eq!(event.notify(), 0);
        assert!(event.has_changed(observed));
        assert_eq!(event.wait_for_change(observed), 0);
    }

    #[test]
    fn event_sequence_notification_is_observable_by_all_waiters() {
        use std::sync::Barrier;

        const WAITERS: usize = 4;
        let event = Arc::new(EventSequence::new());
        let rendezvous = Arc::new(Barrier::new(WAITERS + 1));
        let workers: Vec<_> = (0..WAITERS).map(|_| {
            let event = Arc::clone(&event);
            let rendezvous = Arc::clone(&rendezvous);
            thread::spawn(move || {
                let observed = event.observe();
                rendezvous.wait();
                event.wait_for_change(observed)
            })
        }).collect();

        // Every worker has captured the old generation before notification.
        rendezvous.wait();
        let notified_generation = event.notify();

        for worker in workers {
            assert_eq!(worker.join().expect("waiter thread panicked"), notified_generation);
        }
        assert_eq!(event.observe(), notified_generation);
    }

    #[test]
    fn irqsave_restoration_only_reenables_previously_enabled_interrupts() {
        assert!(saved_if_was_enabled(1 << 9));
        assert!(saved_if_was_enabled((1 << 9) | 2));
        assert!(!saved_if_was_enabled(0));
        assert!(!saved_if_was_enabled(2));
        assert!(!saved_if_was_enabled(u64::MAX ^ (1 << 9)));
    }

    #[test]
    fn try_lock_fails_while_guard_is_held_and_recovers_after_drop() {
        let lock = SpinLock::new(7);
        assert!(!lock.is_locked());
        let guard = lock.lock();
        assert!(lock.is_locked());
        assert!(lock.try_lock().is_none());
        assert_eq!(*guard, 7);
        drop(guard);
        assert!(!lock.is_locked());
        assert_eq!(*lock.try_lock().expect("lock should be available"), 7);
    }

    #[test]
    fn guard_mutation_is_visible_after_release() {
        let lock = SpinLock::new(1usize);
        *lock.lock() += 41;
        assert_eq!(*lock.lock(), 42);
    }

    #[test]
    fn ticket_lock_guard_releases_and_reports_idle_state() {
        let lock = TicketLock::new(7usize);
        assert!(!lock.is_locked());
        {
            let mut guard = lock.lock();
            assert!(lock.is_locked());
            *guard += 5;
            assert_eq!(*guard, 12);
        }
        assert!(!lock.is_locked());
        assert_eq!(*lock.lock(), 12);
    }

    #[test]
    fn ticket_lock_preserves_fifo_order_for_reserved_tickets() {
        use core::hint::spin_loop;
        use std::sync::Mutex;

        let lock = Arc::new(TicketLock::new(()));
        let order = Arc::new(Mutex::new(Vec::new()));
        let first_guard = lock.lock(); // Keep ticket 0 at the head of the queue.

        let first_lock = Arc::clone(&lock);
        let first_order = Arc::clone(&order);
        let first = thread::spawn(move || {
            let _guard = first_lock.lock(); // Reserves ticket 1.
            first_order.lock().unwrap().push(1);
        });
        // Do not launch the second contender until ticket 1 is definitely reserved.
        let mut spins = 0usize;
        while lock.next_ticket.load(Ordering::Acquire) != 2 {
            spin_loop();
            spins += 1;
            if spins % 1024 == 0 { thread::yield_now(); }
            assert!(spins < 10_000_000, "first contender did not reserve a ticket");
        }

        let second_lock = Arc::clone(&lock);
        let second_order = Arc::clone(&order);
        let second = thread::spawn(move || {
            let _guard = second_lock.lock(); // Reserves ticket 2.
            second_order.lock().unwrap().push(2);
        });
        spins = 0;
        while lock.next_ticket.load(Ordering::Acquire) != 3 {
            spin_loop();
            spins += 1;
            if spins % 1024 == 0 { thread::yield_now(); }
            assert!(spins < 10_000_000, "second contender did not reserve a ticket");
        }

        drop(first_guard);
        first.join().expect("first ticket-lock worker panicked");
        second.join().expect("second ticket-lock worker panicked");
        assert_eq!(*order.lock().unwrap(), vec![1, 2]);
        assert!(!lock.is_locked());
    }

    #[test]
    fn ticket_lock_failed_try_lock_never_reserves_or_skips_a_ticket() {
        let lock = TicketLock::new(0usize);
        let owner = lock.lock();
        assert_eq!(lock.next_ticket.load(Ordering::Acquire), 1);
        assert_eq!(lock.serving.load(Ordering::Acquire), 0);

        // Repeated failures must be side-effect free: a try-lock caller is
        // opportunistic and must not leave a phantom ticket in the FIFO queue.
        for _ in 0..10_000 {
            assert!(lock.try_lock().is_none());
        }
        assert_eq!(lock.next_ticket.load(Ordering::Acquire), 1);
        assert_eq!(lock.serving.load(Ordering::Acquire), 0);
        drop(owner);

        let mut guard = lock.try_lock().expect("released lock should be available");
        *guard = 9;
        drop(guard);
        assert_eq!(lock.next_ticket.load(Ordering::Acquire), 2);
        assert_eq!(lock.serving.load(Ordering::Acquire), 2);
        assert!(!lock.is_locked());
    }

    #[test]
    fn ticket_lock_try_lock_never_bypasses_a_queued_waiter() {
        use core::hint::spin_loop;

        let lock = Arc::new(TicketLock::new(0usize));
        let owner = lock.lock();
        let worker_lock = Arc::clone(&lock);
        let worker = thread::spawn(move || {
            let mut guard = worker_lock.lock();
            *guard = 2;
        });

        let mut spins = 0usize;
        while lock.next_ticket.load(Ordering::Acquire) != 2 {
            spin_loop();
            spins += 1;
            if spins % 1024 == 0 { thread::yield_now(); }
            assert!(spins < 10_000_000, "worker did not reserve a ticket");
        }
        assert!(lock.try_lock().is_none(), "try_lock must not jump the queue");
        drop(owner);
        worker.join().expect("queued worker panicked");
        {
            let mut guard = lock.try_lock().expect("idle lock should be acquired immediately");
            assert_eq!(*guard, 2);
            *guard = 3;
        }
        assert!(!lock.is_locked());
        assert_eq!(*lock.lock(), 3);
    }

    #[test]
    fn ticket_lock_counter_wraparound_preserves_exclusive_progress() {
        let lock = TicketLock::new(0usize);
        lock.next_ticket.store(usize::MAX - 1, Ordering::Relaxed);
        lock.serving.store(usize::MAX - 1, Ordering::Relaxed);

        *lock.lock() += 1; // usize::MAX - 1 -> usize::MAX
        assert_eq!(lock.serving.load(Ordering::Acquire), usize::MAX);
        *lock.lock() += 1; // usize::MAX -> 0
        assert_eq!(lock.serving.load(Ordering::Acquire), 0);
        *lock.lock() += 1; // 0 -> 1
        assert_eq!(lock.serving.load(Ordering::Acquire), 1);

        assert_eq!(*lock.lock(), 3);
        assert!(!lock.is_locked());
        assert_eq!(lock.next_ticket.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn ticket_lock_serializes_concurrent_updates_under_contention() {
        const THREADS: usize = 8;
        const ITERATIONS: usize = 10_000;
        let shared = Arc::new(TicketLock::new(0usize));
        let workers: Vec<_> = (0..THREADS).map(|_| {
            let shared = Arc::clone(&shared);
            thread::spawn(move || {
                for _ in 0..ITERATIONS {
                    *shared.lock() += 1;
                }
            })
        }).collect();
        for worker in workers {
            worker.join().expect("ticket-lock worker thread panicked");
        }
        assert_eq!(*shared.lock(), THREADS * ITERATIONS);
        assert!(!shared.is_locked());
    }

    #[test]
    fn concurrent_updates_are_serialized() {
        const THREADS: usize = 8;
        const ITERATIONS: usize = 10_000;
        let shared = Arc::new(SpinLock::new(0usize));
        let workers: Vec<_> = (0..THREADS).map(|_| {
            let shared = Arc::clone(&shared);
            thread::spawn(move || {
                for _ in 0..ITERATIONS {
                    *shared.lock() += 1;
                }
            })
        }).collect();
        for worker in workers {
            worker.join().expect("worker thread panicked");
        }
        assert_eq!(*shared.lock(), THREADS * ITERATIONS);
    }
}
