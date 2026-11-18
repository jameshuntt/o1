//! Fixed-capacity slot storage: [`FreeList`] (index-addressed) and [`GenerationalArena`]
//! (handle-addressed, stale handles rejected).
//!
//! Both store `N` slots inline, never allocate, never resize, and give a rejected value back
//! when full. Both are written without `unsafe`: each slot is an `enum` whose vacant variant
//! stores the free-list link **in the payload's place**. That removes the need for a separate
//! link array; it does not remove per-slot metadata — the enum discriminant remains (and is
//! often absorbed into a niche of `T`).
//!
//! Slot indices are `u32`; `N <= u32::MAX` is a compile-time check:
//!
//! ```compile_fail,E0080
//! let _ = o1_core::arena::FreeList::<u8, { 1usize << 32 }>::new();
//! ```
//!
//! # Boundary with higher-level arenas
//!
//! These are the slot + generation core only. Keyed check words, selectors, paging and
//! growth (as in petr-collections' `KeyedArena`) belong above this layer.

use core::fmt;
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;

use o1_contract::{
    Access, Cost, ExecutionContract, Isolation, PolicyDesc, Progress, Quantity, ResourceEnvelope,
    Sharing,
};

/// Sentinel for "no next free slot".
const NIL: u32 = u32::MAX;

enum Slot<T> {
    Occupied(T),
    /// On the free list; `next` is the next free slot or `NIL`.
    Vacant {
        next: u32,
    },
    /// Never used yet (above the bump mark).
    Unused,
}

/// A fixed-capacity pool of `N` slots addressed by `usize` index.
///
/// | operation | work |
/// |---|---|
/// | `insert`, `remove`, `get`, `get_mut`, `len` | worst-case O(1) |
/// | `new` | O(N) (slot array initialisation) |
/// | `iter`, `Drop` | O(N) — every slot is visited |
///
/// Indices are reused immediately after `remove` (LIFO), so an index kept after its value
/// was removed can name a different value later. Use [`GenerationalArena`] when that must be
/// detected.
///
/// ```
/// use o1_core::arena::FreeList;
/// let mut f = FreeList::<&str, 2>::new();
/// let a = f.insert("a").unwrap();
/// let b = f.insert("b").unwrap();
/// assert_eq!(f.insert("c"), Err("c")); // full: value returned
/// assert_eq!(f.remove(a), Some("a"));
/// assert_eq!(f.insert("c"), Ok(a)); // slot reused
/// assert_eq!(f.get(b), Some(&"b"));
/// ```
pub struct FreeList<T, const N: usize> {
    slots: [Slot<T>; N],
    /// Head of the free list, or `NIL`.
    free_head: u32,
    /// Slots `bump..N` are `Unused`.
    bump: u32,
    len: u32,
}

impl<T, const N: usize> FreeList<T, N> {
    const VALID: () = assert!(
        N <= u32::MAX as usize,
        "FreeList: N must not exceed u32::MAX"
    );

