//! Sans-IO length-prefixed framing over caller-owned buffers.
//!
//! Wire format: a 4-byte **little-endian `u32`** payload length, then the payload. This is
//! the format of the `abut` crate (0.2.x), so the two interoperate on one stream.
//!
//! Nothing here performs I/O, allocates, or depends on an OS. An I/O layer (for example
//! `o1_linux::uds`, or a test) drives the state machines:
//!
//! * [`FrameDecoder`]: ask [`spare`](FrameDecoder::spare) for where the next bytes go, read
//!   into it, report the count with [`filled`](FrameDecoder::filled). Payload bytes are read
//!   straight into their final place in the caller's buffer: **copy-avoiding userspace
//!   framing** (the kernel→userspace copy of `read` still happens; there is no second copy).
//! * [`FrameEncoder`]: [`pending`](FrameEncoder::pending) gives the unsent header bytes and
//!   payload bytes as two slices for one vectored write; report the count with
//!   [`advance`](FrameEncoder::advance). The payload is borrowed, never copied.
//!
//! Partial reads and writes may end anywhere (inside the header, inside the payload, on a
//! boundary); the state machines keep their place, so non-blocking I/O can stop on
//! `EAGAIN` and resume later with no data lost.
//!
//! **Bounded by construction:** the decoder never stores more than its buffer holds, and a
//! peer-supplied length cannot cause allocation or unbounded work: lengths above the limit
//! are refused (or, up to an explicit `discard_limit`, read into the same buffer and
//! dropped).

use core::fmt;

use o1_contract::{
    Access, Cost, ExecutionContract, Isolation, PeakGrowth, PolicyDesc, Progress, Quantity,
    ResourceEnvelope, Sharing, Unit,
};

/// Size of the length prefix in bytes.
pub const HEADER_LEN: usize = 4;

/// What [`FrameDecoder::filled`] observed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// More bytes are needed; call [`FrameDecoder::spare`] again.
    NeedMore,
    /// A frame of this many bytes is complete; read it with [`FrameDecoder::frame`] and then
    /// call [`FrameDecoder::release`].
    Ready(usize),
    /// An oversized frame of this length (within `discard_limit`) was read and dropped. The
    /// stream is still aligned; the decoder is ready for the next header.
    Discarded(usize),
    /// End of stream on a frame boundary (`filled(0)` with no partial frame). Clean close.
    Eof,
}

/// Why decoding stopped. The decoder is poisoned: every later call returns the same error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    /// End of stream inside a frame (after `got` of the header or payload bytes).
    UnexpectedEof {
        /// Bytes of the current header (if in the header) or payload that had arrived.
        got: usize,
        /// Bytes that header or payload needed.
        needed: usize,
    },
    /// The announced length exceeds `max_len` and `discard_limit`.
    Oversized {
        /// Announced payload length.
        len: u32,
        /// The decoder's maximum accepted length.
        max: usize,
    },
    /// `filled(n)` reported more bytes than `spare()` offered (a bug in the I/O driver).
    Overfill {
        /// Bytes reported.
        reported: usize,
        /// Bytes that were available.
        available: usize,
    },
    /// `spare`/`filled` were called while a frame was ready and not yet released.
    NotReleased,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::UnexpectedEof { got, needed } => {
                write!(f, "end of stream inside a frame ({got} of {needed} bytes)")
            }
            Self::Oversized { len, max } => write!(f, "frame length {len} exceeds maximum {max}"),
            Self::Overfill {
                reported,
                available,
            } => write!(f, "{reported} bytes reported but only {available} offered"),
            Self::NotReleased => f.write_str("previous frame not released"),
        }
    }
}

impl core::error::Error for DecodeError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DState {
    Header { have: u8 },
    Body { len: usize, have: usize },
    Discard { len: usize, remaining: usize },
    Ready { len: usize },
    Failed(DecodeError),
}

