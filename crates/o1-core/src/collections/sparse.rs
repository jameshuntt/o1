use core::fmt;
use core::iter::FusedIterator;

/// A set of integers in `0..N` with worst-case O(1) insert, remove, membership and clear.
///
/// Two arrays: `dense[..len]` lists the members in insertion order (perturbed by removals),
/// and `sparse[x]` is `x`'s position in `dense`. `x` is a member iff
/// `sparse[x] < len && dense[sparse[x]] == x`.
///
/// | operation | work |
/// |---|---|
/// | `insert`, `remove`, `contains`, `len` | worst-case O(1) |
/// | `clear` | worst-case O(1) (only `len` is reset) |
/// | `iter`, `as_slice` iteration | O(len) — **not** O(1) |
/// | `new` | O(N) (both arrays are zero-initialised; see below) |
///
/// Both arrays are fully initialised. The classic formulation leaves `sparse` uninitialised;
/// reading uninitialised memory is undefined behaviour in Rust, so it is not done here.
/// Stale values in `sparse` are harmless because membership re-checks `dense`.
///
/// Memory: `8 * N` bytes plus a `u32`. `N <= u32::MAX` is enforced at compile time:
///
/// ```compile_fail,E0080
/// // 2^32 members do not fit u32 positions (and would not fit the stack either).
/// let _ = o1_core::collections::SparseSet::<{ 1usize << 32 }>::new();
/// ```
///
/// Iteration order is `dense` order: insertion order, except that `remove` moves the last
/// member into the removed member's place.
///
/// ```
/// use o1_core::collections::SparseSet;
/// let mut s = SparseSet::<16>::new();
/// assert_eq!(s.insert(3), Ok(true));
/// assert_eq!(s.insert(3), Ok(false)); // already present
/// assert_eq!(s.insert(16), Err(16)); // out of range
/// assert!(s.contains(3));
/// s.clear();
/// assert!(!s.contains(3));
/// ```
#[derive(Clone)]
pub struct SparseSet<const N: usize> {
    dense: [u32; N],
    sparse: [u32; N],
    len: u32,
}

impl<const N: usize> SparseSet<N> {
    const VALID: () = assert!(
        N <= u32::MAX as usize,
        "SparseSet: N must not exceed u32::MAX"
    );

    /// An empty set. O(N): both arrays are zero-filled.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        let () = Self::VALID;
        Self {
            dense: [0; N],
            sparse: [0; N],
            len: 0,
        }
    }

    /// Universe size, `N`: members are in `0..N`.
    #[inline]
    #[must_use]
    pub const fn universe(&self) -> usize {
        N
    }

    /// Number of members.
    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len as usize
    }

    /// Whether the set is empty.
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Whether `x` is a member. `x >= N` is never a member.
    #[inline]
    #[must_use]
    pub fn contains(&self, x: usize) -> bool {
        match self.sparse.get(x) {
            Some(&pos) => pos < self.len && self.dense[pos as usize] as usize == x,
            None => false,
        }
    }

    /// Inserts `x`. `Ok(true)` if newly inserted, `Ok(false)` if already present.
    ///
    /// # Errors
    ///
    /// `Err(x)` if `x >= N`. (The set can never be "full": it holds at most `N` distinct
    /// members of `0..N`.)
    #[inline]
    pub fn insert(&mut self, x: usize) -> Result<bool, usize> {
        if x >= N {
            return Err(x);
        }
        if self.contains(x) {
            return Ok(false);
        }
        let pos = self.len;
        // x < N <= u32::MAX, and len < N because x is not yet a member.
        self.dense[pos as usize] = x as u32;
        self.sparse[x] = pos;
        self.len += 1;
        Ok(true)
    }

    /// Removes `x`. Returns whether it was a member. Moves the last member into `x`'s place.
    #[inline]
    pub fn remove(&mut self, x: usize) -> bool {
        if !self.contains(x) {
            return false;
        }
        let pos = self.sparse[x];
        let last = self.dense[(self.len - 1) as usize];
        self.dense[pos as usize] = last;
        self.sparse[last as usize] = pos;
        self.len -= 1;
        true
    }

    /// Removes every member. Worst-case O(1).
    #[inline]
    pub fn clear(&mut self) {
        self.len = 0;
    }

    /// The members, in dense order.
    #[inline]
    #[must_use]
    pub fn as_slice(&self) -> &[u32] {
        &self.dense[..self.len as usize]
    }

    /// Iterator over the members in dense order. O(len) in total.
    #[inline]
    pub fn iter(&self) -> Iter<'_> {
        Iter {
            inner: self.as_slice().iter(),
        }
    }
}

impl<const N: usize> Default for SparseSet<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> fmt::Debug for SparseSet<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }
}

/// Iterator over a [`SparseSet`]'s members.
#[derive(Debug, Clone)]
pub struct Iter<'a> {
    inner: core::slice::Iter<'a, u32>,
}

impl Iterator for Iter<'_> {
    type Item = usize;
    #[inline]
    fn next(&mut self) -> Option<usize> {
        self.inner.next().map(|&x| x as usize)
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl ExactSizeIterator for Iter<'_> {}
impl FusedIterator for Iter<'_> {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec::Vec;

    #[test]
    fn basic() {
        let mut s = SparseSet::<8>::new();
        for x in [5, 1, 7] {
            assert_eq!(s.insert(x), Ok(true));
        }
        assert_eq!(s.as_slice(), &[5, 1, 7]);
        assert!(s.remove(5));
        assert!(!s.remove(5));
        assert_eq!(s.as_slice(), &[7, 1]);
        assert!(s.contains(1) && s.contains(7) && !s.contains(5));
        assert!(!s.contains(100));
        assert!(!s.remove(100));
        s.clear();
        assert!(s.is_empty());
        // After clear, stale sparse entries must not resurrect members.
        for x in 0..8 {
            assert!(!s.contains(x));
        }
        assert_eq!(s.insert(1), Ok(true));
        assert_eq!(s.iter().collect::<Vec<_>>(), [1]);
    }

    #[test]
    fn fill_universe() {
        let mut s = SparseSet::<4>::new();
        for x in 0..4 {
            assert_eq!(s.insert(x), Ok(true));
        }
        assert_eq!(s.len(), 4);
        assert_eq!(s.insert(4), Err(4));
        let empty = SparseSet::<0>::new();
        assert!(!empty.contains(0));
    }
}