    /// An empty pool.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        let () = Self::VALID;
        Self {
            slots: [const { Slot::Unused }; N],
            free_head: NIL,
            bump: 0,
            len: 0,
        }
    }

    /// Capacity, `N`.
    #[inline]
    #[must_use]
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Number of occupied slots.
    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len as usize
    }

    /// Whether no slot is occupied.
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Stores `value` and returns its index.
    ///
    /// # Errors
    ///
    /// `Err(value)` if all `N` slots are occupied.
    #[inline]
    pub fn insert(&mut self, value: T) -> Result<usize, T> {
        let index = if self.free_head != NIL {
            let index = self.free_head;
            self.free_head = match self.slots[index as usize] {
                Slot::Vacant { next } => next,
                // Only vacant slots are linked; unreachable.
                _ => NIL,
            };
            index
        } else if (self.bump as usize) < N {
            let index = self.bump;
            self.bump += 1;
            index
        } else {
            return Err(value);
        };
        self.slots[index as usize] = Slot::Occupied(value);
        self.len += 1;
        Ok(index as usize)
    }

    /// Removes and returns the value at `index`; `None` if the slot is not occupied or
    /// `index >= N`.
    #[inline]
    pub fn remove(&mut self, index: usize) -> Option<T> {
        let slot = self.slots.get_mut(index)?;
        if !matches!(slot, Slot::Occupied(_)) {
            return None;
        }
        let old = core::mem::replace(
            slot,
            Slot::Vacant {
                next: self.free_head,
            },
        );
        self.free_head = index as u32;
        self.len -= 1;
        match old {
            Slot::Occupied(v) => Some(v),
            _ => None,
        }
    }

    /// Shared access to the value at `index`.
    #[inline]
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&T> {
        match self.slots.get(index)? {
            Slot::Occupied(v) => Some(v),
            _ => None,
        }
    }

    /// Mutable access to the value at `index`.
    #[inline]
    #[must_use]
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        match self.slots.get_mut(index)? {
            Slot::Occupied(v) => Some(v),
            _ => None,
        }
    }

    /// `(index, &value)` for every occupied slot, in index order. O(N), not O(len).
    pub fn iter(&self) -> impl Iterator<Item = (usize, &T)> + '_ {
        self.slots[..self.bump as usize]
            .iter()
            .enumerate()
            .filter_map(|(i, s)| match s {
                Slot::Occupied(v) => Some((i, v)),
                _ => None,
            })
    }
}

impl<T, const N: usize> Default for FreeList<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: fmt::Debug, const N: usize> fmt::Debug for FreeList<T, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

/// A handle to a value in a [`GenerationalArena<T, N>`]: slot index plus the slot's
/// generation when the value was inserted. 8 bytes. `Copy`, comparable, hashable.
///
/// A handle is only meaningful for the arena that issued it. Presenting it to a different
/// arena of the same `T` is memory-safe but may name an unrelated value (not detected).
pub struct Handle<T> {
    index: u32,
    generation: u32,
    _for: PhantomData<fn() -> T>,
}

impl<T> Handle<T> {
    /// Slot index.
    #[inline]
    #[must_use]
    pub const fn index(self) -> u32 {
        self.index
    }

    /// Generation the slot had when the value was inserted.
    #[inline]
    #[must_use]
    pub const fn generation(self) -> u32 {
        self.generation
    }

    /// Rebuilds a handle from its parts (e.g. after serialisation). Safe: a forged handle
    /// is either rejected by the arena or names whatever currently lives under exactly that
    /// index and generation.
    #[inline]
    #[must_use]
    pub const fn from_parts(index: u32, generation: u32) -> Self {
        Self {
            index,
            generation,
            _for: PhantomData,
        }
    }
}

impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Handle<T> {}
impl<T> PartialEq for Handle<T> {
    fn eq(&self, o: &Self) -> bool {
        self.index == o.index && self.generation == o.generation
    }
}
impl<T> Eq for Handle<T> {}
impl<T> Hash for Handle<T> {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.index.hash(h);
        self.generation.hash(h);
    }
}
impl<T> fmt::Debug for Handle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Handle({}v{})", self.index, self.generation)
    }
}

enum Entry<T> {
    Occupied {
        generation: u32,
        value: T,
    },
    Vacant {
        generation: u32,
        next: u32,
    },
    Unused,
    /// Generation exhausted; never reused.
    Retired,
}

/// A fixed-capacity arena whose handles detect reuse.
///
/// Every slot has a generation. `remove` bumps it, so every handle to the removed value is
/// rejected from then on, in O(1) (one index bounds check and one comparison).
///
/// **Generation exhaustion.** A slot's generation starts at 0 and increases by one per
/// removal. When a removal would take it past `u32::MAX`, the slot is **retired** instead:
/// it is never handed out again, and [`capacity_remaining`](Self::capacity_remaining) drops
/// by one. Generations never wrap, so a stale handle can never become valid again (no ABA).
/// Reaching this needs 2^32 removals from one slot.
///
/// | operation | work |
/// |---|---|
/// | `insert`, `remove`, `get`, `get_mut`, `contains`, `len` | worst-case O(1) |
/// | `new` | O(N) (slot array initialisation) |
/// | `iter`, `Drop` | O(N) |
///
/// ```
/// use o1_core::arena::GenerationalArena;
/// let mut a = GenerationalArena::<u32, 4>::new();
/// let h = a.insert(10).unwrap();
/// assert_eq!(a.get(h), Some(&10));
/// assert_eq!(a.remove(h), Some(10));
/// let h2 = a.insert(20).unwrap();
/// assert_eq!(h2.index(), h.index()); // same slot ...
/// assert_eq!(a.get(h), None); // ... but the old handle is stale
/// assert_eq!(a.get(h2), Some(&20));
/// ```
pub struct GenerationalArena<T, const N: usize> {
    entries: [Entry<T>; N],
    free_head: u32,
    bump: u32,
    len: u32,
    retired: u32,
}

