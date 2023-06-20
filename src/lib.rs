//! Atomics which can be subscribed to and asynchronously notify when updated.
//!
//! The main structure is [`Atomic`] that behaves like stdlib's atomics,
//! but don't take an explicit [`Ordering`](`core::sync::atomic::Ordering`) for simplicity.
//!
//! An [`Subscriber`] can be created from [`Atomic`] to asynchronously wait for changes.
//! It is only one subscriber allowed for each atomic.

#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

mod atomic;
mod subscriber;

pub use crate::atomic::*;
pub use subscriber::*;

#[cfg(all(test, feature = "std"))]
mod tests;
