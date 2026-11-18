//! Strict-provenance tagged pointers and alignment helpers.
//!
//! Every address computation here goes through [`NonNull::map_addr`] or
//! [`pointer::map_addr`](https://doc.rust-lang.org/core/primitive.pointer.html#method.map_addr):
//! the resulting pointer carries the provenance of the pointer it was derived from.
//! There is no pointer→integer→pointer round trip anywhere in this module, and exposed
//! provenance (`expose_provenance` / `with_exposed_provenance`) is never used.
//!
//! # Tag capacity
//!
//! A `T` is always stored at an address that is a multiple of `align_of::<T>()`, so the low
//! [`tag_capacity::<T>()`](tag_capacity) bits of a valid `*const T` are zero and can carry a
//! tag. [`TaggedPtr<T, BITS>`] refuses, **at compile time**, any `BITS` larger than that:
//!
//! ```compile_fail,E0080
//! use o1_core::ptr::TaggedPtr;
//! // u16 has alignment 2: one tag bit, not two.
//! let x = 7u16;
//! let _ = TaggedPtr::<u16, 2>::from_ref(&x, 0);
//! ```
//!
//! ```compile_fail,E0080
//! use o1_core::ptr::TaggedPtr;
//! // u8 has alignment 1: no tag bits at all.
//! let x = 7u8;
//! let _ = TaggedPtr::<u8, 1>::from_ref(&x, 0);
//! ```
//!
//! ```
//! use o1_core::ptr::TaggedPtr;
//! let x = 7u64; // alignment 8: up to 3 tag bits
//! let p = TaggedPtr::<u64, 3>::from_ref(&x, 0b101).unwrap();
//! assert_eq!(p.tag(), 0b101);
//! // SAFETY: `p` was made from `&x`, which is live and not mutably borrowed.
//! assert_eq!(unsafe { *p.as_ref() }, 7);
//! assert!(TaggedPtr::<u64, 3>::from_ref(&x, 0b1000).is_err()); // tag too wide at run time
//! ```

use core::fmt;
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;
use core::mem::align_of;
use core::num::NonZeroUsize;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicPtr, Ordering};

/// Number of low address bits that are always zero in a well-aligned `*const T`:
/// `align_of::<T>().trailing_zeros()`.
///
/// ```
/// assert_eq!(o1_core::ptr::tag_capacity::<u8>(), 0);
/// assert_eq!(o1_core::ptr::tag_capacity::<u32>(), 2);
/// assert_eq!(o1_core::ptr::tag_capacity::<[u64; 0]>(), 3); // zero-sized, align 8
/// ```
#[inline]
#[must_use]
pub const fn tag_capacity<T>() -> u32 {
    align_of::<T>().trailing_zeros()
}

/// Why a tagged pointer could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagError {
    /// The tag has bits set outside the low `BITS` bits.
    TagTooWide {
        /// The rejected tag.
        tag: usize,
        /// Largest tag that fits (`2^BITS - 1`).
        max: usize,
    },
    /// The pointer's low `BITS` address bits are not zero (the pointer is not aligned
    /// enough to carry a tag). Only possible for raw pointers, never for references.
    Misaligned {
        /// The rejected pointer's address.
        addr: usize,
        /// Required alignment, `2^BITS`.
        required: usize,
    },
}

impl fmt::Display for TagError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::TagTooWide { tag, max } => write!(f, "tag {tag:#x} exceeds maximum {max:#x}"),
            Self::Misaligned { addr, required } => {
                write!(f, "address {addr:#x} is not aligned to {required}")
            }
        }
    }
}

impl core::error::Error for TagError {}

