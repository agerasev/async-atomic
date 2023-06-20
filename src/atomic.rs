use atomic::Atomic as BasicAtomic;
use core::sync::atomic::Ordering;
use futures::task::AtomicWaker;

/// Atomic value that also contains [`Waker`](`core::task::Waker`) to notify subscriber asynchronously.
#[derive(Default, Debug)]
pub struct Atomic<T: Copy> {
    pub(crate) value: BasicAtomic<T>,
    pub(crate) waker: AtomicWaker,
}

impl<T: Copy> Atomic<T> {
    pub const fn new(value: T) -> Self {
        Self {
            value: BasicAtomic::new(value),
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
        impl Atomic<$T> {
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

        impl Atomic<$T> {
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
