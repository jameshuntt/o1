//! A fixed-capacity, allocation-free, wait-free single-producer single-consumer ring.
//!
//! ```
//! use o1_sync::spsc::Ring;
//!
//! let mut ring = Ring::<u64, 8>::new();
//! let (mut tx, mut rx) = ring.split();
//! std::thread::scope(|s| {
//!     s.spawn(move || {
//!         for i in 0..1000 {
//!             let mut v = i;
//!             while let Err(back) = tx.push(v) { v = back; std::hint::spin_loop(); }
//!         }
//!     });
//!     let mut expect = 0;
//!     while expect < 1000 {
//!         if let Some(v) = rx.pop() { assert_eq!(v, expect); expect += 1; }
//!     }
//! });
//! ```
//!
//! # One producer, one consumer — enforced by the borrow checker
//!
//! [`Ring::split`] takes `&mut self`, so while a [`Producer`]/[`Consumer`] pair exists no
//! second pair can be made, and neither handle is `Clone`:
//!
//! ```compile_fail,E0499
//! let mut ring = o1_sync::spsc::Ring::<u8, 4>::new();
//! let (p1, _c1) = ring.split();
//! let (p2, _c2) = ring.split(); // second producer: rejected
//! drop((p1, p2));
//! ```
//!
//! A ring whose element type is not `Send` cannot be shared between threads:
//!
//! ```compile_fail,E0277
//! fn needs_sync<T: Sync>() {}
//! needs_sync::<o1_sync::spsc::Ring<std::rc::Rc<u8>, 4>>();
//! ```
//!
//! # Guarantees, per operation
//!
//! | operation | progress | work | allocation |
//! |---|---|---|---|
//! | [`Producer::push`] | wait-free (no loop, no CAS) | worst-case O(1): ≤ 1 acquire load, 1 slot write, 1 release store | none |
//! | [`Consumer::pop`] | wait-free | worst-case O(1): ≤ 1 acquire load, 1 slot read, 1 release store | none |
//! | [`Producer::free_slots`], [`Consumer::available`] | wait-free | O(1), 1 acquire load | none |
//! | [`Ring::split`] | — (needs `&mut`) | O(1) | none |
//! | `Drop for Ring` | — | O(items left) | none |
//!
//! "Wait-free" here means each call finishes in a bounded number of its own steps whatever
//! the other thread does. It is **not** a wall-clock bound: cache-coherence traffic on the
//! two index lines and preemption by the OS are outside any guarantee. A full ring makes
//! `push` return `Err(value)`; an empty ring makes `pop` return `None`. Waiting for space or
//! data (spinning, parking) is the caller's policy.
//!
//! # Memory orderings
//!
//! * `push`: write the slot, then `tail.store(t + 1, Release)`. The release publishes the
//!   slot write to the consumer.
//! * `pop` when its cached `tail` says empty: `tail.load(Acquire)` — synchronises with that
//!   release, so the slot read sees the written value.
//! * `pop`: read (move out of) the slot, then `head.store(h + 1, Release)`.
//! * `push` when its cached `head` says full: `head.load(Acquire)` — synchronises with that
//!   release, so the consumer's move-out happens-before the producer overwrites the slot.
//! * `split` and `Drop` read the counters through `&mut` (no concurrent access exists).
//!
//! No ordering is stronger than this, and none is `SeqCst`: the two counters are each
//! written by one thread, and no decision depends on a total order over both.
//!
//! # Counters
//!
//! `head` and `tail` are free-running `usize` counters that wrap at `usize::MAX`. `N` is a
//! power of two (compile-time check), so `N` divides `2^usize::BITS`: `tail - head`
//! (wrapping) is the length and `counter & (N - 1)` the slot, on both sides of the wrap.
//!
//! ```compile_fail,E0080
//! let _ = o1_sync::spsc::Ring::<u8, 6>::new(); // not a power of two
//! ```
//!
//! # Layout
//!
//! `head` and `tail` each sit in a [`CachePadded`] so the consumer's
//! writes to `head` and the producer's writes to `tail` do not contend for one cache line.
//! Each handle also keeps a private copy of the other side's counter and reloads it only
//! when the ring looks full (producer) or empty (consumer), so in the common case each side
//! touches only its own line plus the slot.
//!
//! # Without `alloc`
//!
//! Handles borrow the ring. Use `std::thread::scope`, a `static`, or memory the caller owns
//! for sharing across threads; there are no `Arc`-owned handles.
//!
//! # Execution contracts
//!
//! [`Ring::PUSH_CONTRACT`] and [`Ring::POP_CONTRACT`] state, per call, the worst case of what
//! the code above does (`o1_contract`; no weights, counts only):
//!
//! | quantity | `push` | `pop` | where it comes from |
//! |---|---|---|---|
//! | ops | 1 | 1 | no loop, no CAS |
//! | bytes written / read | `size_of::<T>()` written | `size_of::<T>()` read | the slot write / read |
//! | atomic loads | ≤ 1 | ≤ 1 | refresh of the other side's counter, only when the ring looks full / empty |
//! | atomic stores | 1 | 1 | publication of `tail` / `head` |
//! | shared reads | ≤ 1 (`head`) | ≤ 2 (`tail`, the slot) | memory the other thread writes |
//! | shared writes | 2 (the slot, `tail`) | 1 (`head`) | memory the other thread reads |
//! | working set | `size_of::<Ring<T, N>>()` | same | the ring |
//!
//! Progress `WaitFree`; sharing `Handoff`; access `Sequential` (slots in ring order);
//! isolation `Padded` with the `CachePadded` unit of the compiled target (128 bytes on
//! x86_64/aarch64): `head` and `tail` each sit alone in such a unit. The **slots are not
//! padded**: adjacent slots can share a cache line, so a producer writing slot *k* and a
//! consumer reading slot *k − 1* can still contend for one line. A full `push` or an empty
//! `pop` does strictly less (one load, nothing else). The unit tests count every atomic access
//! and slot access through the crate's switch module and compare them with these numbers.
//!
//! ```
//! use o1_contract::{Bound, Quantity::*};
//! use o1_sync::spsc::Ring;
//! type R = Ring<u64, 1024>;
//! assert_eq!(R::PUSH_CONTRACT.bound(), Bound::Constant);
//! let million = R::PUSH_CONTRACT.envelope(0).unwrap().repeat(1_000_000).unwrap();
//! assert_eq!(million.get(SharedWrites), 2_000_000);
//! assert_eq!(million.get(BytesWritten), 8_000_000);
//! ```