/// A non-null pointer to `T` with a `BITS`-bit tag packed into its low address bits —
/// one machine word.
///
/// * `BITS <= tag_capacity::<T>()` is enforced at compile time (post-monomorphisation
///   error; see the [module docs](self)).
/// * The pointer keeps the provenance of the pointer it was built from: tag changes use
///   [`NonNull::map_addr`]. [`as_ptr`](Self::as_ptr) returns a pointer that can be
///   dereferenced wherever the original could.
/// * Construction is safe: a `TaggedPtr` is a pointer value, not a reference. Its
///   invariants — the tag fits, and the address with the tag cleared is non-zero — are
///   checked by every constructor. Dereferencing ([`as_ref`](Self::as_ref),
///   [`as_mut`](Self::as_mut)) is `unsafe` with exactly `NonNull`'s contract.
/// * Neither `Send` nor `Sync`, like `NonNull<T>`:
///
/// ```compile_fail,E0277
/// fn needs_send<T: Send>() {}
/// needs_send::<o1_core::ptr::TaggedPtr<u64, 3>>();
/// ```
/// ```compile_fail,E0277
/// fn needs_sync<T: Sync>() {}
/// needs_sync::<o1_core::ptr::TaggedPtr<u64, 3>>();
/// ```
/// * `T` must be `Sized` (`align_of` needs a size).
///
/// Every operation is worst-case O(1) work and allocation-free.
#[repr(transparent)]
pub struct TaggedPtr<T, const BITS: u32> {
    /// Address bits `BITS..` are the pointer, bits `..BITS` are the tag.
    /// Invariant: `raw.addr() & !TAG_MASK != 0`.
    raw: NonNull<T>,
    _owns_nothing: PhantomData<*const T>,
}

impl<T, const BITS: u32> TaggedPtr<T, BITS> {
    /// Compile-time check that `BITS` fits in the alignment of `T`. Every constructor
    /// evaluates it, so an invalid `BITS` cannot be instantiated.
    const VALID: () = assert!(
        BITS <= tag_capacity::<T>(),
        "TaggedPtr: BITS exceeds align_of::<T>().trailing_zeros()"
    );

    /// Mask of the tag bits: `2^BITS - 1`.
    pub const TAG_MASK: usize = {
        let () = Self::VALID;
        crate::bitops::usize::mask_low(BITS)
    };

    /// Largest tag value, equal to [`TAG_MASK`](Self::TAG_MASK).
    pub const MAX_TAG: usize = Self::TAG_MASK;

    /// Tags `ptr` with `tag`.
    ///
    /// # Errors
    ///
    /// [`TagError::TagTooWide`] if `tag > MAX_TAG`; [`TagError::Misaligned`] if the low
    /// `BITS` address bits of `ptr` are not zero.
    #[inline]
    pub fn new(ptr: NonNull<T>, tag: usize) -> Result<Self, TagError> {
        let mask = Self::TAG_MASK;
        if tag & !mask != 0 {
            return Err(TagError::TagTooWide { tag, max: mask });
        }
        if ptr.addr().get() & mask != 0 {
            return Err(TagError::Misaligned {
                addr: ptr.addr().get(),
                required: mask + 1,
            });
        }
        // `ptr` is non-null with its low BITS bits clear, so `addr >= 2^BITS` and the
        // address without the tag is the (non-zero) original address. OR-ing in a tag
        // that fits inside the mask keeps it non-zero.
        let raw = ptr.map_addr(|a| a | tag);
        Ok(Self {
            raw,
            _owns_nothing: PhantomData,
        })
    }

    /// Tags a reference. A reference is always aligned, so only the tag can be rejected.
    ///
    /// # Errors
    ///
    /// [`TagError::TagTooWide`] if `tag > MAX_TAG`.
    #[inline]
    pub fn from_ref(r: &T, tag: usize) -> Result<Self, TagError> {
        Self::new(NonNull::from(r), tag)
    }

    /// Tags a mutable reference; the result may be used for writes (see [`as_mut`](Self::as_mut)).
    ///
    /// # Errors
    ///
    /// [`TagError::TagTooWide`] if `tag > MAX_TAG`.
    #[inline]
    pub fn from_mut(r: &mut T, tag: usize) -> Result<Self, TagError> {
        Self::new(NonNull::from(r), tag)
    }

