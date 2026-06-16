#![cfg_attr(not(feature = "std"), no_std)]
// `let _ = expr` is used for heapless compatibility (push_str returns Result in no_std).
#![allow(clippy::let_unit_value)]
// Pre-existing clippy issues suppressed during refactor (will clean up separately).
#![allow(clippy::clone_on_copy)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::needless_range_loop)]
#![allow(clippy::if_same_then_else)]
#![allow(clippy::not_unsafe_ptr_arg_deref)]
#![allow(clippy::collapsible_match)]

pub mod buffers;
pub mod engine;
pub mod modes;
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
pub use crate::syllable::{NucleusKind, OnsetKind, SylStructure};
