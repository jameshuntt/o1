//! The o1 substrate in one dependency.
//!
//! Re-exports, nothing else. This crate contains no `unsafe` (`#![forbid(unsafe_code)]`);
//! every unsafe site lives in the crates below and is listed in `docs/UNSAFE_INVENTORY.md`.
//!
//! | path | crate | targets |
//! |---|---|---|
//! | [`bitops`], [`ptr`], [`collections`], [`arena`], [`frame`], [`stream`] | `o1-core` | all (`no_std`, no alloc) |
//! | [`spsc`], [`CachePadded`] | `o1-sync` | all with pointer-width atomics |
//! | [`contract`] | `o1-contract` | all (`no_std`, no alloc, no unsafe) |
//! | [`linux`] | `o1-linux` | `errno` everywhere; the rest on Linux x86_64/aarch64 |
//!
//! ```
//! use o1::collections::FixedRing;
//! let mut r = FixedRing::<u8, 2>::new();
//! assert!(r.push_back(1).is_ok());
//!
//! use o1::contract::{Policy, Quantity};
//! use o1::stream::BlockPass;
//! let pass = BlockPass::<256>::with(Policy::new().streaming());
//! let done = pass.read(&[0u8; 1000], |_block| {});
//! assert_eq!((done.blocks, done.remainder), (3, 232));
//! ```
#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub use o1_core::{arena, bitops, collections, frame, ptr, stream};
pub use o1_sync::{CachePadded, spsc};

/// Execution policies, execution contracts and resource envelopes (`o1-contract`).
pub use o1_contract as contract;

/// The Linux ABI layer (`o1-linux`). Only [`errno`](o1_linux::errno) exists off Linux.
pub use o1_linux as linux;
