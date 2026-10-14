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
    use super::SpinLock;
    use std::sync::Arc;
    use std::thread;

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
