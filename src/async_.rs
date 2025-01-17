use crate::Atomic;
use atomig::Atom;
use core::{
    future::Future,
    ops::Deref,
    pin::Pin,
    sync::atomic::Ordering,
    task::{Context, Poll},
};
use futures::stream::{FusedStream, Stream};
use pin_project_lite::pin_project;

/// Generic reference to async atomic.
///
/// Contains `async` methods which returns futures that wait for atomic value change.
///
/// *After one of the futures `poll`ed, all previously `poll`ed futures will not wake.*
/// *This may cause a deadlock, however it is not an UB, so these methods are safe.*
pub trait AsyncAtomic {
    /// Type stored in atomic.
    type Item: Atom;

    /// Get reference to original atomic structure.
    fn as_atomic(&self) -> &Atomic<Self::Item>;

    /// Asynchronously wait for predicate to be `true`.
    fn wait<F: FnMut(Self::Item) -> bool>(&self, pred: F) -> Wait<&Self, F> {
        Wait { inner: self, pred }
    }

    /// Asynchronously wait until `map` returned `Some(x)` and then store `x` in atomic.
    ///
    /// This is an asynchronous version of [`Atomic::fetch_update`].
    fn wait_and_update<F: FnMut(Self::Item) -> Option<Self::Item>>(
        &self,
        map: F,
    ) -> WaitAndUpdate<&Self, F> {
        WaitAndUpdate { inner: self, map }
    }

    /// Convert subscriber into stream that yields when value is changed.
    fn changed(self) -> Changed<Self>
    where
        Self: Sized,
        Self::Item: PartialEq + Clone,
    {
        Changed {
            inner: self,
            prev: None,
        }
    }
}

impl<T: Atom> AsyncAtomic for Atomic<T> {
    type Item = T;
    fn as_atomic(&self) -> &Atomic<Self::Item> {
        self
    }
}

impl<R: Deref<Target: AsyncAtomic>> AsyncAtomic for R {
    type Item = <R::Target as AsyncAtomic>::Item;
    fn as_atomic(&self) -> &Atomic<Self::Item> {
        self.deref().as_atomic()
    }
}

impl<T: Atom + PartialEq> Atomic<T> {}

/// Future to wait for specific value.
pub struct Wait<R: AsyncAtomic, F: FnMut(R::Item) -> bool> {
    pub inner: R,
    pub pred: F,
}

impl<R: AsyncAtomic, F: FnMut(R::Item) -> bool> Unpin for Wait<R, F> {}

impl<R: AsyncAtomic, F: FnMut(R::Item) -> bool> Future for Wait<R, F> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let atomic = self.inner.as_atomic();
        atomic.waker.register(cx.waker());
        let value = atomic.value.load(Ordering::Acquire);
        // TODO: Evaluate predicate on store to avoid spurious wake-ups.
        if (self.pred)(value) {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

pin_project! {
    /// Future to wait and update an atomic value.
    pub struct WaitAndUpdate<R: AsyncAtomic, F: FnMut(R::Item) -> Option<R::Item>> {
        pub inner: R,
        pub map: F,
    }
}

impl<R: AsyncAtomic, F: FnMut(R::Item) -> Option<R::Item>> Future for WaitAndUpdate<R, F> {
    type Output = R::Item;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();
        let atomic = this.inner.as_atomic();
        atomic.waker.register(cx.waker());
        match atomic
            .value
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, &mut this.map)
        {
            Ok(x) => Poll::Ready(x),
            Err(_) => Poll::Pending,
        }
    }
}

/// Stream that yields value when it change.
pub struct Changed<R: AsyncAtomic<Item: PartialEq + Clone>> {
    pub inner: R,
    pub prev: Option<R::Item>,
}

impl<R: AsyncAtomic<Item: PartialEq + Clone>> Deref for Changed<R> {
    type Target = R;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<R: AsyncAtomic<Item: PartialEq + Clone>> Unpin for Changed<R> {}

impl<R: AsyncAtomic<Item: PartialEq + Clone>> Future for Changed<R> {
    type Output = R::Item;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let atomic = self.inner.as_atomic();
        atomic.waker.register(cx.waker());
        let value = atomic.value.load(Ordering::Acquire);
        if self
            .prev
            .replace(value.clone())
            .is_none_or(|prev| prev != value)
        {
            Poll::Ready(value)
        } else {
            Poll::Pending
        }
    }
}

impl<R: AsyncAtomic<Item: PartialEq + Clone>> Stream for Changed<R> {
    type Item = R::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<R::Item>> {
        self.poll(cx).map(Some)
    }
}

impl<R: AsyncAtomic<Item: PartialEq + Clone>> FusedStream for Changed<R> {
    fn is_terminated(&self) -> bool {
        false
    }
}
