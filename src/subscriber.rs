use crate::Atomic;
#[cfg(feature = "alloc")]
use alloc::sync::Arc;
use core::{
    future::Future,
    ops::Deref,
    pin::Pin,
    sync::atomic::Ordering,
    task::{Context, Poll},
};

impl<T: Copy> Atomic<T> {
    /// Wrap `self` in [`Arc`] and then create subscriber from it.
    #[cfg(feature = "alloc")]
    pub fn subscribe(self) -> Subscriber<T> {
        Arc::new(self).subscribe_arc()
    }
    /// Create subscriber from [`Arc`].
    #[cfg(feature = "alloc")]
    pub fn subscribe_arc(self: Arc<Self>) -> Subscriber<T> {
        Subscriber::new(self)
    }
    /// Create subscriber from reference.
    pub fn subscribe_ref(&self) -> RefSubscriber<T> {
        RefSubscriber::new(self)
    }
}

pub struct GenericSubscriber<T: Copy, D: Deref<Target = Atomic<T>>> {
    inner: D,
}

/// Subscriber of the atomic variable.
///
/// References to an underlying atomic could be obtained using [`Deref`].
#[cfg(feature = "alloc")]
pub type Subscriber<T> = GenericSubscriber<T, Arc<Atomic<T>>>;
pub type RefSubscriber<'a, T> = GenericSubscriber<T, &'a Atomic<T>>;

impl<T: Copy, D: Deref<Target = Atomic<T>>> GenericSubscriber<T, D> {
    /// Create subscriber for atomic.
    ///
    /// *If there are multiple subscribers for single atomic then only one of them is notified.*
    pub fn new(atomic_ref: D) -> Self {
        Self { inner: atomic_ref }
    }

    /// Asynchronously wait for predicate to be `true`.
    pub fn wait<F: Fn(T) -> bool>(&mut self, pred: F) -> Wait<'_, T, F> {
        Wait {
            owner: &self.inner,
            pred,
        }
    }

    /// Asynchronously wait until `map` returned `Some(x)` and then store `x` in atomic.
    ///
    /// This is an asynchronous version of [`Atomic::fetch_update`].
    pub fn wait_and_update<F: Fn(T) -> Option<T>>(&mut self, map: F) -> WaitAndUpdate<'_, T, F> {
        WaitAndUpdate {
            owner: &self.inner,
            map,
        }
    }
}

impl<T: Copy, D: Deref<Target = Atomic<T>>> Deref for GenericSubscriber<T, D> {
    type Target = D;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// Future to wait for specific value.
///
/// # Todo
///
/// Evaluate predicate on store to avoid spurious wakeups.
pub struct Wait<'a, T: Copy, F: Fn(T) -> bool> {
    owner: &'a Atomic<T>,
    pred: F,
}
impl<'a, T: Copy, F: Fn(T) -> bool> Unpin for Wait<'a, T, F> {}
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
    owner: &'a Atomic<T>,
    map: F,
}
impl<'a, T: Copy, F: Fn(T) -> Option<T>> Unpin for WaitAndUpdate<'a, T, F> {}
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