use core::fmt;
use core::mem::MaybeUninit;

use o1_contract::{
    Access, Cost, ExecutionContract, Isolation, PolicyDesc, Progress, Quantity, ResourceEnvelope,
    Sharing,
};

use crate::CachePadded;
use crate::sync::{AtomicUsize, Ordering, UnsafeCell, load_mut};

/// The shared ring. See the [module docs](self).
pub struct Ring<T, const N: usize> {
    /// Consumer's counter: next slot to read. Written only by the consumer.
    head: CachePadded<AtomicUsize>,
    /// Producer's counter: next slot to write. Written only by the producer.
    tail: CachePadded<AtomicUsize>,
    /// Slots `head..tail` (mod N) are initialised; the rest are not.
    slots: [UnsafeCell<MaybeUninit<T>>; N],
}

// SAFETY: [unsafe:spsc.sync] `&Ring` only gives access to slots through a `Producer` or a
// `Consumer`, and `split(&mut self)` makes at most one of each exist. The producer only
// writes slots outside `head..tail`, the consumer only reads slots inside it, and the
// Release/Acquire pairs on `tail`/`head` order each slot write before its read and each read
// before the next write. Values of `T` move from the producer's thread to the consumer's
// thread, hence `T: Send`; no `&T` is ever shared, so `T: Sync` is not needed.
unsafe impl<T: Send, const N: usize> Sync for Ring<T, N> {}

impl<T, const N: usize> Ring<T, N> {
    const VALID: () = assert!(
        N.is_power_of_two(),
        "spsc::Ring: N must be a non-zero power of two"
    );
    const MASK: usize = {
        let () = Self::VALID;
        N - 1
    };