    /// A dangling, well-aligned tagged pointer (like [`NonNull::dangling`]) with tag 0.
    #[inline]
    #[must_use]
    pub const fn dangling() -> Self {
        let () = Self::VALID;
        // `NonNull::dangling()` has address `align_of::<T>()`, whose low BITS bits are
        // zero because BITS <= trailing_zeros(align_of::<T>()) (checked above), and which
        // is non-zero.
        Self {
            raw: NonNull::dangling(),
            _owns_nothing: PhantomData,
        }
    }

    /// The tag.
    #[inline]
    #[must_use]
    pub fn tag(self) -> usize {
        self.raw.addr().get() & Self::TAG_MASK
    }

    /// The pointer with the tag bits cleared, with the original provenance.
    #[inline]
    #[must_use]
    pub fn as_non_null(self) -> NonNull<T> {
        let mask = Self::TAG_MASK;
        // The invariant says the cleared address is non-zero; the `unwrap_or(a)` arm is
        // unreachable and exists only to stay in safe code.
        self.raw
            .map_addr(|a| NonZeroUsize::new(a.get() & !mask).unwrap_or(a))
    }

    /// The pointer with the tag bits cleared, as a raw pointer with the original provenance.
    #[inline]
    #[must_use]
    pub fn as_ptr(self) -> *mut T {
        let mask = Self::TAG_MASK;
        // Raw-pointer form: no non-zero re-check needed, so no fallback instruction
        // (see docs/codegen: `as_non_null` carries a branch-free `cmov`/`csel` for it).
        self.raw.as_ptr().map_addr(|a| a & !mask)
    }

    /// The address with the tag cleared. Does not expose provenance.
    #[inline]
    #[must_use]
    pub fn addr(self) -> NonZeroUsize {
        self.as_non_null().addr()
    }

    /// The whole word (address and tag) as a raw pointer, keeping provenance. Suitable for
    /// storing in an [`AtomicPtr`]; **not** suitable for dereferencing unless the tag is 0.
    #[inline]
    #[must_use]
    pub const fn into_raw(self) -> *mut T {
        self.raw.as_ptr()
    }

    /// Rebuilds a tagged pointer from [`into_raw`](Self::into_raw) output.
    ///
    /// Returns `None` if `raw` is null or its address with the tag cleared is zero.
    /// Safe: the result is only a pointer value; dereferencing remains `unsafe`.
    #[inline]
    #[must_use]
    pub fn from_raw(raw: *mut T) -> Option<Self> {
        let raw = NonNull::new(raw)?;
        if raw.addr().get() & !Self::TAG_MASK == 0 {
            return None;
        }
        Some(Self {
            raw,
            _owns_nothing: PhantomData,
        })
    }

    /// The same pointer with another tag.
    ///
    /// # Errors
    ///
    /// [`TagError::TagTooWide`] if `tag > MAX_TAG`.
    #[inline]
    pub fn with_tag(self, tag: usize) -> Result<Self, TagError> {
        Self::new(self.as_non_null(), tag)
    }

    /// The same pointer with the tag reduced modulo `2^BITS` (high tag bits dropped).
    /// Never fails.
    #[inline]
    #[must_use]
    pub fn with_tag_truncated(self, tag: usize) -> Self {
        let mask = Self::TAG_MASK;
        let raw = self.raw.map_addr(|a| {
            // Clearing the tag leaves the non-zero clean address; OR keeps it non-zero.
            NonZeroUsize::new((a.get() & !mask) | (tag & mask)).unwrap_or(a)
        });
        Self {
            raw,
            _owns_nothing: PhantomData,
        }
    }

