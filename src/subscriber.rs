use crate::Atomic;
#[cfg(feature = "alloc")]
use alloc::sync::Arc;
use core::{
    future::Future,
    marker::PhantomData,
    ops::Deref,
    pin::Pin,
    sync::atomic::Ordering,
    task::{Context, Poll},
};
use futures::stream::{FusedStream, Stream};

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

pub struct GenericSubscriber<T: Copy, D: AsRef<R>, R: AsRef<Atomic<T>> = Atomic<T>> {
    inner: D,
    _ghost: PhantomData<(T, R)>,
}

/// Subscriber of the atomic variable.
///
/// References to an underlying atomic could be obtained using [`Deref`].
#[cfg(feature = "alloc")]
pub type Subscriber<T> = GenericSubscriber<T, Arc<Atomic<T>>>;
pub type RefSubscriber<'a, T> = GenericSubscriber<T, &'a Atomic<T>>;

impl<T: Copy, D: AsRef<R>, R: AsRef<Atomic<T>>> GenericSubscriber<T, D, R> {
    /// Create subscriber for atomic.
    ///
    /// *If there are multiple subscribers for single atomic then only one of them is notified.*
    pub fn new(atomic_ref: D) -> Self {
        Self {
            inner: atomic_ref,
            _ghost: PhantomData,
        }
    }

    /// Asynchronously wait for predicate to be `true`.
    pub fn wait<F: FnMut(T) -> bool>(&mut self, pred: F) -> Wait<'_, T, F> {
        Wait {
            owner: self.inner.as_ref().as_ref(),
            pred,
        }
    }

    /// Asynchronously wait until `map` returned `Some(x)` and then store `x` in atomic.
    ///
    /// This is an asynchronous version of [`Atomic::fetch_update`].
    pub fn wait_and_update<F: FnMut(T) -> Option<T>>(&mut self, map: F) -> WaitAndUpdate<'_, T, F> {
        WaitAndUpdate {
            owner: self.inner.as_ref().as_ref(),
            map,
        }
    }
}

impl<T: Copy + PartialEq, D: AsRef<R>, R: AsRef<Atomic<T>>> GenericSubscriber<T, D, R> {
    /// Convert subscriber into stream that yields when value is changed.
    pub fn into_stream(self) -> Changed<T, D, R> {
        Changed {
            inner: self.inner,
            prev: None,
            _ghost: PhantomData,
        }
    }
}

impl<T: Copy, D: AsRef<R>, R: AsRef<Atomic<T>>> Deref for GenericSubscriber<T, D, R> {
    type Target = D;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl<T: Copy, D: AsRef<R>, R: AsRef<Atomic<T>>> AsRef<D> for GenericSubscriber<T, D, R> {
    fn as_ref(&self) -> &D {
        &self.inner
    }
}

/// Future to wait for specific value.
///
/// # Todo
///
/// Evaluate predicate on store to avoid spurious wake-ups.
pub struct Wait<'a, T: Copy, F: FnMut(T) -> bool> {
    owner: &'a Atomic<T>,
    pred: F,
}
impl<'a, T: Copy, F: FnMut(T) -> bool> Unpin for Wait<'a, T, F> {}
impl<'a, T: Copy, F: FnMut(T) -> bool> Future for Wait<'a, T, F> {
    type Output = T;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
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
pub struct WaitAndUpdate<'a, T: Copy, F: FnMut(T) -> Option<T>> {
    owner: &'a Atomic<T>,
    map: F,
}
impl<'a, T: Copy, F: FnMut(T) -> Option<T>> Unpin for WaitAndUpdate<'a, T, F> {}
impl<'a, T: Copy, F: FnMut(T) -> Option<T>> Future for WaitAndUpdate<'a, T, F> {
    type Output = T;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.owner.waker.register(cx.waker());
        match self
            .owner
            .value
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, &mut self.map)
        {
            Ok(x) => Poll::Ready(x),
            Err(_) => Poll::Pending,
        }
    }
}

/// Stream that yields value when it change.
pub struct Changed<T: Copy + PartialEq, D: AsRef<R>, R: AsRef<Atomic<T>> = Atomic<T>> {
    inner: D,
    prev: Option<T>,
    _ghost: PhantomData<R>,
}
impl<T: Copy + PartialEq, D: AsRef<R>, R: AsRef<Atomic<T>>> Unpin for Changed<T, D, R> {}
impl<T: Copy + PartialEq, D: AsRef<R>, R: AsRef<Atomic<T>>> Stream for Changed<T, D, R> {
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T>> {
        self.inner.as_ref().as_ref().waker.register(cx.waker());
        let value = self.inner.as_ref().as_ref().value.load(Ordering::Acquire);
        if self.prev.replace(value) != Some(value) {
            Poll::Ready(Some(value))
        } else {
            Poll::Pending
        }
    }
}
impl<T: Copy + PartialEq, D: AsRef<R>, R: AsRef<Atomic<T>>> FusedStream for Changed<T, D, R> {
    fn is_terminated(&self) -> bool {
        false
    }
}
