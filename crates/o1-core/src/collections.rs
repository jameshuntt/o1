//! Fixed-capacity collections: [`FixedRing`] (FIFO ring) and [`SparseSet`].
//!
//! Both store everything inline (no heap), have a capacity fixed by the type, never resize,
//! and report exhaustion explicitly. A large `N` makes a large value: put it in a `static`
//! or in memory the caller owns rather than on a small stack.

mod ring;
mod sparse;

pub use ring::{FixedRing, Iter as FixedRingIter};
pub use sparse::{Iter as SparseSetIter, SparseSet};