impl<T, const N: usize> GenerationalArena<T, N> {
    const VALID: () = assert!(
        N <= u32::MAX as usize,
        "GenerationalArena: N must not exceed u32::MAX"
    );

    /// An empty arena.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        let () = Self::VALID;
        Self {
            entries: [const { Entry::Unused }; N],
            free_head: NIL,
            bump: 0,
            len: 0,
            retired: 0,
        }
    }

    /// Capacity, `N` (including retired slots).
    #[inline]
    #[must_use]
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Slots that can still hold a value: `N - retired`.
    #[inline]
    #[must_use]
    pub const fn capacity_remaining(&self) -> usize {
        N - self.retired as usize
    }

    /// Number of retired slots (generation exhausted).
    #[inline]
    #[must_use]
    pub const fn retired(&self) -> usize {
        self.retired as usize
    }

    /// Number of live values.
    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len as usize
    }

    /// Whether no value is live.
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Stores `value`; returns its handle.
    ///
    /// # Errors
    ///
    /// `Err(value)` if no slot is free (all occupied or retired).
    #[inline]
    pub fn insert(&mut self, value: T) -> Result<Handle<T>, T> {
        let (index, generation) = if self.free_head != NIL {
            let index = self.free_head;
            match self.entries[index as usize] {
                Entry::Vacant { generation, next } => {
                    self.free_head = next;
                    (index, generation)
                }
                // Only vacant entries are linked; unreachable.
                _ => return Err(value),
            }
        } else if (self.bump as usize) < N {
            let index = self.bump;
            self.bump += 1;
            (index, 0)
        } else {
            return Err(value);
        };
        self.entries[index as usize] = Entry::Occupied { generation, value };
        self.len += 1;
        Ok(Handle::from_parts(index, generation))
    }

    /// Removes the value `h` names. `None` if `h` is stale, out of range, or never issued.
    #[inline]
    pub fn remove(&mut self, h: Handle<T>) -> Option<T> {
        let entry = self.entries.get_mut(h.index as usize)?;
        let generation = match entry {
            Entry::Occupied { generation, .. } if *generation == h.generation => *generation,
            _ => return None,
        };
        let replacement = match generation.checked_add(1) {
            Some(next_gen) => Entry::Vacant {
                generation: next_gen,
                next: self.free_head,
            },
            None => Entry::Retired,
        };
        let old = core::mem::replace(entry, replacement);
        if matches!(self.entries[h.index as usize], Entry::Retired) {
            self.retired += 1;
        } else {
            self.free_head = h.index;
        }
        self.len -= 1;
        match old {
            Entry::Occupied { value, .. } => Some(value),
            _ => None,
        }
    }

    /// Whether `h` names a live value.
    #[inline]
    #[must_use]
    pub fn contains(&self, h: Handle<T>) -> bool {
        self.get(h).is_some()
    }

    /// Shared access to the value `h` names, or `None` if stale.
    #[inline]
    #[must_use]
    pub fn get(&self, h: Handle<T>) -> Option<&T> {
        match self.entries.get(h.index as usize)? {
            Entry::Occupied { generation, value } if *generation == h.generation => Some(value),
            _ => None,
        }
    }

    /// Mutable access to the value `h` names, or `None` if stale.
    #[inline]
    #[must_use]
    pub fn get_mut(&mut self, h: Handle<T>) -> Option<&mut T> {
        match self.entries.get_mut(h.index as usize)? {
            Entry::Occupied { generation, value } if *generation == h.generation => Some(value),
            _ => None,
        }
    }

    /// `(handle, &value)` for every live value, in index order. O(N), not O(len).
    pub fn iter(&self) -> impl Iterator<Item = (Handle<T>, &T)> + '_ {
        self.entries[..self.bump as usize]
            .iter()
            .enumerate()
            .filter_map(|(i, e)| match e {
                Entry::Occupied { generation, value } => {
                    Some((Handle::from_parts(i as u32, *generation), value))
                }
                _ => None,
            })
    }

    /// Test hook: force the generation of a vacant/unused slot (exercises exhaustion).
    #[cfg(test)]
    pub(crate) fn force_generation(&mut self, index: u32, generation: u32) {
        match &mut self.entries[index as usize] {
            Entry::Occupied { generation: g, .. } | Entry::Vacant { generation: g, .. } => {
                *g = generation;
            }
            _ => {}
        }
    }
}

