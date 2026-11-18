//! Drop behaviour, including a destructor that panics, and auto-trait assertions.

use o1_core::arena::{FreeList, GenerationalArena, Handle};
use o1_core::collections::{FixedRing, SparseSet};
use o1_core::ptr::{AtomicTaggedPtr, TaggedPtr};
use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};

thread_local! {
    static DROPS: Cell<usize> = const { Cell::new(0) };
}

struct Bomb {
    explode: bool,
}

impl Drop for Bomb {
    fn drop(&mut self) {
        DROPS.with(|d| d.set(d.get() + 1));
        if self.explode {
            panic!("bomb");
        }
    }
}

fn drops() -> usize {
    DROPS.with(Cell::get)
}

#[test]
fn ring_panic_in_drop_never_double_drops() {
    DROPS.with(|d| d.set(0));
    let result = catch_unwind(AssertUnwindSafe(|| {
        let mut r = FixedRing::<Bomb, 4>::new();
        assert!(r.push_back(Bomb { explode: false }).is_ok());
        assert!(r.push_back(Bomb { explode: true }).is_ok());
        assert!(r.push_back(Bomb { explode: false }).is_ok());
        drop(r);
    }));
    assert!(result.is_err());
    // First dropped normally, second panicked (counted), third leaked: never twice.
    assert_eq!(drops(), 2);
}

#[test]
fn ring_clear_panic_leaves_ring_usable() {
    DROPS.with(|d| d.set(0));
    let mut r = FixedRing::<Bomb, 4>::new();
    assert!(r.push_back(Bomb { explode: true }).is_ok());
    assert!(r.push_back(Bomb { explode: false }).is_ok());
    let result = catch_unwind(AssertUnwindSafe(|| r.clear()));
    assert!(result.is_err());
    // pop_front moved the bomb out before it exploded; the ring is consistent.
    assert_eq!(r.len(), 1);
    r.clear();
    assert_eq!(drops(), 2);
}

#[test]
fn arena_panic_in_drop_never_double_drops() {
    DROPS.with(|d| d.set(0));
    let result = catch_unwind(AssertUnwindSafe(|| {
        let mut a = GenerationalArena::<Bomb, 4>::new();
        assert!(a.insert(Bomb { explode: false }).is_ok());
        assert!(a.insert(Bomb { explode: true }).is_ok());
        assert!(a.insert(Bomb { explode: false }).is_ok());
        drop(a);
    }));
    assert!(result.is_err());
    // Array drop glue keeps dropping the remaining elements after a panic: all three run once.
    assert_eq!(drops(), 3);
}

fn is_send<T: Send>() {}
fn is_sync<T: Sync>() {}

#[test]
fn auto_traits() {
    is_send::<FixedRing<u8, 4>>();
    is_sync::<FixedRing<u8, 4>>();
    is_send::<GenerationalArena<u8, 4>>();
    is_sync::<FreeList<u8, 4>>();
    is_send::<Handle<std::rc::Rc<u8>>>(); // a handle holds no T
    is_sync::<SparseSet<4>>();
    is_send::<AtomicTaggedPtr<u64, 3>>();
    is_sync::<AtomicTaggedPtr<u64, 3>>();
    let _ = core::mem::size_of::<TaggedPtr<u64, 3>>();
    assert_eq!(
        core::mem::size_of::<TaggedPtr<u64, 3>>(),
        core::mem::size_of::<usize>()
    );
    assert_eq!(
        core::mem::size_of::<Option<TaggedPtr<u64, 3>>>(),
        core::mem::size_of::<usize>()
    );
    assert_eq!(core::mem::size_of::<Handle<u64>>(), 8);
}