/// Incremental decoder of length-prefixed frames into a caller-owned buffer.
///
/// * `max_len` = the buffer's length unless lowered with [`with_max_len`](Self::with_max_len).
/// * Oversized frames are an error ([`DecodeError::Oversized`]) unless their length is
///   `<= discard_limit` ([`with_discard_limit`](Self::with_discard_limit), default 0 = never
///   discard), in which case they are read into the buffer in pieces and dropped.
/// * A zero-length frame is valid ([`Status::Ready(0)`](Status::Ready)).
/// * Each call is O(1) work apart from copying the 4 header bytes; no allocation.
///
/// ```
/// use o1_core::frame::{FrameDecoder, Status};
/// let wire = [3, 0, 0, 0, b'a', b'b', b'c'];
/// let mut buf = [0u8; 16];
/// let mut d = FrameDecoder::new(&mut buf);
/// let mut input = &wire[..];
/// loop {
///     let spare = d.spare().unwrap();
///     let n = spare.len().min(input.len()).min(2); // a "read" of at most 2 bytes
///     spare[..n].copy_from_slice(&input[..n]);
///     input = &input[n..];
///     match d.filled(n).unwrap() {
///         Status::Ready(len) => { assert_eq!(d.frame(), Some(&b"abc"[..])); assert_eq!(len, 3); break }
///         Status::NeedMore => continue,
///         other => panic!("{other:?}"),
///     }
/// }
/// d.release();
/// assert_eq!(d.filled(0), Ok(Status::Eof));
/// ```
pub struct FrameDecoder<'buf> {
    buf: &'buf mut [u8],
    header: [u8; HEADER_LEN],
    state: DState,
    max_len: usize,
    discard_limit: usize,
}

impl<'buf> FrameDecoder<'buf> {
    /// A decoder whose frames are stored in `buf`; `max_len = buf.len()`.
    #[inline]
    pub fn new(buf: &'buf mut [u8]) -> Self {
        let max_len = buf.len();
        Self {
            buf,
            header: [0; HEADER_LEN],
            state: DState::Header { have: 0 },
            max_len,
            discard_limit: 0,
        }
    }

    /// Lowers the maximum accepted payload length (clamped to the buffer length).
    #[inline]
    #[must_use]
    pub fn with_max_len(mut self, max_len: usize) -> Self {
        self.max_len = max_len.min(self.buf.len());
        self
    }

    /// Oversized frames with length `<= limit` are read and dropped instead of failing.
    /// Bounds the work a peer can force per frame to `limit` bytes. Has no effect for
    /// lengths `<= max_len`. Needs a non-empty buffer to make progress; with an empty
    /// buffer such frames fail as [`DecodeError::Oversized`].
    #[inline]
    #[must_use]
    pub fn with_discard_limit(mut self, limit: usize) -> Self {
        self.discard_limit = limit;
        self
    }

    /// The maximum accepted payload length.
    #[inline]
    #[must_use]
    pub fn max_len(&self) -> usize {
        self.max_len
    }

    /// Where the next bytes must be read into. Never empty while decoding.
    ///
    /// # Errors
    ///
    /// The stored error if the decoder is poisoned; [`DecodeError::NotReleased`] while a
    /// frame is ready (call [`release`](Self::release) first).
    #[inline]
    pub fn spare(&mut self) -> Result<&mut [u8], DecodeError> {
        match self.state {
            DState::Header { have } => Ok(&mut self.header[have as usize..]),
            DState::Body { len, have } => Ok(&mut self.buf[have..len]),
            DState::Discard { remaining, .. } => {
                let n = remaining.min(self.buf.len());
                Ok(&mut self.buf[..n])
            }
            DState::Ready { .. } => Err(DecodeError::NotReleased),
            DState::Failed(e) => Err(e),
        }
    }

    /// Records that `n` bytes were read into the slice last returned by
    /// [`spare`](Self::spare). `n == 0` means end of stream.
    ///
    /// # Errors
    ///
    /// See [`DecodeError`]. Every error poisons the decoder.
    pub fn filled(&mut self, n: usize) -> Result<Status, DecodeError> {
        let result = self.step(n);
        if let Err(e) = result {
            self.state = DState::Failed(e);
        }
        result
    }

