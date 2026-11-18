//! Futex wait and wake on an [`AtomicU32`] — process-private futexes only.
//!
//! # Two paths, kept apart
//!
//! * **Userspace fast path** — an ordinary atomic load of the word, done by the caller or by
//!   [`wait_while_equal`] before it enters the kernel. Bounded: one load. If the value
//!   already differs from `expected`, no syscall happens.
//! * **Kernel slow path** — [`wait`] and [`wake`], one `futex(2)` syscall each. The kernel
//!   atomically re-checks `*word == expected` and parks the thread; how long the thread
//!   then sleeps, and how long a woken thread takes to run, is decided by the kernel and
//!   the scheduler. **No O(1), bounded-step or bounded-latency guarantee applies** to this
//!   path.
//!
//! # Outcomes of a wait
//!
//! | kernel result | [`WaitOutcome`] | meaning |
//! |---|---|---|
//! | 0 | `Woken` | a `wake` happened **or a spurious wake-up** — re-check the word |
//! | `EAGAIN` | `ValueMismatch` | `*word != expected` when the kernel checked |
//! | `EINTR` | `Interrupted` | a signal handler ran |
//! | `ETIMEDOUT` | `TimedOut` | the relative timeout elapsed |
//!
//! # Memory ordering
//!
//! The futex syscall is not a Rust synchronisation operation; it creates no happens-before
//! edge in Rust's memory model. Correct use: the waker changes the word with a `Release`
//! (or stronger) store/RMW **before** calling [`wake`]; the waiter re-reads the word with
//! `Acquire` after every return from [`wait`] and decides from that value. The kernel's
//! own compare (under its hash-bucket lock) is what prevents a lost wake-up between the
//! waiter's check and its sleep.

use core::sync::atomic::{AtomicU32, Ordering};
use core::time::Duration;

use crate::errno::{Errno, decode};
use crate::syscall::{Arg, int, nr, syscall4};

const FUTEX_WAIT: usize = 0;
const FUTEX_WAKE: usize = 1;
const FUTEX_PRIVATE_FLAG: usize = 128;

/// Kernel `struct __kernel_timespec` (64-bit seconds and nanoseconds on both targets).
#[repr(C)]
struct Timespec {
    sec: i64,
    nsec: i64,
}

/// What one kernel wait returned. See the module docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitOutcome {
    /// Returned 0: woken by [`wake`] or spuriously.
    Woken,
    /// `EAGAIN`: the word did not equal `expected`.
    ValueMismatch,
    /// `EINTR`: interrupted by a signal.
    Interrupted,
    /// `ETIMEDOUT`: the timeout elapsed.
    TimedOut,
}

/// One `FUTEX_WAIT_PRIVATE` call: sleeps while `*word == expected`, until woken, a signal,
/// a spurious wake-up, or `timeout` (relative; `None` = no timeout). Seconds beyond
/// `i64::MAX` saturate.
///
/// # Errors
///
/// Any errno other than the three mapped to [`WaitOutcome`] (for example `EFAULT`, `EINVAL`,
/// `ENOSYS`); none is expected for a valid `&AtomicU32`.
pub fn wait(
    word: &AtomicU32,
    expected: u32,
    timeout: Option<Duration>,
) -> Result<WaitOutcome, Errno> {
    let ts = timeout.map(|d| Timespec {
        sec: i64::try_from(d.as_secs()).unwrap_or(i64::MAX),
        nsec: i64::from(d.subsec_nanos()),
    });
    let ts_ptr: Arg = match &ts {
        Some(t) => (&raw const *t).cast_mut().cast(),
        None => core::ptr::null_mut(),
    };
    // SAFETY: [unsafe:linux.futex-wait] `word.as_ptr()` is a valid, 4-byte-aligned u32 for the
    // whole call (borrowed); the kernel only reads it atomically. `ts_ptr` is null or points
    // to a live `Timespec` on this frame with the kernel's layout, which the kernel only
    // reads. The futex is private, so it cannot affect another process.
    let ret = unsafe {
        syscall4(
            nr::FUTEX,
            word.as_ptr().cast(),
            int(FUTEX_WAIT | FUTEX_PRIVATE_FLAG),
            int(expected as usize),
            ts_ptr,
        )
    };
    match decode(ret) {
        Ok(_) => Ok(WaitOutcome::Woken),
        Err(Errno::EAGAIN) => Ok(WaitOutcome::ValueMismatch),
        Err(Errno::EINTR) => Ok(WaitOutcome::Interrupted),
        Err(Errno::ETIMEDOUT) => Ok(WaitOutcome::TimedOut),
        Err(e) => Err(e),
    }
}

/// One `FUTEX_WAKE_PRIVATE` call: wakes up to `count` threads waiting on `word` (counts
/// above `i32::MAX` are clamped). Returns how many were woken.
///
/// # Errors
///
/// The kernel's errno (none is expected for a valid `&AtomicU32`).
pub fn wake(word: &AtomicU32, count: u32) -> Result<usize, Errno> {
    let count = count.min(i32::MAX as u32) as usize;
    // SAFETY: [unsafe:linux.futex-wake] `word.as_ptr()` is a valid aligned u32 for the call;
    // FUTEX_WAKE uses it only as a key (no access through the 4th argument, passed as null).
    let ret = unsafe {
        syscall4(
            nr::FUTEX,
            word.as_ptr().cast(),
            int(FUTEX_WAKE | FUTEX_PRIVATE_FLAG),
            int(count),
            core::ptr::null_mut(),
        )
    };
    decode(ret)
}

/// Blocks until `word` no longer equals `expected`, as observed by an `Acquire` load.
///
/// Fast path: one `Acquire` load; returns immediately if the value already differs.
/// Slow path: loops over [`wait`], absorbing spurious wake-ups, `EINTR` and value
/// mismatches by re-checking the word. Not wait-free and not bounded: it returns only once
/// another thread changes the word (and, to avoid sleeping forever, calls [`wake`]).
///
/// # Errors
///
/// An unexpected errno from [`wait`].
pub fn wait_while_equal(word: &AtomicU32, expected: u32) -> Result<(), Errno> {
    loop {
        if word.load(Ordering::Acquire) != expected {
            return Ok(());
        }
        // Every outcome (Woken, ValueMismatch, Interrupted, TimedOut-without-timeout) is
        // handled the same way: re-check the word.
        let _ = wait(word, expected, None)?;
    }
}
