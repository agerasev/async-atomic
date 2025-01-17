//! Atomics which can be subscribed to and asynchronously notify when updated.
//!
//! The main structure is [`Atomic`] that behaves like stdlib's atomics,
//! but don't take an explicit [`Ordering`](`core::sync::atomic::Ordering`) for simplicity.
//!
//! An [`Atomic`] can asynchronously wait for updates using methods from [`AsyncAtomic`] trait.
//!
//! *Note that if there are more than one future at the same time then only the most recently `poll`ed future will be notified.*
//! *Older futures will never receive an update, so it's up to user to ensure that only one of them `.await`ing at a time.*

#![no_std]

mod async_;
mod atomic;

pub use atomig::Atom;

pub use async_::*;
pub use atomic::*;

pub mod prelude {
    pub use crate::AsyncAtomic;
}

#[cfg(test)]
mod tests;
