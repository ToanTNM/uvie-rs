#![cfg_attr(not(feature = "std"), no_std)]

pub mod buffers;
pub mod engine;
pub mod modes;
pub mod phonetics;
pub mod tone;

#[cfg(test)]
mod tests;

#[cfg(feature = "std")]
pub mod ffi;

pub use crate::engine::UltraFastViEngine;
pub use crate::modes::{InputMethod, ModeTrait, TelexMode, VniMode};
