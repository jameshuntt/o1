//! OS-independent, allocation-free substrate primitives.
//!
//! `o1-core` is `#![no_std]` and never uses `alloc`. Nothing in it calls the operating
//! system. It is verified to build for `thumbv7em-none-eabihf` (a target with no OS,
//! no allocator and no 64-bit atomics).
//!
//! | module | contents | `unsafe` inside? |
//! |---|---|---|
//! | [`bitops`] | bit scanning, masks and selection with an ISA-independent contract | no |
//! | [`ptr`] | strict-provenance tagged pointers, alignment helpers | yes (dereference, atomics) |
//! | [`collections`] | [`FixedRing`](collections::FixedRing), [`SparseSet`](collections::SparseSet) | `FixedRing` only |
//! | [`arena`] | [`FreeList`](arena::FreeList), [`GenerationalArena`](arena::GenerationalArena), [`Handle`](arena::Handle) | no |
//! | [`frame`] | sans-IO length-prefixed framing over caller-owned buffers | no |
//! | [`stream`] | [`BlockPass`](stream::BlockPass): one sequential pass over fixed-size blocks | no |
//!
//! # Vocabulary
//!
//! Documentation in this crate keeps these properties apart and never uses one for
//! another: worst-case O(1) work; bounded-step / fixed-capacity; allocation-free;
//! wait-free; lock-free; branchless; ISA/intrinsic-backed; constant-time (secret-independent
//! timing — **no operation in o1 claims this**); bounded wall-clock latency (**never
//! claimed**). See `docs/GUARANTEES.md` in the repository for the per-operation matrix.
//!
//! # Panics
//!
//! No public function in this crate panics on any input, except where its documentation
//! says so. Capacity and tag-width errors in type parameters are compile-time errors.
#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]
#![warn(missing_docs)]

#[cfg(test)]
extern crate std;

pub mod arena;
pub mod bitops;
pub mod collections;
pub mod frame;
pub mod ptr;
pub mod stream;
