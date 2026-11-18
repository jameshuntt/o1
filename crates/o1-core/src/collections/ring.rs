use core::fmt;
use core::iter::FusedIterator;
use core::mem::MaybeUninit;

/// A fixed-capacity FIFO ring buffer holding up to `N` values inline.
///
/// * `N` must be a power of two and at least 1 — a **compile-time** error otherwise
///   (slot index is `counter & (N - 1)`):
///
/// ```compile_fail,E0080
/// let _ = o1_core::collections::FixedRing::<u8, 3>::new();
/// ```
/// ```compile_fail,E0080
/// let _ = o1_core::collections::FixedRing::<u8, 0>::new();
/// ```
///
/// * `push_back`, `pop_front`, `front`, `get`, `len` are worst-case O(1) work and
///   allocation-free. Iteration is O(len). `clear` and `Drop` are O(len) (each live element
///   is dropped once).
/// * A full ring refuses `push_back` and hands the value back: `Err(value)`.
/// * Indices are free-running `usize` counters that wrap at `usize::MAX`; because `N`
///   divides `2^usize::BITS`, length and slot positions stay correct across the wrap.
/// * Single-threaded. `Send`/`Sync` follow `T` (auto traits).
/// * Zero-sized `T` is supported; no memory is touched for it.
///
/// ```
/// use o1_core::collections::FixedRing;
/// let mut r = FixedRing::<u32, 4>::new();
/// for i in 0..4 { r.push_back(i).unwrap(); }
/// assert_eq!(r.push_back(9), Err(9)); // full: value returned
/// assert_eq!(r.pop_front(), Some(0));
/// assert_eq!(r.iter().copied().collect::<Vec<_>>(), [1, 2, 3]);
/// ```
pub struct FixedRing<T, const N: usize> {
    /// Slots `head..tail` (mod N) are initialised; all others are not.
    buf: [MaybeUninit<T>; N],
    /// Counter of the oldest element.
    head: usize,
    /// Counter one past the newest element. Invariant: `tail - head <= N` (wrapping).
    tail: usize,
}

impl<T, const N: usize> FixedRing<T, N> {
    const VALID: () = assert!(
        N.is_power_of_two(),
        "FixedRing: N must be a non-zero power of two"
    );
    const MASK: usize = {
        let () = Self::VALID;
        N - 1
    };

    /// An empty ring.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        let () = Self::VALID;
        Self {
            buf: [const { MaybeUninit::uninit() }; N],
            head: 0,
            tail: 0,
        }
    }

    /// Test hook: an empty ring whose counters start at `start` (to exercise wraparound).
    #[cfg(test)]
    pub(crate) const fn with_start(start: usize) -> Self {
        let () = Self::VALID;
        Self {
            buf: [const { MaybeUninit::uninit() }; N],
            head: start,
            tail: start,
        }
    }

    /// Capacity, `N`.
    #[inline]
    #[must_use]
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Number of elements.
    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        self.tail.wrapping_sub(self.head)
    }

    /// Whether the ring holds no elements.
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Whether the ring holds `N` elements.
    #[inline]
    #[must_use]
    pub const fn is_full(&self) -> bool {
        self.len() == N
    }

    /// Appends `value` at the back.
    ///
    /// # Errors
    ///
    /// `Err(value)` if the ring is full; the ring is unchanged.
    #[inline]
    pub fn push_back(&mut self, value: T) -> Result<(), T> {
        if self.is_full() {
            return Err(value);
        }
        self.buf[self.tail & Self::MASK].write(value);
        self.tail = self.tail.wrapping_add(1);
        Ok(())
    }

    /// Removes and returns the front element, or `None` if empty.
    #[inline]
    pub fn pop_front(&mut self) -> Option<T> {
        if self.is_empty() {
            return None;
        }
        let slot = &self.buf[self.head & Self::MASK];
        // SAFETY: [unsafe:ring.pop-read] the ring is non-empty, so the slot at `head` is in
        // `head..tail` and initialised. Advancing `head` right after marks it uninitialised,
        // so the value is moved out exactly once.
        let value = unsafe { slot.assume_init_read() };
        self.head = self.head.wrapping_add(1);
        Some(value)
    }

    /// The front element, or `None` if empty.
    #[inline]
    #[must_use]
    pub fn front(&self) -> Option<&T> {
        self.get(0)
    }

    /// The element `i` positions from the front, or `None` if `i >= len`.
    #[inline]
    #[must_use]
    pub fn get(&self, i: usize) -> Option<&T> {
        if i >= self.len() {
            return None;
        }
        let slot = &self.buf[self.head.wrapping_add(i) & Self::MASK];
        // SAFETY: [unsafe:ring.get-ref] `i < len`, so `head + i` lies in `head..tail` and the
        // slot is initialised; the shared borrow of `self` keeps it initialised for the
        // returned lifetime.
        Some(unsafe { slot.assume_init_ref() })
    }

    /// Mutable access to the element `i` positions from the front.
    #[inline]
    #[must_use]
    pub fn get_mut(&mut self, i: usize) -> Option<&mut T> {
        if i >= self.len() {
            return None;
        }
        let slot = &mut self.buf[self.head.wrapping_add(i) & Self::MASK];
        // SAFETY: [unsafe:ring.get-mut] as for `get`; the exclusive borrow of `self` makes
        // the returned reference unique.
        Some(unsafe { slot.assume_init_mut() })
    }

    /// Drops every element (front to back). O(len).
    ///
    /// If an element's `Drop` panics, that element has already been removed and the elements
    /// after it stay in the ring, which remains consistent and usable; no element is dropped
    /// twice.
    pub fn clear(&mut self) {
        while self.pop_front().is_some() {}
    }

    /// Front-to-back iterator. O(len) in total.
    #[inline]
    pub fn iter(&self) -> Iter<'_, T, N> {
        Iter {
            ring: self,
            front: 0,
            back: self.len(),
        }
    }
}

