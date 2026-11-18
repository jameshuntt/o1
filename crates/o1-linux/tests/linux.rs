//! Linux runtime tests for futex and framed Unix-socket I/O.
//!
//! Compiled only on Linux. They have NOT been run on the development machine (macOS, no
//! Linux runtime); `.github/workflows/ci.yml` runs them on x86_64 and arm64 Linux runners.
#![cfg(target_os = "linux")]

use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use o1_core::frame::{DecodeError, FrameDecoder, FrameEncoder};
use o1_linux::errno::Errno;
use o1_linux::fd::BorrowedFd;
use o1_linux::futex::{self, WaitOutcome};
use o1_linux::io;
use o1_linux::uds::{self, RecvOutcome, SendOutcome, UdsError};

fn fd(s: &UnixStream) -> BorrowedFd<'_> {
    // SAFETY: [unsafe:linux-test.borrow] the returned value borrows `s`, so the descriptor stays
    // open and owned by this test for the borrow's lifetime.
    unsafe { BorrowedFd::borrow_raw(s.as_raw_fd()) }
}

#[test]
fn futex_value_mismatch_returns_immediately() {
    let w = AtomicU32::new(1);
    assert_eq!(futex::wait(&w, 2, None), Ok(WaitOutcome::ValueMismatch));
}

#[test]
fn futex_timeout() {
    let w = AtomicU32::new(0);
    let t0 = Instant::now();
    let r = futex::wait(&w, 0, Some(Duration::from_millis(20)));
    assert!(
        matches!(r, Ok(WaitOutcome::TimedOut) | Ok(WaitOutcome::Woken)),
        "{r:?}"
    );
    if r == Ok(WaitOutcome::TimedOut) {
        assert!(t0.elapsed() >= Duration::from_millis(15));
    }
}

#[test]
fn futex_wake_with_no_waiters_wakes_zero() {
    let w = AtomicU32::new(0);
    assert_eq!(futex::wake(&w, u32::MAX), Ok(0));
}

#[test]
fn futex_cross_thread_wake() {
    let w = Arc::new(AtomicU32::new(0));
    let w2 = Arc::clone(&w);
    let waiter = thread::spawn(move || {
        futex::wait_while_equal(&w2, 0).unwrap();
        w2.load(Ordering::Acquire)
    });
    thread::sleep(Duration::from_millis(20));
    w.store(7, Ordering::Release);
    futex::wake(&w, 1).unwrap();
    assert_eq!(waiter.join().unwrap(), 7);
}

#[test]
fn futex_spurious_wake_loop_keeps_waiting() {
    // Wakes without changing the word must not release wait_while_equal.
    let w = Arc::new(AtomicU32::new(0));
    let w2 = Arc::clone(&w);
    let waiter = thread::spawn(move || futex::wait_while_equal(&w2, 0));
    for _ in 0..5 {
        thread::sleep(Duration::from_millis(5));
        futex::wake(&w, 1).unwrap();
    }
    assert!(!waiter.is_finished());
    w.store(1, Ordering::Release);
    futex::wake(&w, 1).unwrap();
    assert_eq!(waiter.join().unwrap(), Ok(()));
}

#[test]
fn uds_roundtrip_many_frames() {
    let (a, b) = UnixStream::pair().unwrap();
    let payloads: Vec<Vec<u8>> = (0..50u8).map(|i| vec![i; i as usize * 37]).collect();
    let expected = payloads.clone();
    let sender = thread::spawn(move || {
        for p in &payloads {
            let mut e = FrameEncoder::new(p).unwrap();
            assert_eq!(uds::send_frame(fd(&a), &mut e), Ok(SendOutcome::Done));
        }
    });
    let mut buf = vec![0u8; 4096];
    let mut d = FrameDecoder::new(&mut buf);
    for p in &expected {
        assert_eq!(
            uds::recv_frame(fd(&b), &mut d),
            Ok(RecvOutcome::Frame(p.len()))
        );
        assert_eq!(d.frame().unwrap(), &p[..]);
        d.release();
    }
    sender.join().unwrap();
    assert_eq!(uds::recv_frame(fd(&b), &mut d), Ok(RecvOutcome::Eof));
}

