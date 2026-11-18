//! `read`, `write`, and `send_vectored` (`sendmsg` with `MSG_NOSIGNAL`) on a [`BorrowedFd`].
//!
//! Each function makes exactly one syscall and returns its result; `EINTR` and `EAGAIN` are
//! returned to the caller, not retried here. No bounded-latency claim: these enter the kernel.

use core::marker::PhantomData;
use core::mem::{offset_of, size_of};

use crate::errno::{Errno, decode};
use crate::fd::BorrowedFd;
use crate::syscall::{Arg, int, nr, syscall3};

/// `MSG_NOSIGNAL`: a send to a closed peer fails with `EPIPE` instead of raising `SIGPIPE`.
const MSG_NOSIGNAL: usize = 0x4000;

/// One buffer of a vectored write: `#[repr(C)]`, layout-identical to the kernel's
/// `struct iovec` (pointer, length) on x86_64 and aarch64.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct IoSlice<'a> {
    base: *const u8,
    len: usize,
    _borrow: PhantomData<&'a [u8]>,
}

impl<'a> IoSlice<'a> {
    /// Wraps a byte slice.
    #[inline]
    #[must_use]
    pub const fn new(buf: &'a [u8]) -> Self {
        Self {
            base: buf.as_ptr(),
            len: buf.len(),
            _borrow: PhantomData,
        }
    }

    /// Length in bytes.
    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Whether the slice is empty.
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// Kernel `struct user_msghdr` (x86_64 and aarch64: 56 bytes).
#[repr(C)]
struct MsgHdr {
    name: *mut (),
    namelen: i32,
    iov: *const IoSlice<'static>,
    iovlen: usize,
    control: *mut (),
    controllen: usize,
    flags: u32,
}

const _: () = {
    assert!(size_of::<IoSlice<'static>>() == 16);
    assert!(offset_of!(IoSlice<'static>, len) == 8);
    assert!(size_of::<MsgHdr>() == 56);
    assert!(offset_of!(MsgHdr, iov) == 16);
    assert!(offset_of!(MsgHdr, iovlen) == 24);
    assert!(offset_of!(MsgHdr, control) == 32);
    assert!(offset_of!(MsgHdr, controllen) == 40);
    assert!(offset_of!(MsgHdr, flags) == 48);
};

/// Reads up to `buf.len()` bytes. `Ok(0)` is end of stream (or an empty `buf`).
///
/// # Errors
///
/// The kernel's errno, including `EINTR` (signal, nothing read) and `EAGAIN` (non-blocking
/// descriptor with no data).
#[inline]
pub fn read(fd: BorrowedFd<'_>, buf: &mut [u8]) -> Result<usize, Errno> {
    // SAFETY: [unsafe:linux.read] `buf` is valid for writes of `buf.len()` bytes for the whole
    // call (exclusive borrow); `read` writes at most that many bytes there and nothing else
    // in this process's memory. `fd` is a descriptor the caller may use (I/O safety, see
    // `BorrowedFd::borrow_raw`).
    let ret = unsafe {
        syscall3(
            nr::READ,
            int(fd.as_raw() as u32 as usize),
            buf.as_mut_ptr().cast(),
            int(buf.len()),
        )
    };
    decode(ret)
}

/// Writes up to `buf.len()` bytes; returns how many were written.
///
/// On a socket whose peer has closed, `write` raises `SIGPIPE`; use [`send_vectored`] for
/// sockets.
///
/// # Errors
///
/// The kernel's errno, including `EINTR` and `EAGAIN`.
#[inline]
pub fn write(fd: BorrowedFd<'_>, buf: &[u8]) -> Result<usize, Errno> {
    // SAFETY: [unsafe:linux.write] `buf` is valid for reads of `buf.len()` bytes for the whole
    // call; `write` only reads them. I/O safety as for `read`.
    let ret = unsafe {
        syscall3(
            nr::WRITE,
            int(fd.as_raw() as u32 as usize),
            buf.as_ptr().cast_mut().cast(),
            int(buf.len()),
        )
    };
    decode(ret)
}

/// Sends the buffers in order with one `sendmsg(fd, msg, MSG_NOSIGNAL)` on a **socket**;
/// returns how many bytes were sent (possibly fewer than the total: a partial send).
///
/// `MSG_NOSIGNAL` turns "peer closed" into `Err(EPIPE)` instead of a `SIGPIPE` that would
/// terminate a process that has not ignored it.
///
/// # Errors
///
/// The kernel's errno, including `EINTR`, `EAGAIN`, `EPIPE`, and `ENOTSOCK` (88) for a
/// descriptor that is not a socket.
#[inline]
pub fn send_vectored(fd: BorrowedFd<'_>, bufs: &[IoSlice<'_>]) -> Result<usize, Errno> {
    let msg = MsgHdr {
        name: core::ptr::null_mut(),
        namelen: 0,
        iov: bufs.as_ptr().cast(),
        iovlen: bufs.len(),
        control: core::ptr::null_mut(),
        controllen: 0,
        flags: 0,
    };
    let msg_ptr: Arg = (&raw const msg).cast_mut().cast();
    // SAFETY: [unsafe:linux.sendmsg] `msg` lives on this stack frame for the whole call and has
    // the kernel's `user_msghdr` layout (const-asserted above); its `iov` points to `bufs`,
    // whose `IoSlice`s are layout-identical to `iovec` and each borrow a live slice; the
    // kernel only reads all of these. No name/control buffers are passed. I/O safety as for
    // `read`.
    let ret = unsafe {
        syscall3(
            nr::SENDMSG,
            int(fd.as_raw() as u32 as usize),
            msg_ptr,
            int(MSG_NOSIGNAL),
        )
    };
    decode(ret)
}