    /// Shared reference to the pointee.
    ///
    /// # Safety
    ///
    /// Same as [`NonNull::as_ref`] applied to [`as_non_null`](Self::as_non_null): the
    /// pointer must be valid for reads of `T`, point to an initialised `T`, and the
    /// pointee must not be mutated (except through `UnsafeCell`) for `'a`.
    #[inline]
    #[must_use]
    pub unsafe fn as_ref<'a>(self) -> &'a T {
        // SAFETY: [unsafe:ptr.as-ref] forwarded verbatim from this function's contract.
        unsafe { self.as_non_null().as_ref() }
    }

    /// Mutable reference to the pointee.
    ///
    /// # Safety
    ///
    /// Same as [`NonNull::as_mut`] applied to [`as_non_null`](Self::as_non_null): the
    /// pointer must be valid for reads and writes of `T`, point to an initialised `T`,
    /// carry write permission (e.g. built with [`from_mut`](Self::from_mut)), and no other
    /// reference to the pointee may be used for `'a`.
    #[inline]
    #[must_use]
    pub unsafe fn as_mut<'a>(self) -> &'a mut T {
        let mut p = self.as_non_null();
        // SAFETY: [unsafe:ptr.as-mut] forwarded verbatim from this function's contract.
        unsafe { p.as_mut() }
    }
}

impl<T, const BITS: u32> Clone for TaggedPtr<T, BITS> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}
impl<T, const BITS: u32> Copy for TaggedPtr<T, BITS> {}

impl<T, const BITS: u32> PartialEq for TaggedPtr<T, BITS> {
    /// Equal when both address and tag are equal (provenance is not compared).
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}
impl<T, const BITS: u32> Eq for TaggedPtr<T, BITS> {}

impl<T, const BITS: u32> Hash for TaggedPtr<T, BITS> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.raw.hash(state);
    }
}

impl<T, const BITS: u32> fmt::Debug for TaggedPtr<T, BITS> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TaggedPtr")
            .field("ptr", &self.as_ptr())
            .field("tag", &self.tag())
            .finish()
    }
}

/// An atomic [`TaggedPtr`]: an [`AtomicPtr<T>`] holding the tagged word, so the stored
/// pointer keeps its provenance (unlike packing the word into an `AtomicUsize`).
///
/// Operations are single atomic instructions or a single compare-exchange; each is
/// lock-free where `AtomicPtr` is (every target with `target_has_atomic = "ptr"`), and
/// `load`/`store`/`swap`/`compare_exchange` are each a single atomic operation (no retry
/// loop inside). Tag-only read-modify-write is not provided; a caller's CAS loop over
/// [`compare_exchange_weak`](Self::compare_exchange_weak) is lock-free, not wait-free.
///
/// `Send`/`Sync` follow `AtomicPtr<T>` (both, unconditionally), as the type only moves
/// pointer values; dereferencing a loaded pointer is the caller's `unsafe`.
#[repr(transparent)]
pub struct AtomicTaggedPtr<T, const BITS: u32> {
    inner: AtomicPtr<T>,
}

impl<T, const BITS: u32> AtomicTaggedPtr<T, BITS> {
    /// A new atomic tagged pointer.
    #[inline]
    #[must_use]
    pub const fn new(p: TaggedPtr<T, BITS>) -> Self {
        Self {
            inner: AtomicPtr::new(p.into_raw()),
        }
    }

    #[inline]
    fn wrap(raw: *mut T) -> TaggedPtr<T, BITS> {
        // Every value ever stored came from a `TaggedPtr`, so it satisfies the invariant;
        // the fallback is unreachable and keeps this function safe.
        TaggedPtr::from_raw(raw).unwrap_or_else(TaggedPtr::dangling)
    }

    /// Atomic load.
    #[inline]
    pub fn load(&self, order: Ordering) -> TaggedPtr<T, BITS> {
        Self::wrap(self.inner.load(order))
    }

    /// Atomic store.
    #[inline]
    pub fn store(&self, p: TaggedPtr<T, BITS>, order: Ordering) {
        self.inner.store(p.into_raw(), order);
    }

