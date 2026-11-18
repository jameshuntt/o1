use core::fmt;
use core::ops::{Deref, DerefMut};

/// Aligns and pads `T` to the size used to keep two hot values on different cache lines.
///
/// | target | alignment | reason |
/// |---|---|---|
/// | `x86_64`, `aarch64`, `powerpc64` | 128 | Intel's adjacent-line prefetcher pulls lines in 128-byte pairs; Apple M-series and some other AArch64 cores have 128-byte lines |
/// | 32-bit `arm`, `mips`, `riscv32`, … | 32 | small lines |
/// | everything else | 64 | common line size |
///
/// The table follows `crossbeam-utils`' `CachePadded` (prior art; not a dependency). It is
/// a performance layout choice, not a correctness requirement: the SPSC ring is correct
/// with any alignment. The measured effect is recorded in the project's benchmark notes,
/// which are not part of the published crate.
#[cfg_attr(
    any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "powerpc64"
    ),
    repr(align(128))
)]
#[cfg_attr(
    any(
        target_arch = "arm",
        target_arch = "mips",
        target_arch = "mips32r6",
        target_arch = "riscv32",
        target_arch = "sparc",
        target_arch = "hexagon",
        target_arch = "m68k",
    ),
    repr(align(32))
)]
#[cfg_attr(
    not(any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "powerpc64",
        target_arch = "arm",
        target_arch = "mips",
        target_arch = "mips32r6",
        target_arch = "riscv32",
        target_arch = "sparc",
        target_arch = "hexagon",
        target_arch = "m68k",
    )),
    repr(align(64))
)]
#[derive(Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CachePadded<T> {
    value: T,
}

impl<T> CachePadded<T> {
    /// Wraps `value`.
    #[inline]
    pub const fn new(value: T) -> Self {
        Self { value }
    }

    /// Unwraps the value.
    #[inline]
    pub fn into_inner(self) -> T {
        self.value
    }
}

impl<T> Deref for CachePadded<T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &T {
        &self.value
    }
}

impl<T> DerefMut for CachePadded<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        &mut self.value
    }
}

impl<T: fmt::Debug> fmt::Debug for CachePadded<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("CachePadded").field(&self.value).finish()
    }
}
