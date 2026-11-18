//! The one switch point between real atomics/cells and loom's model-checked ones.
//!
//! In this crate's own unit tests (`cfg(test)` without `loom`), and in a build with the
//! non-default feature `access-counting` (tooling only), the switch also counts: every atomic
//! load and store and every slot access the ring performs is tallied per thread, so a test or
//! a verifier can compare what `push`/`pop` structurally do with their published contracts.
//! The counting types forward to the real `core` atomics with the same orderings. A default
//! build compiles none of it (checked by `scripts/codegen.sh`: the SPSC probes contain no
//! thread-local access).

#[cfg(loom)]
pub(crate) use loom::sync::atomic::{AtomicUsize, Ordering};

#[cfg(not(loom))]
pub(crate) use core::sync::atomic::Ordering;

#[cfg(all(not(loom), not(any(test, feature = "access-counting"))))]
pub(crate) use core::sync::atomic::AtomicUsize;

#[cfg(all(not(loom), any(test, feature = "access-counting")))]
pub(crate) use counting::AtomicUsize;

/// Reads an atomic through `&mut` (no concurrent access possible).
#[inline]
pub(crate) fn load_mut(a: &mut AtomicUsize) -> usize {
    #[cfg(loom)]
    {
        a.with_mut(|v| *v)
    }
    #[cfg(not(loom))]
    {
        *a.get_mut()
    }
}

/// `UnsafeCell` with loom's closure-based access API, so the ring's slot accesses are
/// tracked by loom under `--cfg loom` and compile to plain pointer accesses otherwise.
#[cfg(not(loom))]
#[repr(transparent)]
pub(crate) struct UnsafeCell<T>(core::cell::UnsafeCell<T>);

#[cfg(not(loom))]
impl<T> UnsafeCell<T> {
    #[inline]
    pub(crate) const fn new(v: T) -> Self {
        Self(core::cell::UnsafeCell::new(v))
    }

    #[inline]
    pub(crate) fn with_mut<R>(&self, f: impl FnOnce(*mut T) -> R) -> R {
        #[cfg(any(test, feature = "access-counting"))]
        counting::bump(|c| c.cell_accesses += 1);
        f(self.0.get())
    }
}

#[cfg(loom)]
pub(crate) use loom::cell::UnsafeCell;

#[cfg(all(any(test, feature = "access-counting"), not(loom)))]
pub mod counting {
    //! Per-thread counts of the ring's atomic loads, atomic stores and slot accesses.
    //!
    //! Compiled only in this crate's unit tests and with the non-default feature
    //! `access-counting` (verification tooling; never for production). The counters are
    //! thread-local, so each thread sees only its own accesses: call [`take`] before and after
    //! one `push` or `pop` on the same thread.
    use core::cell::Cell;
    use core::sync::atomic::{self, Ordering};

    use o1_contract::{Quantity, ResourceEnvelope};

    /// What the current thread has done since the last [`take`].
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    pub struct Counts {
        /// Atomic loads (any ordering).
        pub loads: u64,
        /// Atomic stores (any ordering).
        pub stores: u64,
        /// Slot accesses (one per element written or read).
        pub cell_accesses: u64,
    }

    impl Counts {
        /// What one `push` did, as an envelope. By construction of the ring, every atomic
        /// access in `push` is to a counter shared with the consumer (a load of `head`, a
        /// store of `tail`) and every slot access is a write of a slot the consumer reads;
        /// `size` is `size_of::<T>()`.
        #[must_use]
        pub fn push_envelope(self, size: u64) -> ResourceEnvelope {
            ResourceEnvelope::ZERO
                .with(Quantity::Ops, 1)
                .with(Quantity::BytesWritten, self.cell_accesses * size)
                .with(Quantity::AtomicLoads, self.loads)
                .with(Quantity::AtomicStores, self.stores)
                .with(Quantity::SharedReads, self.loads)
                .with(Quantity::SharedWrites, self.stores + self.cell_accesses)
        }

        /// The mirror image for `pop`: the slot access is a read of producer-written memory.
        #[must_use]
        pub fn pop_envelope(self, size: u64) -> ResourceEnvelope {
            ResourceEnvelope::ZERO
                .with(Quantity::Ops, 1)
                .with(Quantity::BytesRead, self.cell_accesses * size)
                .with(Quantity::AtomicLoads, self.loads)
                .with(Quantity::AtomicStores, self.stores)
                .with(Quantity::SharedReads, self.loads + self.cell_accesses)
                .with(Quantity::SharedWrites, self.stores)
        }
    }

    std::thread_local! {
        static COUNTS: Cell<Counts> = const {
            Cell::new(Counts { loads: 0, stores: 0, cell_accesses: 0 })
        };
    }

    pub(crate) fn bump(f: impl FnOnce(&mut Counts)) {
        COUNTS.with(|c| {
            let mut v = c.get();
            f(&mut v);
            c.set(v);
        });
    }

    /// Returns this thread's counts and resets them.
    pub fn take() -> Counts {
        COUNTS.with(|c| c.replace(Counts::default()))
    }

    /// `core::sync::atomic::AtomicUsize` that counts loads and stores. It has no
    /// read-modify-write methods: a read-modify-write in the ring would not compile in a
    /// counting build.
    #[repr(transparent)]
    pub(crate) struct AtomicUsize(atomic::AtomicUsize);

    impl AtomicUsize {
        pub(crate) const fn new(v: usize) -> Self {
            Self(atomic::AtomicUsize::new(v))
        }
        pub(crate) fn load(&self, order: Ordering) -> usize {
            bump(|c| c.loads += 1);
            self.0.load(order)
        }
        pub(crate) fn store(&self, v: usize, order: Ordering) {
            bump(|c| c.stores += 1);
            self.0.store(v, order);
        }
        pub(crate) fn get_mut(&mut self) -> &mut usize {
            self.0.get_mut()
        }
    }
}
