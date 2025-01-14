//! Atomics which can be subscribed to and asynchronously notify when updated.
//!
//! The main structure is [`Atomic`] that behaves like stdlib's atomics,
//! but don't take an explicit [`Ordering`](`core::sync::atomic::Ordering`) for simplicity.
//!
//! An [`Atomic`] can asynchronously wait for changes, but only one waiter will be notified at a time.

#![no_std]

mod async_;
mod atomic;

pub use atomig::Atom;

pub use async_::*;
pub use atomic::*;

pub mod prelude {
    pub use crate::AtomicRef;
}

#[cfg(test)]
mod tests;
