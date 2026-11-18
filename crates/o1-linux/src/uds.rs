//! Framed messages over a Unix-domain **stream** socket, driving [`o1_core::frame`].
//!
//! The framing state machines are OS-independent and live in `o1-core` (tested and
//! Miri-checked on any host). This module only moves bytes: `read` straight into the
//! decoder's spare slice (payload bytes land in the caller's buffer once — copy-avoiding
//! userspace framing, not zero-copy: the kernel still copies from the socket buffer), and
//! `sendmsg(MSG_NOSIGNAL)` of the encoder's header and payload slices in one vectored call.
//!
//! * **Blocking or non-blocking descriptors.** On `EAGAIN` the functions return
//!   [`RecvOutcome::WouldBlock`] / [`SendOutcome::WouldBlock`]; the decoder/encoder keep every byte already
//!   transferred, so calling again later resumes exactly where it stopped.
//! * **`EINTR`** is retried inside the call (no bytes are lost: an interrupted call
//!   transferred nothing). The number of retries is not bounded by this code; it is bounded by
//!   how many signals arrive.
//! * **EOF**: on a frame boundary it is [`RecvOutcome::Eof`]; inside a frame it is
//!   [`DecodeError::UnexpectedEof`].
//! * **Peer closed while sending**: `EPIPE` (or `ECONNRESET`) as [`UdsError::Os`]; never
//!   `SIGPIPE`.
//! * **Malicious lengths**: bounded by the decoder (refused, or discarded up to its explicit
//!   discard limit). No allocation anywhere.
//!
//! Socket creation, path hygiene and peer credentials are out of scope (that is the `mine`
//! crate's job); any connected `SOCK_STREAM` socket descriptor works. `recv_frame` also works
//! on pipes; `send_frame` needs a socket (`sendmsg`).

use core::fmt;

use o1_core::frame::{DecodeError, EncodeError, FrameDecoder, FrameEncoder, Status};

use crate::errno::Errno;
use crate::fd::BorrowedFd;
use crate::io::{IoSlice, read, send_vectored};

/// Result of [`recv_frame`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecvOutcome {
    /// A frame of this length is ready in the decoder (`FrameDecoder::frame`); call
    /// `FrameDecoder::release` before receiving the next one.
    Frame(usize),
    /// An oversized frame (within the decoder's discard limit) was skipped.
    Discarded(usize),
    /// Clean end of stream on a frame boundary.
    Eof,
    /// Non-blocking descriptor has no data now; progress so far is kept.
    WouldBlock,
}

/// Result of [`send_frame`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendOutcome {
    /// The whole frame has been handed to the kernel.
    Done,
    /// Non-blocking descriptor's buffer is full; progress so far is kept.
    WouldBlock,
}

/// Why a framed transfer failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UdsError {
    /// The kernel returned this errno (other than `EINTR`/`EAGAIN`, which are handled).
    Os(Errno),
    /// The decoder refused the stream (malformed/oversized length, EOF inside a frame).
    Decode(DecodeError),
    /// The encoder refused (payload over `u32::MAX`, or a driver bug).
    Encode(EncodeError),
    /// `sendmsg` reported 0 bytes sent for a non-empty send.
    WriteZero,
}

impl fmt::Display for UdsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Os(e) => write!(f, "os error {e}"),
            Self::Decode(e) => write!(f, "frame decode: {e}"),
            Self::Encode(e) => write!(f, "frame encode: {e}"),
            Self::WriteZero => f.write_str("sendmsg wrote zero bytes"),
        }
    }
}

impl core::error::Error for UdsError {}

impl From<DecodeError> for UdsError {
    fn from(e: DecodeError) -> Self {
        Self::Decode(e)
    }
}

impl From<EncodeError> for UdsError {
    fn from(e: EncodeError) -> Self {
        Self::Encode(e)
    }
}

/// Reads from `fd` until a frame completes, a frame is discarded, the stream ends, or the
/// descriptor would block.
///
/// # Errors
///
/// [`UdsError::Decode`] (the decoder is then poisoned) or [`UdsError::Os`].
pub fn recv_frame(
    fd: BorrowedFd<'_>,
    decoder: &mut FrameDecoder<'_>,
) -> Result<RecvOutcome, UdsError> {
    loop {
        let spare = decoder.spare()?;
        let n = match read(fd, spare) {
            Ok(n) => n,
            Err(Errno::EINTR) => continue,
            Err(Errno::EAGAIN) => return Ok(RecvOutcome::WouldBlock),
            Err(e) => return Err(UdsError::Os(e)),
        };
        match decoder.filled(n)? {
            Status::NeedMore => {}
            Status::Ready(len) => return Ok(RecvOutcome::Frame(len)),
            Status::Discarded(len) => return Ok(RecvOutcome::Discarded(len)),
            Status::Eof => return Ok(RecvOutcome::Eof),
        }
    }
}

/// Sends the rest of `encoder`'s frame on socket `fd`, one `sendmsg` per step.
///
/// # Errors
///
/// [`UdsError::Os`] (e.g. `EPIPE` when the peer closed), [`UdsError::WriteZero`], or
/// [`UdsError::Encode`] if the kernel reported more bytes than were pending.
pub fn send_frame(
    fd: BorrowedFd<'_>,
    encoder: &mut FrameEncoder<'_>,
) -> Result<SendOutcome, UdsError> {
    while !encoder.is_done() {
        let (header, payload) = encoder.pending();
        let bufs = [IoSlice::new(header), IoSlice::new(payload)];
        let n = match send_vectored(fd, &bufs) {
            Ok(n) => n,
            Err(Errno::EINTR) => continue,
            Err(Errno::EAGAIN) => return Ok(SendOutcome::WouldBlock),
            Err(e) => return Err(UdsError::Os(e)),
        };
        if n == 0 {
            return Err(UdsError::WriteZero);
        }
        encoder.advance(n)?;
    }
    Ok(SendOutcome::Done)
}
