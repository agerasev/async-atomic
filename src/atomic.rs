use atomig::{
    impls::{PrimitiveAtom, PrimitiveAtomInteger, PrimitiveAtomLogic},
    Atom, AtomInteger, AtomLogic, Atomic as BasicAtomic,
};
use core::sync::atomic::Ordering;
use futures::task::AtomicWaker;

/// Atomic value that also contains [`Waker`](`core::task::Waker`) to notify subscriber asynchronously.
///
/// *There is only a single waker, so there should be only single subscription at a time.*
/// *Otherwise older subscriptions will not receive updates anymore.*
#[derive(Default, Debug)]
pub struct AsyncAtomic<T: Atom> {
    pub(crate) value: BasicAtomic<T>,
    pub(crate) waker: AtomicWaker,
}

impl<T: Atom> AsyncAtomic<T> {
    pub fn new(value: T) -> Self {
        Self {
            value: BasicAtomic::new(value),
            waker: AtomicWaker::new(),
        }
    }

    pub const fn from_impl(repr: <T::Repr as PrimitiveAtom>::Impl) -> Self {
        Self {
            value: BasicAtomic::from_impl(repr),
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
            .inspect(|_| self.waker.wake())
    }

    pub fn fetch_update<F: FnMut(T) -> Option<T>>(&self, f: F) -> Result<T, T> {
        self.value
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, f)
            .inspect(|_| self.waker.wake())
    }
}

impl<T: AtomLogic> AsyncAtomic<T>
where
    T::Repr: PrimitiveAtomLogic,
{
    pub fn fetch_and(&self, val: T) -> T {
        let old = self.value.fetch_and(val, Ordering::AcqRel);
        self.waker.wake();
        old
    }
    pub fn fetch_or(&self, val: T) -> T {
        let old = self.value.fetch_or(val, Ordering::AcqRel);
        self.waker.wake();
        old
    }
    pub fn fetch_xor(&self, val: T) -> T {
        let old = self.value.fetch_xor(val, Ordering::AcqRel);
        self.waker.wake();
        old
    }
}

impl<T: AtomInteger> AsyncAtomic<T>
where
    T::Repr: PrimitiveAtomInteger,
{
    pub fn fetch_add(&self, val: T) -> T {
        let old = self.value.fetch_add(val, Ordering::AcqRel);
        self.waker.wake();
        old
    }
    pub fn fetch_sub(&self, val: T) -> T {
        let old = self.value.fetch_sub(val, Ordering::AcqRel);
        self.waker.wake();
        old
    }
    pub fn fetch_max(&self, val: T) -> T {
        let old = self.value.fetch_max(val, Ordering::AcqRel);
        self.waker.wake();
        old
    }
    pub fn fetch_min(&self, val: T) -> T {
        let old = self.value.fetch_min(val, Ordering::AcqRel);
        self.waker.wake();
        old
    }
}

impl<T: Atom> AsRef<AsyncAtomic<T>> for AsyncAtomic<T> {
    fn as_ref(&self) -> &AsyncAtomic<T> {
        self
    }
}