    /// An empty ring.
    #[cfg(not(loom))]
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        let () = Self::VALID;
        Self {
            head: CachePadded::new(AtomicUsize::new(0)),
            tail: CachePadded::new(AtomicUsize::new(0)),
            slots: [const { UnsafeCell::new(MaybeUninit::uninit()) }; N],
        }
    }

    /// An empty ring (loom build: loom's atomics are not `const`).
    #[cfg(loom)]
    #[must_use]
    pub fn new() -> Self {
        Self::with_start(0)
    }

    /// An empty ring whose counters start at `start` (test hook for counter wraparound).
    #[cfg(any(test, loom))]
    #[doc(hidden)]
    #[must_use]
    pub fn with_start(start: usize) -> Self {
        let () = Self::VALID;
        Self {
            head: CachePadded::new(AtomicUsize::new(start)),
            tail: CachePadded::new(AtomicUsize::new(start)),
            slots: core::array::from_fn(|_| UnsafeCell::new(MaybeUninit::uninit())),
        }
    }

    /// Capacity, `N`. The ring holds up to `N` items (not `N - 1`).
    #[inline]
    #[must_use]
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Splits into the one producer and the one consumer. The `&mut` borrow lasts as long as
    /// either handle, so no second pair can exist meanwhile. Items left in the ring when the
    /// handles are dropped stay there; a later `split` continues from them.
    #[inline]
    pub fn split(&mut self) -> (Producer<'_, T, N>, Consumer<'_, T, N>) {
        let head = load_mut(&mut self.head);
        let tail = load_mut(&mut self.tail);
        let ring: &Self = self;
        (
            Producer {
                ring,
                tail,
                cached_head: head,
            },
            Consumer {
                ring,
                head,
                cached_tail: tail,
            },
        )
    }
}

impl<T, const N: usize> Ring<T, N> {
    /// Contract of [`Producer::push`] (worst case; see the [module docs](self)).
    pub const PUSH_CONTRACT: ExecutionContract = Self::handoff_contract(0, size_of::<T>(), 1, 2);

    /// Contract of [`Consumer::pop`] (worst case; see the [module docs](self)).
    pub const POP_CONTRACT: ExecutionContract = Self::handoff_contract(size_of::<T>(), 0, 2, 1);

    const fn handoff_contract(
        read: usize,
        written: usize,
        shared_reads: u64,
        shared_writes: u64,
    ) -> ExecutionContract {
        ExecutionContract {
            progress: Progress::WaitFree,
            policy: PolicyDesc {
                access: Some(Access::Sequential),
                reuse: None,
                sharing: Some(Sharing::Handoff),
                write: None,
            },
            isolation: Isolation::Padded {
                unit_bytes: size_of::<CachePadded<AtomicUsize>>() as u64,
            },
            cost: Cost::constant(
                ResourceEnvelope::ZERO
                    .with(Quantity::Ops, 1)
                    .with(Quantity::BytesRead, read as u64)
                    .with(Quantity::BytesWritten, written as u64)
                    .with(Quantity::AtomicLoads, 1)
                    .with(Quantity::AtomicStores, 1)
                    .with(Quantity::SharedReads, shared_reads)
                    .with(Quantity::SharedWrites, shared_writes)
                    .with(Quantity::WorkingSet, size_of::<Self>() as u64),
            ),
        }
        .assert_valid()
    }
}

impl<T, const N: usize> Default for Ring<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const N: usize> Drop for Ring<T, N> {
    fn drop(&mut self) {
        let head = load_mut(&mut self.head);
        let tail = load_mut(&mut self.tail);
        let mut i = head;
        while i != tail {
            self.slots[i & Self::MASK].with_mut(|p| {
                // SAFETY: [unsafe:spsc.drop] `&mut self` means no handle is alive; slots
                // `head..tail` are initialised and each is visited once. If a destructor
                // panics the remaining items are leaked, never dropped twice (the ring is
                // being destroyed, so nothing reads its counters again).
                // `MaybeUninit<T>` is `repr(transparent)` over `T`, so the cast pointer is valid for `T`.
                unsafe { p.cast::<T>().drop_in_place() }
            });
            i = i.wrapping_add(1);
        }
    }
}