    fn step(&mut self, n: usize) -> Result<Status, DecodeError> {
        match self.state {
            DState::Failed(e) => Err(e),
            DState::Ready { .. } => Err(DecodeError::NotReleased),
            DState::Header { have } => {
                let have = have as usize;
                let available = HEADER_LEN - have;
                if n > available {
                    return Err(DecodeError::Overfill {
                        reported: n,
                        available,
                    });
                }
                if n == 0 {
                    return if have == 0 {
                        Ok(Status::Eof)
                    } else {
                        Err(DecodeError::UnexpectedEof {
                            got: have,
                            needed: HEADER_LEN,
                        })
                    };
                }
                let have = have + n;
                if have < HEADER_LEN {
                    self.state = DState::Header { have: have as u8 };
                    return Ok(Status::NeedMore);
                }
                let announced = u32::from_le_bytes(self.header);
                // On 16-bit targets a u32 may not fit usize: treat as oversized.
                let len = usize::try_from(announced).unwrap_or(usize::MAX);
                if len <= self.max_len {
                    if len == 0 {
                        self.state = DState::Ready { len: 0 };
                        return Ok(Status::Ready(0));
                    }
                    self.state = DState::Body { len, have: 0 };
                    Ok(Status::NeedMore)
                } else if len <= self.discard_limit && !self.buf.is_empty() {
                    self.state = DState::Discard {
                        len,
                        remaining: len,
                    };
                    Ok(Status::NeedMore)
                } else {
                    Err(DecodeError::Oversized {
                        len: announced,
                        max: self.max_len,
                    })
                }
            }
            DState::Body { len, have } => {
                let available = len - have;
                if n > available {
                    return Err(DecodeError::Overfill {
                        reported: n,
                        available,
                    });
                }
                if n == 0 {
                    return Err(DecodeError::UnexpectedEof {
                        got: have,
                        needed: len,
                    });
                }
                let have = have + n;
                if have == len {
                    self.state = DState::Ready { len };
                    Ok(Status::Ready(len))
                } else {
                    self.state = DState::Body { len, have };
                    Ok(Status::NeedMore)
                }
            }
            DState::Discard { len, remaining } => {
                let available = remaining.min(self.buf.len());
                if n > available {
                    return Err(DecodeError::Overfill {
                        reported: n,
                        available,
                    });
                }
                if n == 0 {
                    return Err(DecodeError::UnexpectedEof {
                        got: len - remaining,
                        needed: len,
                    });
                }
                let remaining = remaining - n;
                if remaining == 0 {
                    self.state = DState::Header { have: 0 };
                    Ok(Status::Discarded(len))
                } else {
                    self.state = DState::Discard { len, remaining };
                    Ok(Status::NeedMore)
                }
            }
        }
    }

    /// The completed frame's payload, if a frame is ready.
    #[inline]
    #[must_use]
    pub fn frame(&self) -> Option<&[u8]> {
        match self.state {
            DState::Ready { len } => Some(&self.buf[..len]),
            _ => None,
        }
    }

    /// Mutable access to the completed frame's payload (e.g. to decode in place).
    #[inline]
    #[must_use]
    pub fn frame_mut(&mut self) -> Option<&mut [u8]> {
        match self.state {
            DState::Ready { len } => Some(&mut self.buf[..len]),
            _ => None,
        }
    }

    /// Finishes with the ready frame and starts the next header. No effect otherwise.
    #[inline]
    pub fn release(&mut self) {
        if let DState::Ready { .. } = self.state {
            self.state = DState::Header { have: 0 };
        }
    }

    /// Whether the decoder is between frames (no partial header or payload held).
    #[inline]
    #[must_use]
    pub fn is_at_boundary(&self) -> bool {
        matches!(self.state, DState::Header { have: 0 })
    }

    /// Execution contract of decoding **one frame** with this decoder: the steps from the
    /// first header byte to [`Status::Ready`], [`Status::Discarded`] or an error.
    ///
    /// One step is one [`spare`](Self::spare) + [`filled`](Self::filled) pair (each O(1)).
    /// Every step accepts at least one byte, so a frame announcing `len` payload bytes takes
    /// at most `HEADER_LEN + len` steps (exactly that many when every read returns one byte,
    /// at least 1 or 2 when reads are large): `fixed = 4 ops`, `per byte = 1 op`, bounded by
    /// `max(max_len, discard_limit)` bytes — a frame above that is refused within the 4
    /// header steps. The decoder copies no payload bytes itself (the I/O layer reads them into
    /// [`spare`](Self::spare)), so bytes read/written are 0; its working set is the 4-byte
    /// header plus the caller's buffer. Access `Sequential`, sharing `Exclusive`
    /// (`&mut self`), progress `NoSynchronization`. The kernel work of the reads belongs to
    /// the I/O layer's envelope (`KernelEntries`), not to this one.
    ///
    /// ```
    /// use o1_contract::{Bound, Quantity::Ops, Unit};
    /// use o1_core::frame::FrameDecoder;
    /// let mut buf = [0u8; 1024];
    /// let c = FrameDecoder::new(&mut buf).frame_contract();
    /// assert_eq!(c.bound(), Bound::Capacity { unit: Unit::Byte, max: 1024 });
    /// assert_eq!(c.envelope(100).unwrap().get(Ops), 104);
    /// assert!(c.envelope(1025).is_err()); // this decoder never accepts such a frame
    /// ```
    #[must_use]
    pub fn frame_contract(&self) -> ExecutionContract {
        ExecutionContract {
            progress: Progress::NoSynchronization,
            policy: PolicyDesc {
                access: Some(Access::Sequential),
                reuse: None,
                sharing: Some(Sharing::Exclusive),
                write: None,
            },
            isolation: Isolation::NoClaim,
            cost: Cost {
                fixed: ResourceEnvelope::ZERO
                    .with(Quantity::Ops, HEADER_LEN as u64)
                    // Cannot overflow: a slice is at most `isize::MAX` bytes.
                    .with(
                        Quantity::WorkingSet,
                        HEADER_LEN as u64 + self.buf.len() as u64,
                    ),
                per_unit: ResourceEnvelope::ZERO.with(Quantity::Ops, 1),
                unit: Unit::Byte,
                max_units: Some(self.max_len.max(self.discard_limit) as u64),
                peak_growth: PeakGrowth::Fixed,
            },
        }
    }

