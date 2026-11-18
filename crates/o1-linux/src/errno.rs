//! Linux's syscall error-return convention, as pure functions (testable on any host).
//!
//! A raw Linux syscall returns one machine word. Values in `-4095..=-1` (as a signed word)
//! are `-errno`; everything else is a successful result. Errno numbers below are the
//! `asm-generic` ones, identical on x86_64 and aarch64 (cross-checked against
//! `linux-raw-sys` 0.12.1).

use core::fmt;

/// Largest errno value the kernel returns (`MAX_ERRNO`).
pub const MAX_ERRNO: usize = 4095;

/// A Linux errno value, `1..=4095`.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Errno(u16);

impl Errno {
    /// Interrupted system call.
    pub const EINTR: Self = Self(4);
    /// Bad file descriptor.
    pub const EBADF: Self = Self(9);
    /// Try again / operation would block (`EWOULDBLOCK` is the same value).
    pub const EAGAIN: Self = Self(11);
    /// Bad address.
    pub const EFAULT: Self = Self(14);
    /// Invalid argument.
    pub const EINVAL: Self = Self(22);
    /// Broken pipe.
    pub const EPIPE: Self = Self(32);
    /// Function not implemented.
    pub const ENOSYS: Self = Self(38);
    /// Connection reset by peer.
    pub const ECONNRESET: Self = Self(104);
    /// Timed out.
    pub const ETIMEDOUT: Self = Self(110);

    /// The errno for `raw` if it is in `1..=4095`.
    #[inline]
    #[must_use]
    pub const fn new(raw: u16) -> Option<Self> {
        if raw >= 1 && raw as usize <= MAX_ERRNO {
            Some(Self(raw))
        } else {
            None
        }
    }

    /// The errno number.
    #[inline]
    #[must_use]
    pub const fn raw(self) -> u16 {
        self.0
    }

    /// Symbolic name for the values this crate names; `None` for others.
    #[must_use]
    pub const fn name(self) -> Option<&'static str> {
        Some(match self.0 {
            4 => "EINTR",
            9 => "EBADF",
            11 => "EAGAIN",
            14 => "EFAULT",
            22 => "EINVAL",
            32 => "EPIPE",
            38 => "ENOSYS",
            104 => "ECONNRESET",
            110 => "ETIMEDOUT",
            _ => return None,
        })
    }
}

impl fmt::Debug for Errno {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.name() {
            Some(n) => write!(f, "{n}({})", self.0),
            None => write!(f, "Errno({})", self.0),
        }
    }
}

impl fmt::Display for Errno {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl core::error::Error for Errno {}

/// Decodes a raw syscall return word.
///
/// `ret` in `usize::MAX - 4094 ..= usize::MAX` (that is, `-4095..=-1`) is `Err(-ret)`;
/// anything else is `Ok(ret)`. Worst-case O(1), no branches on secret data claimed.
///
/// ```
/// use o1_linux::errno::{decode, Errno};
/// assert_eq!(decode(0), Ok(0));
/// assert_eq!(decode(4096), Ok(4096));
/// assert_eq!(decode((-11isize) as usize), Err(Errno::EAGAIN));
/// assert_eq!(decode((-4095isize) as usize).unwrap_err().raw(), 4095);
/// assert_eq!(decode((-4096isize) as usize), Ok((-4096isize) as usize)); // not an error
/// ```
#[inline]
pub const fn decode(ret: usize) -> Result<usize, Errno> {
    if ret > usize::MAX - MAX_ERRNO {
        // ret.wrapping_neg() is in 1..=4095, which fits u16.
        Err(Errno(ret.wrapping_neg() as u16))
    } else {
        Ok(ret)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boundaries() {
        assert_eq!(decode(usize::MAX), Err(Errno(1)));
        assert_eq!(decode(usize::MAX - 4094), Err(Errno(4095)));
        assert_eq!(decode(usize::MAX - 4095), Ok(usize::MAX - 4095));
        assert_eq!(decode(isize::MAX as usize), Ok(isize::MAX as usize));
        assert_eq!(Errno::new(0), None);
        assert_eq!(Errno::new(4096), None);
        assert_eq!(Errno::new(110), Some(Errno::ETIMEDOUT));
        assert_eq!(std::format!("{:?}", Errno::EINTR), "EINTR(4)");
        assert_eq!(std::format!("{}", Errno(200)), "Errno(200)");
    }
}