#[test]
fn uds_nonblocking_partial_progress_is_kept() {
    let (a, b) = UnixStream::pair().unwrap();
    b.set_nonblocking(true).unwrap();
    let mut buf = [0u8; 16];
    let mut d = FrameDecoder::new(&mut buf);
    assert_eq!(uds::recv_frame(fd(&b), &mut d), Ok(RecvOutcome::WouldBlock));
    // Header split across two writes, payload across two more.
    for chunk in [&[5u8, 0][..], &[0, 0, b'h', b'e'][..], &b"llo"[..]] {
        (&a).write_all(chunk).unwrap();
        let r = uds::recv_frame(fd(&b), &mut d).unwrap();
        if chunk.len() == 3 {
            assert_eq!(r, RecvOutcome::Frame(5));
        } else {
            assert_eq!(r, RecvOutcome::WouldBlock);
        }
    }
    assert_eq!(d.frame(), Some(&b"hello"[..]));
}

#[test]
fn uds_nonblocking_send_fills_then_resumes() {
    let (a, b) = UnixStream::pair().unwrap();
    a.set_nonblocking(true).unwrap();
    let big = vec![0xabu8; 4 << 20];
    let mut e = FrameEncoder::new(&big).unwrap();
    assert_eq!(uds::send_frame(fd(&a), &mut e), Ok(SendOutcome::WouldBlock));
    assert!(!e.is_done());
    let reader = thread::spawn(move || {
        let mut buf = vec![0u8; 4 << 20];
        let mut d = FrameDecoder::new(&mut buf);
        let r = uds::recv_frame(fd(&b), &mut d).unwrap();
        (r, d.frame().map(|f| f.iter().all(|&x| x == 0xab)))
    });
    loop {
        match uds::send_frame(fd(&a), &mut e) {
            Ok(SendOutcome::Done) => break,
            Ok(SendOutcome::WouldBlock) => thread::sleep(Duration::from_millis(1)),
            Err(err) => panic!("{err:?}"),
        }
    }
    assert_eq!(
        reader.join().unwrap(),
        (RecvOutcome::Frame(4 << 20), Some(true))
    );
}

#[test]
fn uds_eof_mid_frame_is_an_error() {
    let (a, b) = UnixStream::pair().unwrap();
    (&a).write_all(&[9, 0, 0, 0, 1, 2]).unwrap();
    drop(a);
    let mut buf = [0u8; 16];
    let mut d = FrameDecoder::new(&mut buf);
    assert_eq!(
        uds::recv_frame(fd(&b), &mut d),
        Err(UdsError::Decode(DecodeError::UnexpectedEof {
            got: 2,
            needed: 9
        }))
    );
}

#[test]
fn uds_oversized_rejected() {
    let (a, b) = UnixStream::pair().unwrap();
    (&a).write_all(&u32::MAX.to_le_bytes()).unwrap();
    let mut buf = [0u8; 16];
    let mut d = FrameDecoder::new(&mut buf);
    assert_eq!(
        uds::recv_frame(fd(&b), &mut d),
        Err(UdsError::Decode(DecodeError::Oversized {
            len: u32::MAX,
            max: 16
        }))
    );
}

#[test]
fn uds_send_to_closed_peer_is_epipe_not_sigpipe() {
    let (a, b) = UnixStream::pair().unwrap();
    drop(b);
    let mut e = FrameEncoder::new(b"x").unwrap();
    let r = uds::send_frame(fd(&a), &mut e);
    assert!(
        matches!(
            r,
            Err(UdsError::Os(Errno::EPIPE)) | Err(UdsError::Os(Errno::ECONNRESET))
        ),
        "{r:?}"
    );
}

#[test]
fn io_read_write_and_ebadf() {
    let (a, b) = UnixStream::pair().unwrap();
    assert_eq!(io::write(fd(&a), b"abc"), Ok(3));
    let mut buf = [0u8; 8];
    assert_eq!(io::read(fd(&b), &mut buf), Ok(3));
    assert_eq!(&buf[..3], b"abc");
    // SAFETY: [unsafe:linux-test.bad-fd] -2 is never an open descriptor; the kernel rejects it
    // with EBADF without touching any memory. Violates the "open descriptor" precondition on
    // purpose to observe the error path; no I/O on another owner's descriptor can happen.
    let bad = unsafe { BorrowedFd::borrow_raw(-2) };
    assert_eq!(io::read(bad, &mut buf), Err(Errno::EBADF));
}