    /// The error that poisoned the decoder, if any.
    #[inline]
    #[must_use]
    pub fn error(&self) -> Option<DecodeError> {
        match self.state {
            DState::Failed(e) => Some(e),
            _ => None,
        }
    }
}

impl fmt::Debug for FrameDecoder<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FrameDecoder")
            .field("state", &self.state)
            .field("max_len", &self.max_len)
            .field("discard_limit", &self.discard_limit)
            .field("buf_len", &self.buf.len())
            .finish()
    }
}

/// Why a frame could not be encoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodeError {
    /// The payload is longer than `u32::MAX` bytes.
    TooLong {
        /// Payload length.
        len: usize,
    },
    /// `advance(n)` reported more bytes than were pending (a bug in the I/O driver).
    Overadvance {
        /// Bytes reported.
        reported: usize,
        /// Bytes pending.
        pending: usize,
    },
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::TooLong { len } => write!(f, "payload of {len} bytes exceeds u32::MAX"),
            Self::Overadvance { reported, pending } => {
                write!(
                    f,
                    "{reported} bytes reported written but only {pending} pending"
                )
            }
        }
    }
}

impl core::error::Error for EncodeError {}

/// Incremental encoder of one frame. Borrows the payload; never copies it.
///
/// ```
/// use o1_core::frame::FrameEncoder;
/// let mut e = FrameEncoder::new(b"hi").unwrap();
/// let (h, p) = e.pending();
/// assert_eq!((h, p), (&[2u8, 0, 0, 0][..], &b"hi"[..]));
/// e.advance(5).unwrap(); // a short write ending inside the payload
/// assert_eq!(e.pending(), (&[][..], &b"i"[..]));
/// e.advance(1).unwrap();
/// assert!(e.is_done());
/// ```
#[derive(Debug, Clone)]
pub struct FrameEncoder<'a> {
    header: [u8; HEADER_LEN],
    payload: &'a [u8],
    /// Bytes of header + payload already written.
    written: usize,
}

impl<'a> FrameEncoder<'a> {
    /// An encoder for `payload`.
    ///
    /// # Errors
    ///
    /// [`EncodeError::TooLong`] if `payload.len() > u32::MAX`.
    #[inline]
    pub fn new(payload: &'a [u8]) -> Result<Self, EncodeError> {
        let len = u32::try_from(payload.len())
            .map_err(|_| EncodeError::TooLong { len: payload.len() })?;
        Ok(Self {
            header: len.to_le_bytes(),
            payload,
            written: 0,
        })
    }

    /// Unsent bytes as `(header_rest, payload_rest)`. Either may be empty.
    #[inline]
    #[must_use]
    pub fn pending(&self) -> (&[u8], &'a [u8]) {
        if self.written < HEADER_LEN {
            (&self.header[self.written..], self.payload)
        } else {
            (&[], &self.payload[self.written - HEADER_LEN..])
        }
    }

    /// Number of unsent bytes.
    #[inline]
    #[must_use]
    pub fn remaining(&self) -> usize {
        HEADER_LEN + self.payload.len() - self.written
    }

    /// Records that `n` of the pending bytes were written.
    ///
    /// # Errors
    ///
    /// [`EncodeError::Overadvance`] if `n > remaining()`; the encoder is unchanged.
    #[inline]
    pub fn advance(&mut self, n: usize) -> Result<(), EncodeError> {
        let pending = self.remaining();
        if n > pending {
            return Err(EncodeError::Overadvance {
                reported: n,
                pending,
            });
        }
        self.written += n;
        Ok(())
    }

