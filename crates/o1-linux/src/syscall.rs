//! The only `asm!` in o1: raw Linux syscall entry for x86_64 and aarch64.
//!
//! Arguments are passed as pointer-typed registers (`*mut ()`): pointer arguments keep
//! their provenance (they are handed to the kernel, which may access that memory), and
//! integer arguments are made with `ptr::without_provenance_mut` — no pointer↔integer
//! round trip and no exposed provenance. Only the arities this crate uses (3 and 4) exist.
//!
//! Register conventions (Linux syscall ABI):
//!
//! | | number | args 1–4 | return | clobbered by the instruction |
//! |---|---|---|---|---|
//! | x86_64 `syscall` | `rax` | `rdi rsi rdx r10` | `rax` | `rcx` (return RIP), `r11` (saved RFLAGS) |
//! | aarch64 `svc 0` | `x8` | `x0 x1 x2 x3` | `x0` | none besides `x0` |
//!
//! Options: `nostack` (neither instruction touches the user stack, and the kernel does not
//! write below the user stack pointer), `preserves_flags` (x86_64 `sysret` restores RFLAGS
//! from `r11`; aarch64 `eret` restores PSTATE). **Not** `nomem`/`readonly`/`pure`: these
//! syscalls read and write user memory through their pointer arguments and have side effects,
//! so the compiler must assume the asm reads and writes memory.
//!
//! The return word is decoded by [`crate::errno::decode`].

use core::arch::asm;

/// A syscall argument register value.
pub(crate) type Arg = *mut ();

/// An integer argument (no provenance).
#[inline]
pub(crate) const fn int(v: usize) -> Arg {
    core::ptr::without_provenance_mut(v)
}

#[cfg(target_arch = "x86_64")]
pub(crate) mod nr {
    pub(crate) const READ: usize = 0;
    pub(crate) const WRITE: usize = 1;
    pub(crate) const SENDMSG: usize = 46;
    pub(crate) const FUTEX: usize = 202;
}

#[cfg(target_arch = "aarch64")]
pub(crate) mod nr {
    pub(crate) const READ: usize = 63;
    pub(crate) const WRITE: usize = 64;
    pub(crate) const SENDMSG: usize = 211;
    pub(crate) const FUTEX: usize = 98;
}

/// Three-argument syscall.
///
/// # Safety
///
/// `nr` and the arguments must form a syscall whose memory accesses through pointer
/// arguments are valid for the duration of the call (each pointer valid for the reads and
/// writes that syscall performs with the given lengths), and whose side effects (on file
/// descriptors, memory mappings, signals, …) do not violate any invariant Rust code relies
/// on.
#[inline]
pub(crate) unsafe fn syscall3(nr: usize, a0: Arg, a1: Arg, a2: Arg) -> usize {
    let ret: usize;
    #[cfg(target_arch = "x86_64")]
    // SAFETY: [unsafe:linux.asm3-x86_64] register assignment per the Linux x86_64 syscall
    // ABI; rcx/r11 declared clobbered; memory effects are not hidden from the compiler
    // (no nomem/readonly). Validity of the call itself is this function's contract.
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") nr => ret,
            in("rdi") a0,
            in("rsi") a1,
            in("rdx") a2,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack, preserves_flags),
        );
    }
    #[cfg(target_arch = "aarch64")]
    // SAFETY: [unsafe:linux.asm3-aarch64] register assignment per the Linux aarch64 syscall
    // ABI (x8 number, x0..x2 args, x0 return); memory effects not hidden from the compiler.
    // Validity of the call itself is this function's contract.
    unsafe {
        asm!(
            "svc 0",
            in("x8") nr,
            inlateout("x0") a0 => ret,
            in("x1") a1,
            in("x2") a2,
            options(nostack, preserves_flags),
        );
    }
    ret
}

/// Four-argument syscall.
///
/// # Safety
///
/// As for [`syscall3`].
#[inline]
pub(crate) unsafe fn syscall4(nr: usize, a0: Arg, a1: Arg, a2: Arg, a3: Arg) -> usize {
    let ret: usize;
    #[cfg(target_arch = "x86_64")]
    // SAFETY: [unsafe:linux.asm4-x86_64] as for syscall3; the fourth argument goes in r10
    // (not rcx, which `syscall` overwrites).
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") nr => ret,
            in("rdi") a0,
            in("rsi") a1,
            in("rdx") a2,
            in("r10") a3,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack, preserves_flags),
        );
    }
    #[cfg(target_arch = "aarch64")]
    // SAFETY: [unsafe:linux.asm4-aarch64] as for syscall3; fourth argument in x3.
    unsafe {
        asm!(
            "svc 0",
            in("x8") nr,
            inlateout("x0") a0 => ret,
            in("x1") a1,
            in("x2") a2,
            in("x3") a3,
            options(nostack, preserves_flags),
        );
    }
    ret
}
