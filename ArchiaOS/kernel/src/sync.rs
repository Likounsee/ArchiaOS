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
    use super::{saved_if_was_enabled, EventSequence, SpinLock};
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
