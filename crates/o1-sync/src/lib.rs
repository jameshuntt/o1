//! Allocation-free bounded synchronisation primitives.
//!
//! * [`spsc`]: a fixed-capacity single-producer single-consumer ring. `push`/`pop` are
//!   **wait-free** (a fixed number of steps, no loop, no compare-exchange) and
//!   allocation-free. They are not "bounded latency": cache misses, contention on the two
//!   index lines, and preemption are outside any guarantee.
//! * [`CachePadded`]: aligns (and so pads) a value to 128 bytes on x86_64, aarch64 and
//!   powerpc64, 32 bytes on arm, mips, mips32r6, riscv32, sparc, hexagon and m68k, and 64 bytes
//!   on every other target; used to keep the producer's and consumer's indices off one line.
//!
//! `#![no_std]`, no `alloc`, no OS calls. Needs `AtomicUsize` load/store only
//! (`target_has_atomic = "ptr"`), so it builds for `thumbv7em-none-eabihf`, which has 32-bit
//! but no 64-bit atomics.
//!
//! # Loom
//!
//! Building with `RUSTFLAGS="--cfg loom"` swaps the atomics and the slot cells for
//! [loom](https://docs.rs/loom)'s model-checked versions (one switch module). The Loom
//! models live in this crate's unit tests.
//!
//! # Feature `access-counting`
//!
//! Off by default. Tooling only: compiles the crate's counting switch into the ring, so a
//! verifier can count each `push`/`pop`'s atomic loads, stores and slot accesses (module
//! `counting`, present only with the feature; thread-local counters, needs `std`). It changes
//! no ordering and no result; it adds a thread-local update to every counted access, so it is
//! never for production. Ignored under `--cfg loom`.
#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]
#![warn(missing_docs)]

#[cfg(any(test, feature = "access-counting"))]
extern crate std;

mod padded;
pub mod spsc;
mod sync;

pub use padded::CachePadded;

/// Access counting for verification tooling (feature `access-counting`, off by default; needs
/// `std`): per-thread counts of the SPSC ring's atomic loads, stores and slot accesses.
#[cfg(all(feature = "access-counting", not(loom)))]
pub use sync::counting;