/// Execution contracts of the O(1) slot operations (`o1_contract`).
///
/// | operation | ops | bytes read | bytes written | atomics | shared | working set | sharing |
/// |---|---|---|---|---|---|---|---|
/// | [`insert`](Self::insert) | 1 | 0 | `size_of::<T>()` (value moved in) | 0 | 0 | `size_of::<Self>()` | `Exclusive` (`&mut self`) |
/// | [`remove`](Self::remove) | 1 | `size_of::<T>()` (value moved out) | 0 | 0 | 0 | `size_of::<Self>()` | `Exclusive` (`&mut self`) |
/// | [`get`](Self::get) | 1 | 0 (returns a reference) | 0 | 0 | 0 | `size_of::<Self>()` | `SharedRead` (`&self`) |
///
/// Access is `Random` (the slot is chosen by the handle); progress `NoSynchronization`.
/// The working set is the whole arena: a lookup may address any slot, so n lookups need
/// the arena resident (`repeat(n)` keeps the peak). Control fields (`len`, `free_head`, the
/// entry's generation and discriminant) are not counted as bytes. The numbers are read
/// from the code (no loop on these paths, one indexed entry access each); they are not
/// counted by an instrumented run.
///
/// ```
/// use o1_contract::{Bound, Quantity::*};
/// use o1_core::arena::GenerationalArena;
/// type A = GenerationalArena<[u8; 24], 1024>;
/// assert_eq!(A::GET_CONTRACT.bound(), Bound::Constant);
/// let lookups = A::GET_CONTRACT.envelope(0).unwrap().repeat(1_000_000).unwrap();
/// assert_eq!(lookups.get(Ops), 1_000_000);
/// assert_eq!(lookups.get(WorkingSet), core::mem::size_of::<A>() as u64);
/// assert_eq!(A::INSERT_CONTRACT.envelope(0).unwrap().get(BytesWritten), 24);
/// ```
impl<T, const N: usize> GenerationalArena<T, N> {
    /// Contract of [`insert`](Self::insert).
    pub const INSERT_CONTRACT: ExecutionContract =
        slot_contract::<Self>(Sharing::Exclusive, 0, size_of::<T>());
    /// Contract of [`remove`](Self::remove).
    pub const REMOVE_CONTRACT: ExecutionContract =
        slot_contract::<Self>(Sharing::Exclusive, size_of::<T>(), 0);
    /// Contract of [`get`](Self::get) (and [`contains`](Self::contains)).
    pub const GET_CONTRACT: ExecutionContract = slot_contract::<Self>(Sharing::SharedRead, 0, 0);
}

const fn slot_contract<Arena>(sharing: Sharing, read: usize, written: usize) -> ExecutionContract {
    ExecutionContract {
        progress: Progress::NoSynchronization,
        policy: PolicyDesc {
            access: Some(Access::Random),
            reuse: None,
            sharing: Some(sharing),
            write: None,
        },
        isolation: Isolation::NoClaim,
        cost: Cost::constant(
            ResourceEnvelope::ZERO
                .with(Quantity::Ops, 1)
                .with(Quantity::BytesRead, read as u64)
                .with(Quantity::BytesWritten, written as u64)
                .with(Quantity::WorkingSet, size_of::<Arena>() as u64),
        ),
    }
    .assert_valid()
}