impl<T, const N: usize> fmt::Debug for Ring<T, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Ring")
            .field("capacity", &N)
            .field("head", &self.head.load(Ordering::Relaxed))
            .field("tail", &self.tail.load(Ordering::Relaxed))
            .finish()
    }
}

/// The producing half of a [`Ring`]. `Send` when `T: Send`; not `Clone`.
pub struct Producer<'a, T, const N: usize> {
    ring: &'a Ring<T, N>,
    /// Private copy of `ring.tail` (this handle is its only writer).
    tail: usize,
    /// Last value of `ring.head` seen; `head` only grows, so this is a lower bound.
    cached_head: usize,
}

impl<T, const N: usize> Producer<'_, T, N> {
    /// Appends `value`. Wait-free; worst-case O(1).
    ///
    /// # Errors
    ///
    /// `Err(value)` if the ring is full; nothing is written.
    #[inline]
    pub fn push(&mut self, value: T) -> Result<(), T> {
        if self.tail.wrapping_sub(self.cached_head) == N {
            // Looks full: refresh. Acquire pairs with the consumer's Release store of `head`,
            // so its move-out of the slot we are about to reuse happened-before our write.
            self.cached_head = self.ring.head.load(Ordering::Acquire);
            if self.tail.wrapping_sub(self.cached_head) == N {
                return Err(value);
            }
        }
        self.ring.slots[self.tail & Ring::<T, N>::MASK].with_mut(|p| {
            // SAFETY: [unsafe:spsc.push-write] `tail - head < N`, so this slot is outside
            // `head..tail`: uninitialised (or already moved out by the consumer, ordered by
            // the Acquire above or an earlier one) and not accessed by the consumer until we
            // publish it below. Only this producer writes slots.
            unsafe { p.write(MaybeUninit::new(value)) }
        });
        self.tail = self.tail.wrapping_add(1);
        // Release publishes the slot write to the consumer's Acquire load of `tail`.
        self.ring.tail.store(self.tail, Ordering::Release);
        Ok(())
    }

    /// Free slots right now (refreshes the consumer's counter with one Acquire load). The
    /// true number can only grow until the next `push`.
    #[inline]
    pub fn free_slots(&mut self) -> usize {
        self.cached_head = self.ring.head.load(Ordering::Acquire);
        N - self.tail.wrapping_sub(self.cached_head)
    }

    /// Capacity, `N`.
    #[inline]
    #[must_use]
    pub const fn capacity(&self) -> usize {
        N
    }
}

impl<T, const N: usize> fmt::Debug for Producer<'_, T, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Producer")
            .field("tail", &self.tail)
            .field("cached_head", &self.cached_head)
            .finish()
    }
}

/// The consuming half of a [`Ring`]. `Send` when `T: Send`; not `Clone`.
pub struct Consumer<'a, T, const N: usize> {
    ring: &'a Ring<T, N>,
    /// Private copy of `ring.head` (this handle is its only writer).
    head: usize,
    /// Last value of `ring.tail` seen; `tail` only grows, so this is a lower bound.
    cached_tail: usize,
}

