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
use std::sync::Arc;

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

    pub fn load(&self, order: Ordering) -> T {
        self.value.load(order)
    }
    pub fn store(&self, val: T, order: Ordering) {
        self.value.store(val, order);
        self.waker.wake();
    }
    pub fn swap(&self, val: T, order: Ordering) -> T {
        let old = self.value.swap(val, order);
        self.waker.wake();
        old
    }
    pub fn compare_exchange(
        &self,
        current: T,
        new: T,
        success: Ordering,
        failure: Ordering,
    ) -> Result<T, T> {
        self.value
            .compare_exchange(current, new, success, failure)
            .map(|x| {
                self.waker.wake();
                x
            })
    }
    pub fn compare_exchange_weak(
        &self,
        current: T,
        new: T,
        success: Ordering,
        failure: Ordering,
    ) -> Result<T, T> {
        self.value
            .compare_exchange_weak(current, new, success, failure)
            .map(|x| {
                self.waker.wake();
                x
            })
    }
    pub fn fetch_update<F: FnMut(T) -> Option<T>>(
        &self,
        set_order: Ordering,
        fetch_order: Ordering,
        f: F,
    ) -> Result<T, T> {
        self.value.fetch_update(set_order, fetch_order, f).map(|x| {
            self.waker.wake();
            x
        })
    }
}

macro_rules! impl_atomic_num {
    ($T:ty) => {
        impl AsyncAtomic<$T> {
            pub fn fetch_add(&self, val: $T, order: Ordering) -> $T {
                let old = self.value.fetch_add(val, order);
                self.waker.wake();
                old
            }
            pub fn fetch_sub(&self, val: $T, order: Ordering) -> $T {
                let old = self.value.fetch_sub(val, order);
                self.waker.wake();
                old
            }
        }
    };
}

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
    pub fn split(self) -> (Arc<Self>, AtomicSubscriber<T, Arc<Self>>) {
        let arc = Arc::new(self);
        (arc.clone(), unsafe { AtomicSubscriber::new(arc) })
    }
    pub fn split_ref(&mut self) -> (&Self, AtomicSubscriber<T, &Self>) {
        (self, unsafe { AtomicSubscriber::new(self) })
    }
}

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
    pub fn wait<F: Fn(T) -> bool>(&self, order: Ordering, pred: F) -> Wait<'_, T, F> {
        Wait {
            owner: &self.owner,
            order,
            pred,
        }
    }

    /// Asynchronously wait until `f` returned `Some(x)` and then store `x` in atomic.
    ///
    /// This is an asynchronous version of [`Atomic::fetch_update`].
    pub fn wait_and_update<F: Fn(T) -> Option<T>>(
        &self,
        set_order: Ordering,
        fetch_order: Ordering,
        f: F,
    ) -> WaitAndUpdate<'_, T, F> {
        WaitAndUpdate {
            owner: &self.owner,
            set_order,
            fetch_order,
            f,
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
    order: Ordering,
    pred: F,
}
impl<'a, T: Copy, F: Fn(T) -> bool> Unpin for Wait<'a, T, F> {}
impl<'a, T: Copy, F: Fn(T) -> bool> Future for Wait<'a, T, F> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.owner.waker.register(cx.waker());
        let value = self.owner.value.load(self.order);
        if (self.pred)(value) {
            Poll::Ready(value)
        } else {
            Poll::Pending
        }
    }
}

/// Future to wait and update atomic value based on a preicate.
pub struct WaitAndUpdate<'a, T: Copy, F: Fn(T) -> Option<T>> {
    owner: &'a AsyncAtomic<T>,
    f: F,
    set_order: Ordering,
    fetch_order: Ordering,
}
impl<'a, T: Copy, F: Fn(T) -> Option<T>> Unpin for WaitAndUpdate<'a, T, F> {}
impl<'a, T: Copy, F: Fn(T) -> Option<T>> Future for WaitAndUpdate<'a, T, F> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.owner.waker.register(cx.waker());
        match self
            .owner
            .value
            .fetch_update(self.set_order, self.fetch_order, &self.f)
        {
            Ok(x) => Poll::Ready(x),
            Err(_) => Poll::Pending,
        }
    }
}
