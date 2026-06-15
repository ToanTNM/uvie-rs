#![cfg_attr(not(feature = "std"), no_std)]

pub mod buffers;
pub mod engine;
pub mod modes;
pub mod replay;
pub mod syllable;
pub mod tables;
pub mod tone;

/// Deprecated: replaced by `tables.rs` positive validation.
///
/// This module will be removed in a future release. It is kept temporarily
/// so downstream code that imports `uvie::phonetics` does not break before
/// migration is complete.
#[deprecated(
    since = "2.0.0",
    note = "Use `uvie::tables` positive-pattern validation instead."
)]
pub mod phonetics;

#[cfg(test)]
mod tests;

#[cfg(feature = "std")]
pub mod ffi;

pub use crate::engine::UltraFastViEngine;
pub use crate::modes::{InputMethod, ModeTrait, TelexMode, VniMode};
pub use crate::replay::ReplayEngine;