impl<T, const N: usize> Consumer<'_, T, N> {
    /// Removes the oldest item. Wait-free; worst-case O(1). `None` if empty.
    #[inline]
    pub fn pop(&mut self) -> Option<T> {
        if self.head == self.cached_tail {
            // Looks empty: refresh. Acquire pairs with the producer's Release store of
            // `tail`, so the slot write happened-before our read.
            self.cached_tail = self.ring.tail.load(Ordering::Acquire);
            if self.head == self.cached_tail {
                return None;
            }
        }
        let value = self.ring.slots[self.head & Ring::<T, N>::MASK].with_mut(|p| {
            // SAFETY: [unsafe:spsc.pop-read] `head != tail` (as last observed with Acquire),
            // so this slot is in `head..tail`: initialised, published, and not written by the
            // producer until we release it below. Advancing `head` marks it moved-out, so it
            // is read exactly once. Only this consumer reads slots.
            // `MaybeUninit<T>` is `repr(transparent)` over `T`, so the cast pointer is valid for `T`.
            unsafe { p.cast::<T>().read() }
        });
        self.head = self.head.wrapping_add(1);
        // Release orders the move-out before the producer's Acquire load of `head`.
        self.ring.head.store(self.head, Ordering::Release);
        Some(value)
    }

    /// Items available right now (refreshes the producer's counter with one Acquire load).
    /// The true number can only grow until the next `pop`.
    #[inline]
    pub fn available(&mut self) -> usize {
        self.cached_tail = self.ring.tail.load(Ordering::Acquire);
        self.cached_tail.wrapping_sub(self.head)
    }

    /// Capacity, `N`.
    #[inline]
    #[must_use]
    pub const fn capacity(&self) -> usize {
        N
    }
}

impl<T, const N: usize> fmt::Debug for Consumer<'_, T, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Consumer")
            .field("head", &self.head)
            .field("cached_tail", &self.cached_tail)
            .finish()
    }
}

#[cfg(all(test, not(loom)))]
mod tests {
    use super::*;
    use core::cell::Cell;
    use std::vec::Vec;

    #[test]
    fn full_empty_transitions() {
        let mut ring = Ring::<u32, 4>::new();
        let (mut p, mut c) = ring.split();
        assert_eq!(c.pop(), None);
        for i in 0..4 {
            p.push(i).unwrap();
        }
        assert_eq!(p.push(9), Err(9));
        assert_eq!(p.free_slots(), 0);
        assert_eq!(c.available(), 4);
        assert_eq!(c.pop(), Some(0));
        assert_eq!(p.free_slots(), 1);
        p.push(4).unwrap();
        assert_eq!(
            (1..=4).map(|_| c.pop().unwrap()).collect::<Vec<_>>(),
            [1, 2, 3, 4]
        );
        assert_eq!(c.pop(), None);
    }

    #[test]
    fn counter_wraparound_repeated() {
        let mut ring = Ring::<u64, 8>::with_start(usize::MAX - 5);
        let (mut p, mut c) = ring.split();
        let mut next_in = 0u64;
        let mut next_out = 0u64;
        for round in 0..100 {
            let k = (round % 8) + 1;
            for _ in 0..k {
                p.push(next_in).unwrap();
                next_in += 1;
            }
            for _ in 0..k {
                assert_eq!(c.pop(), Some(next_out));
                next_out += 1;
            }
        }
        assert_eq!(c.pop(), None);
    }

    #[test]
    fn resplit_keeps_items() {
        let mut ring = Ring::<u8, 2>::new();
        {
            let (mut p, _c) = ring.split();
            p.push(1).unwrap();
        }
        let (_p, mut c) = ring.split();
        assert_eq!(c.pop(), Some(1));
    }

