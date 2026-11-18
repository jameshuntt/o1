//! Explicit Linux ABI layer for x86_64 and aarch64. **Linux-specific.**
//!
//! | module | targets | contents |
//! |---|---|---|
//! | [`errno`] | every target (pure) | Linux's `-4095..=-1` error-return convention, errno values |
//! | `fd` | Linux | `BorrowedFd` for `no_std` |
//! | `io` | Linux | `read`, `write`, `send_vectored` (`sendmsg` + `MSG_NOSIGNAL`) |
//! | `futex` | Linux | `wait`, `wake`, `wait_while_equal` on `AtomicU32` |
//! | `uds` | Linux | drives [`o1_core::frame`] over a stream-socket fd |
//!
//! # Why raw syscalls
//!
//! Not speed: a libc wrapper is not the cost that matters. The reasons are architectural —
//! a `#![no_std]` Linux build with no libc, and an ABI layer whose every register, clobber,
//! constant and struct layout is written down in this crate and cross-checked against
//! `linux-raw-sys` 0.12.1 (see `docs/DESIGN.md` §7). The `asm!` lives in one private module;
//! everything public is a safe function whose arguments make the kernel's memory accesses
//! valid by construction.
//!
//! # Targets
//!
//! * Linux on x86_64 or aarch64: everything.
//! * Linux on any other architecture: `compile_error!` (no silent stub).
//! * Any other OS: only [`errno`] (pure code); the Linux modules do not exist.
//!
//! # Not claimed
//!
//! No operation that enters the kernel has a bounded-latency or O(1) guarantee: scheduling,
//! signal delivery, socket buffers and page faults are the kernel's. Runtime behaviour on
//! Linux is tested only where a Linux machine runs the test suite (see
//! `docs/VERIFICATION.md`).
#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]
#![warn(missing_docs)]

#[cfg(test)]
extern crate std;

#[cfg(all(
    target_os = "linux",
    not(any(target_arch = "x86_64", target_arch = "aarch64"))
))]
compile_error!("o1-linux supports Linux only on x86_64 and aarch64");

pub mod errno;

#[cfg(target_os = "linux")]
pub mod fd;
#[cfg(target_os = "linux")]
pub mod futex;
#[cfg(target_os = "linux")]
pub mod io;
#[cfg(target_os = "linux")]
mod syscall;
#[cfg(target_os = "linux")]
pub mod uds;