    /// Whether the whole frame has been written.
    #[inline]
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.remaining() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed_all(
        d: &mut FrameDecoder<'_>,
        mut input: &[u8],
        chunk: usize,
    ) -> Result<Status, DecodeError> {
        loop {
            let spare = d.spare()?;
            let n = spare.len().min(input.len()).min(chunk);
            spare[..n].copy_from_slice(&input[..n]);
            input = &input[n..];
            match d.filled(n)? {
                Status::NeedMore => continue,
                s => return Ok(s),
            }
        }
    }

    #[test]
    fn eof_clean_and_mid_header_and_mid_body() {
        let mut buf = [0u8; 8];
        let mut d = FrameDecoder::new(&mut buf);
        assert_eq!(d.filled(0), Ok(Status::Eof));

        let mut d = FrameDecoder::new(&mut buf);
        d.spare().unwrap()[..2].copy_from_slice(&[1, 0]);
        assert_eq!(d.filled(2), Ok(Status::NeedMore));
        assert_eq!(
            d.filled(0),
            Err(DecodeError::UnexpectedEof { got: 2, needed: 4 })
        );
        assert_eq!(
            d.spare().err(),
            Some(DecodeError::UnexpectedEof { got: 2, needed: 4 })
        );

        let mut d = FrameDecoder::new(&mut buf);
        assert_eq!(
            feed_all(&mut d, &[5, 0, 0, 0, 1, 2], 64),
            Err(DecodeError::UnexpectedEof { got: 2, needed: 5 })
        );
    }

    #[test]
    fn oversized_rejected_or_discarded() {
        let mut buf = [0u8; 4];
        let mut d = FrameDecoder::new(&mut buf);
        assert_eq!(
            feed_all(&mut d, &[5, 0, 0, 0], 64),
            Err(DecodeError::Oversized { len: 5, max: 4 })
        );
        assert!(d.error().is_some());

        let mut d = FrameDecoder::new(&mut buf).with_discard_limit(10);
        let wire = [6, 0, 0, 0, 9, 9, 9, 9, 9, 9, 2, 0, 0, 0, 7, 8];
        let mut rest = &wire[..];
        // Discard is reported, then the next frame decodes normally.
        let mut seen = [None, None];
        for slot in &mut seen {
            loop {
                let spare = d.spare().unwrap();
                let n = spare.len().min(rest.len());
                spare[..n].copy_from_slice(&rest[..n]);
                rest = &rest[n..];
                match d.filled(n).unwrap() {
                    Status::NeedMore => {}
                    s => {
                        *slot = Some(s);
                        break;
                    }
                }
            }
        }
        assert_eq!(seen, [Some(Status::Discarded(6)), Some(Status::Ready(2))]);
        assert_eq!(d.frame(), Some(&[7u8, 8][..]));

        let mut d = FrameDecoder::new(&mut buf).with_max_len(1);
        assert_eq!(d.max_len(), 1);
        assert_eq!(
            feed_all(&mut d, &[2, 0, 0, 0], 64),
            Err(DecodeError::Oversized { len: 2, max: 1 })
        );
    }

    #[test]
    fn zero_length_frame_and_release_protocol() {
        let mut buf = [0u8; 4];
        let mut d = FrameDecoder::new(&mut buf);
        assert_eq!(feed_all(&mut d, &[0, 0, 0, 0], 1), Ok(Status::Ready(0)));
        assert_eq!(d.frame(), Some(&[][..]));
        assert_eq!(d.spare().err(), Some(DecodeError::NotReleased));
        d.release();
        assert!(d.is_at_boundary());
        assert_eq!(
            d.filled(5),
            Err(DecodeError::Overfill {
                reported: 5,
                available: 4
            })
        );
    }

    #[test]
    fn encoder_partial_writes_and_overadvance() {
        let mut e = FrameEncoder::new(b"xyz").unwrap();
        assert_eq!(e.remaining(), 7);
        e.advance(1).unwrap();
        assert_eq!(e.pending(), (&[0u8, 0, 0][..], &b"xyz"[..]));
        assert_eq!(
            e.advance(9),
            Err(EncodeError::Overadvance {
                reported: 9,
                pending: 6
            })
        );
        e.advance(3).unwrap();
        assert_eq!(e.pending(), (&[][..], &b"xyz"[..]));
        e.advance(3).unwrap();
        assert!(e.is_done());
        assert_eq!(e.pending(), (&[][..], &[][..]));
    }
}