impl<T, const N: usize> Default for FixedRing<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const N: usize> Drop for FixedRing<T, N> {
    fn drop(&mut self) {
        // Mark the ring empty *before* running any destructor: if one panics, the remaining
        // elements are leaked instead of being dropped again by an unwinding caller.
        let (head, len) = (self.head, self.len());
        self.head = self.tail;
        for i in 0..len {
            let slot = &mut self.buf[head.wrapping_add(i) & Self::MASK];
            // SAFETY: [unsafe:ring.drop] slots `head..head+len` were initialised when this
            // loop started, each is visited once, and the ring was marked empty first, so no
            // other path reads them again.
            unsafe { slot.assume_init_drop() };
        }
    }
}

impl<T: fmt::Debug, const N: usize> fmt::Debug for FixedRing<T, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

/// Front-to-back iterator over a [`FixedRing`].
#[derive(Debug)]
pub struct Iter<'a, T, const N: usize> {
    ring: &'a FixedRing<T, N>,
    front: usize,
    back: usize,
}

impl<'a, T, const N: usize> Iterator for Iter<'a, T, N> {
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<&'a T> {
        if self.front == self.back {
            return None;
        }
        let item = self.ring.get(self.front);
        self.front += 1;
        item
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.back - self.front;
        (n, Some(n))
    }
}

impl<T, const N: usize> DoubleEndedIterator for Iter<'_, T, N> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.front == self.back {
            return None;
        }
        self.back -= 1;
        self.ring.get(self.back)
    }
}

impl<T, const N: usize> ExactSizeIterator for Iter<'_, T, N> {}
impl<T, const N: usize> FusedIterator for Iter<'_, T, N> {}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::Cell;
    use std::vec::Vec;

    struct Counted<'a>(&'a Cell<usize>);
    impl Drop for Counted<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    #[test]
    fn fill_drain_and_full() {
        let mut r = FixedRing::<u8, 2>::new();
        assert!(r.is_empty());
        r.push_back(1).unwrap();
        r.push_back(2).unwrap();
        assert!(r.is_full());
        assert_eq!(r.push_back(3), Err(3));
        assert_eq!(r.pop_front(), Some(1));
        r.push_back(3).unwrap();
        assert_eq!(r.get(0), Some(&2));
        assert_eq!(r.get(1), Some(&3));
        assert_eq!(r.get(2), None);
        *r.get_mut(1).unwrap() = 30;
        assert_eq!(r.iter().rev().copied().collect::<Vec<_>>(), [30, 2]);
        assert_eq!(r.pop_front(), Some(2));
        assert_eq!(r.pop_front(), Some(30));
        assert_eq!(r.pop_front(), None);
    }

    #[test]
    fn counter_wraparound() {
        let mut r = FixedRing::<u32, 4>::with_start(usize::MAX - 2);
        for round in 0..10u32 {
            for i in 0..4 {
                r.push_back(round * 10 + i).unwrap();
            }
            assert!(r.is_full());
            for i in 0..4 {
                assert_eq!(r.pop_front(), Some(round * 10 + i));
            }
            assert!(r.is_empty());
        }
    }

    #[test]
    fn drop_partial_and_wrapped() {
        let drops = Cell::new(0);
        {
            let mut r = FixedRing::<Counted<'_>, 4>::with_start(usize::MAX);
            for _ in 0..3 {
                assert!(r.push_back(Counted(&drops)).is_ok());
            }
            drop(r.pop_front());
            assert_eq!(drops.get(), 1);
        }
        assert_eq!(drops.get(), 3);
    }

    #[test]
    fn zst() {
        let mut r = FixedRing::<(), 8>::new();
        for _ in 0..8 {
            r.push_back(()).unwrap();
        }
        assert_eq!(r.push_back(()), Err(()));
        assert_eq!(r.len(), 8);
        r.clear();
        assert!(r.is_empty());
    }
}