    /// Atomic swap.
    #[inline]
    pub fn swap(&self, p: TaggedPtr<T, BITS>, order: Ordering) -> TaggedPtr<T, BITS> {
        Self::wrap(self.inner.swap(p.into_raw(), order))
    }

    /// Strong compare-exchange of the whole tagged word (address and tag).
    ///
    /// # Errors
    ///
    /// Returns the current value if it was not `current`.
    #[inline]
    pub fn compare_exchange(
        &self,
        current: TaggedPtr<T, BITS>,
        new: TaggedPtr<T, BITS>,
        success: Ordering,
        failure: Ordering,
    ) -> Result<TaggedPtr<T, BITS>, TaggedPtr<T, BITS>> {
        self.inner
            .compare_exchange(current.into_raw(), new.into_raw(), success, failure)
            .map(Self::wrap)
            .map_err(Self::wrap)
    }

    /// Weak compare-exchange (may fail spuriously; use in a loop).
    ///
    /// # Errors
    ///
    /// Returns the current value if it was not `current`, or spuriously.
    #[inline]
    pub fn compare_exchange_weak(
        &self,
        current: TaggedPtr<T, BITS>,
        new: TaggedPtr<T, BITS>,
        success: Ordering,
        failure: Ordering,
    ) -> Result<TaggedPtr<T, BITS>, TaggedPtr<T, BITS>> {
        self.inner
            .compare_exchange_weak(current.into_raw(), new.into_raw(), success, failure)
            .map(Self::wrap)
            .map_err(Self::wrap)
    }

    /// Consumes the atomic and returns the value.
    #[inline]
    pub fn into_inner(self) -> TaggedPtr<T, BITS> {
        Self::wrap(self.inner.into_inner())
    }
}

impl<T, const BITS: u32> fmt::Debug for AtomicTaggedPtr<T, BITS> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.load(Ordering::Relaxed), f)
    }
}

/// A power-of-two alignment in bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Align(NonZeroUsize);

impl Align {
    /// `Some` if `bytes` is a non-zero power of two.
    #[inline]
    #[must_use]
    pub const fn new(bytes: usize) -> Option<Self> {
        if bytes.is_power_of_two() {
            match NonZeroUsize::new(bytes) {
                Some(n) => Some(Self(n)),
                None => None,
            }
        } else {
            None
        }
    }

    /// The alignment of `T`.
    #[inline]
    #[must_use]
    pub const fn of<T>() -> Self {
        match NonZeroUsize::new(align_of::<T>()) {
            Some(n) => Self(n),
            // align_of is never zero; unreachable.
            None => Self(NonZeroUsize::MIN),
        }
    }

    /// The alignment in bytes.
    #[inline]
    #[must_use]
    pub const fn get(self) -> usize {
        self.0.get()
    }
}

/// Rounds `p`'s address down to a multiple of `align`, keeping provenance.
///
/// Worst-case O(1) work. The result may lie before the start of `p`'s allocation; using it
/// for access is then undefined behaviour, as for any out-of-bounds pointer.
#[inline]
#[must_use]
pub fn align_down<T>(p: *const T, align: Align) -> *const T {
    let mask = align.get() - 1;
    p.map_addr(|a| a & !mask)
}