    struct Counted<'a>(&'a Cell<usize>);
    impl Drop for Counted<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    #[test]
    fn drop_drops_unconsumed_items_across_wrap() {
        let drops = Cell::new(0);
        {
            let mut ring = Ring::<Counted<'_>, 4>::with_start(usize::MAX - 1);
            let (mut p, mut c) = ring.split();
            for _ in 0..4 {
                assert!(p.push(Counted(&drops)).is_ok());
            }
            drop(c.pop());
            assert_eq!(drops.get(), 1);
        }
        assert_eq!(drops.get(), 4);
    }

    #[test]
    fn zst_items() {
        let mut ring = Ring::<(), 2>::new();
        let (mut p, mut c) = ring.split();
        p.push(()).unwrap();
        p.push(()).unwrap();
        assert_eq!(p.push(()), Err(()));
        assert_eq!(c.pop(), Some(()));
        assert_eq!(c.available(), 1);
    }

    #[test]
    fn two_threads_in_order() {
        let mut ring = Ring::<u32, 16>::new();
        let (mut p, mut c) = ring.split();
        let n = if cfg!(miri) { 200 } else { 100_000 };
        std::thread::scope(|s| {
            s.spawn(move || {
                for i in 0..n {
                    let mut v = i;
                    while let Err(back) = p.push(v) {
                        v = back;
                        std::thread::yield_now();
                    }
                }
            });
            let mut expect = 0;
            while expect < n {
                match c.pop() {
                    Some(v) => {
                        assert_eq!(v, expect);
                        expect += 1;
                    }
                    None => std::thread::yield_now(),
                }
            }
        });
    }

    // ---- contracts against counted structural behaviour ----

    use crate::sync::counting;
    use o1_contract::Quantity::*;

    fn without_peak(e: ResourceEnvelope) -> ResourceEnvelope {
        e.with(WorkingSet, 0)
    }

    fn check_counts<T: Default + core::fmt::Debug, const N: usize>() {
        let size = size_of::<T>() as u64;
        let push_decl = Ring::<T, N>::PUSH_CONTRACT.envelope(0).unwrap();
        let pop_decl = Ring::<T, N>::POP_CONTRACT.envelope(0).unwrap();
        let mut ring = Ring::<T, N>::new();
        let (mut p, mut c) = ring.split();
        let _ = counting::take();

        // Fill: the cached head is current, so no load.
        for _ in 0..N {
            assert!(p.push(T::default()).is_ok());
            let k = counting::take();
            assert_eq!((k.loads, k.stores, k.cell_accesses), (0, 1, 1));
            assert!(k.push_envelope(size).within(&push_decl));
        }
        // Full: one load, nothing written.
        assert!(p.push(T::default()).is_err());
        let k = counting::take();
        assert_eq!((k.loads, k.stores, k.cell_accesses), (1, 0, 0));
        assert!(k.push_envelope(size).within(&push_decl));

        // First pop refreshes the producer's tail: the worst case, equal to the contract.
        assert!(c.pop().is_some());
        let k = counting::take();
        assert_eq!((k.loads, k.stores, k.cell_accesses), (1, 1, 1));
        assert_eq!(k.pop_envelope(size), without_peak(pop_decl));
        // Next pops use the cached tail.
        for _ in 1..N {
            assert!(c.pop().is_some());
            let k = counting::take();
            assert_eq!((k.loads, k.stores, k.cell_accesses), (0, 1, 1));
            assert!(k.pop_envelope(size).within(&pop_decl));
        }
        // Empty: one load, nothing read.
        assert!(c.pop().is_none());
        let k = counting::take();
        assert_eq!((k.loads, k.stores, k.cell_accesses), (1, 0, 0));
        assert!(k.pop_envelope(size).within(&pop_decl));

        // Push with a stale cached head: refresh, then write — the worst case exactly.
        assert!(p.push(T::default()).is_ok());
        let k = counting::take();
        assert_eq!((k.loads, k.stores, k.cell_accesses), (1, 1, 1));
        assert_eq!(k.push_envelope(size), without_peak(push_decl));
    }

    #[test]
    fn contracts_match_counted_accesses() {
        check_counts::<u64, 4>();
        check_counts::<[u64; 5], 2>();
        check_counts::<(), 1>();
        check_counts::<u8, 8>();
    }

    #[test]
    fn contract_shape_and_isolation() {
        use o1_contract::{Bound, Isolation, Progress, Sharing};
        type R = Ring<u64, 4>;
        for c in [R::PUSH_CONTRACT, R::POP_CONTRACT] {
            assert_eq!(c.bound(), Bound::Constant);
            assert_eq!(c.progress, Progress::WaitFree);
            assert_eq!(c.policy.sharing, Some(Sharing::Handoff));
            assert_eq!(
                c.envelope(0).unwrap().get(WorkingSet),
                size_of::<R>() as u64
            );
            let Isolation::Padded { unit_bytes } = c.isolation else {
                panic!("SPSC control words are padded");
            };
            // Each counter occupies a whole unit by itself: unit = size = alignment of the
            // padded counter, and both counters start on a unit boundary.
            let unit = unit_bytes as usize;
            assert_eq!(size_of::<CachePadded<AtomicUsize>>(), unit);
            assert_eq!(core::mem::align_of::<CachePadded<AtomicUsize>>(), unit);
            assert_eq!(core::mem::offset_of!(R, head) % unit, 0);
            assert_eq!(core::mem::offset_of!(R, tail) % unit, 0);
            assert_ne!(
                core::mem::offset_of!(R, head),
                core::mem::offset_of!(R, tail)
            );
            if cfg!(any(target_arch = "x86_64", target_arch = "aarch64")) {
                assert_eq!(unit, 128);
            }
        }
    }

    fn is_send<T: Send>() {}
    fn is_sync<T: Sync>() {}

    #[test]
    fn auto_traits() {
        is_send::<Ring<u8, 4>>();
        is_sync::<Ring<u8, 4>>();
        is_send::<Producer<'static, u8, 4>>();
        is_send::<Consumer<'static, u8, 4>>();
        // A non-Sync but Send T is fine: values move, never shared.
        is_sync::<Ring<Cell<u8>, 4>>();
        is_send::<Producer<'static, Cell<u8>, 4>>();
    }
}

