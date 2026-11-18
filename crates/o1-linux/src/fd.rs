//! A borrowed file descriptor for `no_std` code.

use core::fmt;
use core::marker::PhantomData;

/// A file descriptor borrowed for `'fd`, like `std::os::fd::BorrowedFd` (which `no_std`
/// code cannot name). `#[repr(transparent)]` over `i32`.
///
/// Using a descriptor is memory-safe whatever its number; the `unsafe` on
/// [`borrow_raw`](Self::borrow_raw) is **I/O safety** (the same convention as std): the
/// caller promises the number names an open descriptor it may use for `'fd`, so this crate
/// never reads from or writes to a descriptor that belongs to someone else.
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct BorrowedFd<'fd> {
    fd: i32,
    _borrow: PhantomData<&'fd ()>,
}

impl BorrowedFd<'_> {
    /// Borrows a raw descriptor number.
    ///
    /// # Safety
    ///
    /// `fd` must be an open file descriptor that stays open, and that the caller is
    /// entitled to use, for the whole lifetime of the returned value (I/O safety). It must
    /// not be `-1`.
    #[inline]
    #[must_use]
    pub const unsafe fn borrow_raw(fd: i32) -> Self {
        Self {
            fd,
            _borrow: PhantomData,
        }
    }

    /// The descriptor number.
    #[inline]
    #[must_use]
    pub const fn as_raw(self) -> i32 {
        self.fd
    }
}

impl fmt::Debug for BorrowedFd<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("BorrowedFd").field(&self.fd).finish()
    }
}
