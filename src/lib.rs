//! Atomics which can be subscribed to and asynchronously notify when updated.
//!
//! The main structure is [`AsyncAtomic`] that behaves like stdlib's atomics,
//! but don't take an explicit [`Ordering`](`core::sync::atomic::Ordering`) for simplicity.
//!
//! An [`AtomicSubscriber`] can be splitted off from [`AsyncAtomic`] to asynchronously wait for changes.
//! It is only one subscriber allowed for each atomic.

#![no_std]

#[cfg(feature = "std")]
extern crate std;

#[cfg(test)]
mod tests;

use atomic::Atomic;
use core::{
    future::Future,
    ops::Deref,
    pin::Pin,
    sync::atomic::Ordering,
    task::{Context, Poll},
};
use futures::task::AtomicWaker;
#[cfg(feature = "std")]
use std::sync::Arc;

/// Atomic value that also contains [`Waker`](`core::task::Waker`) to notify subscriber asynchronously.
#[derive(Default, Debug)]
pub struct AsyncAtomic<T: Copy> {
    value: Atomic<T>,
    waker: AtomicWaker,
}

impl<T: Copy> AsyncAtomic<T> {
    pub fn new(value: T) -> Self {
        Self {
            value: Atomic::new(value),
            waker: AtomicWaker::new(),
        }
    }

    pub fn load(&self) -> T {
        self.value.load(Ordering::Acquire)
    }
    pub fn store(&self, val: T) {
        self.value.store(val, Ordering::Release);
        self.waker.wake();
    }
    pub fn swap(&self, val: T) -> T {
        let old = self.value.swap(val, Ordering::AcqRel);
        self.waker.wake();
        old
    }
    pub fn compare_exchange(&self, current: T, new: T) -> Result<T, T> {
        self.value
            .compare_exchange(current, new, Ordering::AcqRel, Ordering::Acquire)
            .map(|x| {
                self.waker.wake();
                x
            })
    }
    pub fn fetch_update<F: FnMut(T) -> Option<T>>(&self, f: F) -> Result<T, T> {
        self.value
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, f)
            .map(|x| {
                self.waker.wake();
                x
            })
    }
}

macro_rules! impl_atomic_bitwise {
    ($T:ty) => {
        impl AsyncAtomic<$T> {
            pub fn fetch_and(&self, val: $T) -> $T {
                let old = self.value.fetch_and(val, Ordering::AcqRel);
                self.waker.wake();
                old
            }
            pub fn fetch_or(&self, val: $T) -> $T {
                let old = self.value.fetch_or(val, Ordering::AcqRel);
                self.waker.wake();
                old
            }
            pub fn fetch_xor(&self, val: $T) -> $T {
                let old = self.value.fetch_xor(val, Ordering::AcqRel);
                self.waker.wake();
                old
            }
        }
    };
}

macro_rules! impl_atomic_num {
    ($T:ty) => {
        impl_atomic_bitwise!($T);

        impl AsyncAtomic<$T> {
            pub fn fetch_add(&self, val: $T) -> $T {
                let old = self.value.fetch_add(val, Ordering::AcqRel);
                self.waker.wake();
                old
            }
            pub fn fetch_sub(&self, val: $T) -> $T {
                let old = self.value.fetch_sub(val, Ordering::AcqRel);
                self.waker.wake();
                old
            }
            pub fn fetch_max(&self, val: $T) -> $T {
                let old = self.value.fetch_max(val, Ordering::AcqRel);
                self.waker.wake();
                old
            }
            pub fn fetch_min(&self, val: $T) -> $T {
                let old = self.value.fetch_min(val, Ordering::AcqRel);
                self.waker.wake();
                old
            }
        }
    };
}

impl_atomic_bitwise!(bool);

impl_atomic_num!(u8);
impl_atomic_num!(u16);
impl_atomic_num!(u32);
impl_atomic_num!(u64);
impl_atomic_num!(usize);

impl_atomic_num!(i8);
impl_atomic_num!(i16);
impl_atomic_num!(i32);
impl_atomic_num!(i64);
impl_atomic_num!(isize);

impl<T: Copy> AsyncAtomic<T> {
    #[cfg(feature = "std")]
    /// Split subscriber off using [`Arc`].
    pub fn split(self) -> (Arc<Self>, AtomicSubscriber<T, Arc<Self>>) {
        let arc = Arc::new(self);
        (arc.clone(), unsafe { AtomicSubscriber::new(arc) })
    }
    /// Split subscriber off using reference.
    pub fn split_ref(&mut self) -> (&Self, AtomicSubscriber<T, &Self>) {
        (self, unsafe { AtomicSubscriber::new(self) })
    }
}

/// Subscriber of the atomic variable.
pub struct AtomicSubscriber<T: Copy, D: Deref<Target = AsyncAtomic<T>>> {
    owner: D,
}

impl<T: Copy, D: Deref<Target = AsyncAtomic<T>>> AtomicSubscriber<T, D> {
    /// # Safety
    ///
    /// Only one subscriber allowed for an atomic value.
    pub unsafe fn new(atomic_ref: D) -> Self {
        Self { owner: atomic_ref }
    }

    /// Asynchronously wait for predicate to be `true`.
    pub fn wait<F: Fn(T) -> bool>(&self, pred: F) -> Wait<'_, T, F> {
        Wait {
            owner: &self.owner,
            pred,
        }
    }

    /// Asynchronously wait until `map` returned `Some(x)` and then store `x` in atomic.
    ///
    /// This is an asynchronous version of [`AsyncAtomic::fetch_update`].
    pub fn wait_and_update<F: Fn(T) -> Option<T>>(&self, map: F) -> WaitAndUpdate<'_, T, F> {
        WaitAndUpdate {
            owner: &self.owner,
            map,
        }
    }
}

impl<T: Copy, D: Deref<Target = AsyncAtomic<T>>> Deref for AtomicSubscriber<T, D> {
    type Target = D;
    fn deref(&self) -> &Self::Target {
        &self.owner
    }
}

/// Future to wait for atomic value change.
///
/// # Todo
///
/// Evaluate predicate on store to avoid spurious wakeups.
pub struct Wait<'a, T: Copy, F: Fn(T) -> bool> {
    owner: &'a AsyncAtomic<T>,
    pred: F,
}

impl<'a, T: Copy, F: Fn(T) -> bool> Future for Wait<'a, T, F> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.owner.waker.register(cx.waker());
        let value = self.owner.value.load(Ordering::Acquire);
        if (self.pred)(value) {
            Poll::Ready(value)
        } else {
            Poll::Pending
        }
    }
}

/// Future to wait and update an atomic value.
pub struct WaitAndUpdate<'a, T: Copy, F: Fn(T) -> Option<T>> {
    owner: &'a AsyncAtomic<T>,
    map: F,
}

impl<'a, T: Copy, F: Fn(T) -> Option<T>> Future for WaitAndUpdate<'a, T, F> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.owner.waker.register(cx.waker());
        match self
            .owner
            .value
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, &self.map)
        {
            Ok(x) => Poll::Ready(x),
            Err(_) => Poll::Pending,
        }
    }
}