/// Rounds `p`'s address up to a multiple of `align`, keeping provenance. `None` if that
/// would overflow the address space.
#[inline]
#[must_use]
pub fn align_up<T>(p: *const T, align: Align) -> Option<*const T> {
    let mask = align.get() - 1;
    let up = p.addr().checked_add(mask)? & !mask;
    Some(p.with_addr(up))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_roundtrip_keeps_provenance() {
        let mut x = 41u64;
        let p = TaggedPtr::<u64, 3>::from_mut(&mut x, 0b110).unwrap();
        assert_eq!(p.tag(), 0b110);
        // SAFETY: [unsafe:ptr-test.write-through] built from `&mut x`; `x` is live and not otherwise borrowed here.
        unsafe { *p.as_mut() += 1 };
        let q = p.with_tag(1).unwrap();
        // SAFETY: [unsafe:ptr-test.read-after-retag] same pointer; the mutable use above has ended.
        assert_eq!(unsafe { *q.as_ref() }, 42);
        assert_eq!(q.as_ptr(), core::ptr::addr_of_mut!(x));
    }

    #[test]
    fn rejects_wide_tag_and_misaligned_ptr() {
        let x = [0u32; 2];
        let base = NonNull::from(&x[0]);
        assert_eq!(
            TaggedPtr::<u32, 2>::new(base, 4),
            Err(TagError::TagTooWide { tag: 4, max: 3 })
        );
        let odd = base.map_addr(|a| a.checked_add(1).unwrap());
        assert!(matches!(
            TaggedPtr::<u32, 2>::new(odd, 0),
            Err(TagError::Misaligned { required: 4, .. })
        ));
    }

    #[test]
    fn zero_bits_and_zst() {
        let unit = ();
        let p = TaggedPtr::<(), 0>::from_ref(&unit, 0).unwrap();
        assert_eq!(p.tag(), 0);
        assert!(TaggedPtr::<(), 0>::from_ref(&unit, 1).is_err());
        let z: [u64; 0] = [];
        let q = TaggedPtr::<[u64; 0], 3>::from_ref(&z, 7).unwrap();
        assert_eq!(q.tag(), 7);
        // SAFETY: [unsafe:ptr-test.zst-ref] zero-sized reads through a non-null aligned pointer are valid.
        let r: &[u64; 0] = unsafe { q.as_ref() };
        assert_eq!(r.len(), 0);
        let d = TaggedPtr::<[u64; 0], 3>::dangling().with_tag_truncated(0xff);
        assert_eq!(d.tag(), 7);
        assert_eq!(d.addr().get(), 8);
    }

    #[test]
    fn raw_roundtrip_and_atomic() {
        let x = 5u32;
        let p = TaggedPtr::<u32, 2>::from_ref(&x, 3).unwrap();
        let back = TaggedPtr::<u32, 2>::from_raw(p.into_raw()).unwrap();
        assert_eq!(back, p);
        assert!(TaggedPtr::<u32, 2>::from_raw(core::ptr::null_mut()).is_none());
        // A raw word whose clean address is zero is rejected.
        assert!(TaggedPtr::<u32, 2>::from_raw(core::ptr::without_provenance_mut(3)).is_none());

        let a = AtomicTaggedPtr::new(p);
        let p2 = p.with_tag(1).unwrap();
        assert_eq!(
            a.compare_exchange(p, p2, Ordering::AcqRel, Ordering::Acquire),
            Ok(p)
        );
        assert_eq!(
            a.compare_exchange(p, p2, Ordering::AcqRel, Ordering::Acquire),
            Err(p2)
        );
        let loaded = a.load(Ordering::Acquire);
        assert_eq!(loaded.tag(), 1);
        // SAFETY: [unsafe:ptr-test.atomic-roundtrip-read] provenance survived the atomic round trip; `x` is live.
        assert_eq!(unsafe { *loaded.as_ref() }, 5);
        assert_eq!(a.swap(p, Ordering::AcqRel), p2);
        assert_eq!(a.into_inner(), p);
    }

    #[test]
    fn align_helpers() {
        let buf = [0u8; 64];
        let p = buf.as_ptr();
        let a16 = Align::new(16).unwrap();
        let up = align_up(p.wrapping_add(1), a16).unwrap();
        assert_eq!(up.addr() % 16, 0);
        assert!(up.addr() > p.addr());
        let down = align_down(p.wrapping_add(17), a16);
        assert_eq!(down.addr() % 16, 0);
        assert!(Align::new(0).is_none());
        assert!(Align::new(12).is_none());
        assert_eq!(Align::of::<u64>().get(), align_of::<u64>());
        let top = core::ptr::without_provenance::<u8>(usize::MAX);
        assert!(align_up(top, a16).is_none());
    }
}