#[cfg(all(test, loom))]
mod loom_models {
    //! Run with `RUSTFLAGS="--cfg loom" cargo test -p o1-sync --release --lib`.
    use super::*;
    use loom::sync::Arc;
    use loom::sync::atomic::AtomicUsize as LAtomic;
    use loom::thread;
    use std::boxed::Box;

    /// A leaked ring, so the borrowed handles are `'static` for `loom::thread::spawn`.
    fn leaked<T, const N: usize>(start: usize) -> &'static mut Ring<T, N> {
        Box::leak(Box::new(Ring::with_start(start)))
    }

    fn transfer<const N: usize>(start: usize, items: u32) {
        loom::model(move || {
            let ring = leaked::<u32, N>(start);
            let (mut p, mut c) = ring.split();
            let producer = thread::spawn(move || {
                for i in 0..items {
                    let mut v = i;
                    while let Err(back) = p.push(v) {
                        v = back;
                        thread::yield_now();
                    }
                }
            });
            let mut expect = 0;
            while expect < items {
                match c.pop() {
                    Some(v) => {
                        assert_eq!(v, expect);
                        expect += 1;
                    }
                    None => thread::yield_now(),
                }
            }
            producer.join().unwrap();
            assert_eq!(c.pop(), None);
        });
    }

    #[test]
    fn handoff_one_slot() {
        transfer::<1>(0, 2);
    }

    #[test]
    fn fill_to_full_and_wrap_slots() {
        // 3 items through 2 slots: exercises full, empty and slot reuse.
        transfer::<2>(0, 3);
    }

    #[test]
    fn counter_wrap_at_usize_max() {
        transfer::<2>(usize::MAX - 1, 3);
    }

    struct Tracked(Arc<LAtomic>);
    impl Drop for Tracked {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }

    #[test]
    fn concurrent_pop_then_drop_drops_each_item_once() {
        loom::model(|| {
            let drops = Arc::new(LAtomic::new(0));
            let ring: &'static mut Ring<Tracked, 2> = leaked(0);
            let raw: *mut Ring<Tracked, 2> = ring;
            {
                // SAFETY: [unsafe:spsc.loom-reborrow] `raw` comes from `Box::leak`; this is the
                // only live reference to the ring until the handles below are dropped.
                let ring = unsafe { &mut *raw };
                let (mut p, mut c) = ring.split();
                let d = drops.clone();
                let producer = thread::spawn(move || {
                    let _ = p.push(Tracked(d.clone()));
                    let _ = p.push(Tracked(d));
                });
                let popped = c.pop();
                producer.join().unwrap();
                drop(popped);
            }
            // SAFETY: [unsafe:spsc.loom-unleak] both handles and the spawned thread are gone;
            // `raw` came from `Box::leak(Box::new(..))` and is turned back into that box once.
            drop(unsafe { Box::from_raw(raw) });
            assert_eq!(drops.load(Ordering::Relaxed), 2);
        });
    }
}