impl<T, const N: usize> Default for GenerationalArena<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: fmt::Debug, const N: usize> fmt::Debug for GenerationalArena<T, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

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
    fn free_list_reuse_and_bounds() {
        let mut f = FreeList::<u32, 3>::new();
        let a = f.insert(1).unwrap();
        let b = f.insert(2).unwrap();
        let c = f.insert(3).unwrap();
        assert_eq!(f.insert(4), Err(4));
        assert_eq!(f.remove(b), Some(2));
        assert_eq!(f.remove(b), None);
        assert_eq!(f.remove(99), None);
        assert_eq!(f.get(99), None);
        assert_eq!(f.insert(5), Ok(b));
        assert_eq!(
            f.iter().map(|(i, v)| (i, *v)).collect::<Vec<_>>(),
            [(a, 1), (b, 5), (c, 3)]
        );
        *f.get_mut(c).unwrap() = 30;
        assert_eq!(f.get(c), Some(&30));
        assert_eq!(f.len(), 3);
    }

    #[test]
    fn arena_stale_rejected() {
        let mut a = GenerationalArena::<u32, 2>::new();
        let h1 = a.insert(1).unwrap();
        let h2 = a.insert(2).unwrap();
        assert!(a.insert(3).is_err());
        assert_eq!(a.remove(h1), Some(1));
        assert_eq!(a.remove(h1), None);
        let h3 = a.insert(3).unwrap();
        assert_eq!(h3.index(), h1.index());
        assert_eq!(h3.generation(), h1.generation() + 1);
        assert!(!a.contains(h1));
        assert!(a.contains(h2) && a.contains(h3));
        assert_eq!(a.get(Handle::from_parts(7, 0)), None);
        assert_eq!(a.get(Handle::from_parts(h2.index(), 9)), None);
    }

    #[test]
    fn generation_exhaustion_retires_slot() {
        let mut a = GenerationalArena::<u8, 2>::new();
        let h = a.insert(1).unwrap();
        a.force_generation(h.index(), u32::MAX);
        let h = Handle::from_parts(h.index(), u32::MAX);
        assert_eq!(a.remove(h), Some(1));
        assert_eq!(a.retired(), 1);
        assert_eq!(a.capacity_remaining(), 1);
        assert_eq!(a.get(h), None);
        // The retired slot is never handed out again.
        let h2 = a.insert(2).unwrap();
        assert_ne!(h2.index(), h.index());
        assert_eq!(a.insert(3), Err(3));
        assert_eq!(a.remove(h2), Some(2));
        let h3 = a.insert(4).unwrap();
        assert_eq!(h3.index(), h2.index());
        assert_eq!(h3.generation(), 1);
    }

    #[test]
    fn drops_exactly_live_values() {
        let drops = Cell::new(0);
        {
            let mut a = GenerationalArena::<Counted<'_>, 8>::new();
            let mut f = FreeList::<Counted<'_>, 8>::new();
            let hs: Vec<_> = (0..5)
                .map(|_| a.insert(Counted(&drops)).ok().unwrap())
                .collect();
            let is: Vec<_> = (0..5)
                .map(|_| f.insert(Counted(&drops)).ok().unwrap())
                .collect();
            drop(a.remove(hs[1]));
            drop(f.remove(is[3]));
            assert_eq!(drops.get(), 2);
        }
        assert_eq!(drops.get(), 10);
    }

    #[test]
    fn zst_values() {
        let mut a = GenerationalArena::<(), 4>::new();
        let hs: Vec<_> = (0..4).map(|_| a.insert(()).unwrap()).collect();
        assert_eq!(a.insert(()), Err(()));
        assert_eq!(a.remove(hs[0]), Some(()));
        assert_eq!(a.iter().count(), 3);
        let mut f = FreeList::<(), 0>::new();
        assert_eq!(f.insert(()), Err(()));
    }
}
