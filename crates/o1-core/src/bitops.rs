//! Bit operations with an ISA-independent contract.
//!
//! One module per unsigned width ([`u8`](mod@u8), [`u16`](mod@u16), [`u32`](mod@u32), [`u64`](mod@u64), [`u128`](mod@u128), [`usize`](mod@usize)),
//! each with the same `const fn`s, generated from one definition so the contract is written
//! once. Functions are named for what they compute, not for an instruction.
//!
//! # Contract (every width; `BITS` is the width)
//!
//! | function | result | zero input / out-of-range argument |
//! |---|---|---|
//! | `trailing_zero_count(x)` | trailing zero bits | `x == 0` → `BITS` |
//! | `leading_zero_count(x)` | leading zero bits | `x == 0` → `BITS` |
//! | `first_set(x)` | `Some(index of lowest set bit)` | `x == 0` → `None` |
//! | `last_set(x)` | `Some(index of highest set bit)` | `x == 0` → `None` |
//! | `pop_count(x)` | set bits | — |
//! | `mask_low(w)` | low `w` bits set | `w >= BITS` → all ones |
//! | `bit_test(x, i)` | bit `i` | `i >= BITS` → `false` |
//! | `bit_set(x, i)`, `bit_clear(x, i)` | `x` with bit `i` set / cleared | `i >= BITS` → `x` unchanged |
//! | `clear_lowest(x)` | `x` without its lowest set bit | `0` → `0` |
//! | `isolate_lowest(x)` | only the lowest set bit of `x` | `0` → `0` |
//! | `select(c, a, b)` | `a` if `c` else `b` | — |
//! | `select_masked(c, a, b)` | same value as `select`, computed with masks | — |
//! | `set_bits(x)` | iterator over set-bit indices, ascending | `0` → empty |
//!
//! No function panics, for any input, in debug or release builds.
//!
//! # Guarantees and what is not claimed
//!
//! Every function except iteration does worst-case O(1) work and is allocation-free.
//! [`SetBits`](u64::SetBits) yields `pop_count(x)` items, each in O(1).
//!
//! The implementation is ordinary `core` (`trailing_zeros`, `leading_zeros`, `count_ones`,
//! `checked_shl`). Which instructions that becomes is the compiler's choice; the
//! repository's `scripts/codegen.sh` records it for x86_64 (baseline and `+bmi1,+lzcnt`)
//! and aarch64 in `docs/codegen/`. Nothing here is claimed to be branchless, to compile
//! to a particular instruction, or to be constant-time in the cryptographic sense.
//! `select_masked` is written without a conditional, but the compiler may still emit a
//! branch or a conditional move for either form; see the codegen record.
//!
//! ```
//! use o1_core::bitops::u64 as b;
//!
//! assert_eq!(b::trailing_zero_count(0), 64);
//! assert_eq!(b::first_set(0b1000), Some(3));
//! assert_eq!(b::first_set(0), None);
//! assert_eq!(b::mask_low(3), 0b111);
//! assert_eq!(b::mask_low(200), u64::MAX);
//! assert!(!b::bit_test(u64::MAX, 64));
//! assert_eq!(b::set_bits(0b1010_0001).collect::<Vec<_>>(), [0, 5, 7]);
//! ```

macro_rules! width_module {
    ($(#[$meta:meta])* $name:ident, $t:ty) => {
        $(#[$meta])*
        pub mod $name {
            /// Width of this integer type in bits.
            pub const BITS: u32 = <$t>::BITS;

            /// Number of trailing zero bits. Returns [`BITS`] for `0`.
            #[inline]
            #[must_use]
            pub const fn trailing_zero_count(x: $t) -> u32 {
                x.trailing_zeros()
            }

            /// Number of leading zero bits. Returns [`BITS`] for `0`.
            #[inline]
            #[must_use]
            pub const fn leading_zero_count(x: $t) -> u32 {
                x.leading_zeros()
            }

            /// Index of the lowest set bit, or `None` for `0`.
            #[inline]
            #[must_use]
            pub const fn first_set(x: $t) -> Option<u32> {
                if x == 0 { None } else { Some(x.trailing_zeros()) }
            }

            /// Index of the highest set bit, or `None` for `0`.
            #[inline]
            #[must_use]
            pub const fn last_set(x: $t) -> Option<u32> {
                if x == 0 { None } else { Some(BITS - 1 - x.leading_zeros()) }
            }

            /// Number of set bits.
            #[inline]
            #[must_use]
            pub const fn pop_count(x: $t) -> u32 {
                x.count_ones()
            }

            /// A value with the low `width` bits set. Saturates: `width >= BITS` gives all ones.
            ///
            /// Never performs an overflowing shift.
            #[inline]
            #[must_use]
            pub const fn mask_low(width: u32) -> $t {
                match (1 as $t).checked_shl(width) {
                    Some(bit) => bit.wrapping_sub(1),
                    None => <$t>::MAX,
                }
            }

            /// Whether bit `index` of `x` is set. `index >= BITS` is `false`.
            #[inline]
            #[must_use]
            pub const fn bit_test(x: $t, index: u32) -> bool {
                match x.checked_shr(index) {
                    Some(shifted) => shifted & 1 == 1,
                    None => false,
                }
            }

            /// `x` with bit `index` set. `index >= BITS` returns `x` unchanged.
            #[inline]
            #[must_use]
            pub const fn bit_set(x: $t, index: u32) -> $t {
                match (1 as $t).checked_shl(index) {
                    Some(bit) => x | bit,
                    None => x,
                }
            }

            /// `x` with bit `index` cleared. `index >= BITS` returns `x` unchanged.
            #[inline]
            #[must_use]
            pub const fn bit_clear(x: $t, index: u32) -> $t {
                match (1 as $t).checked_shl(index) {
                    Some(bit) => x & !bit,
                    None => x,
                }
            }

            /// `x` with its lowest set bit cleared (`x & (x - 1)`); `0` stays `0`.
            #[inline]
            #[must_use]
            pub const fn clear_lowest(x: $t) -> $t {
                x & x.wrapping_sub(1)
            }

            /// Only the lowest set bit of `x` (`x & -x`); `0` stays `0`.
            #[inline]
            #[must_use]
            pub const fn isolate_lowest(x: $t) -> $t {
                x & x.wrapping_neg()
            }

            /// `a` if `condition`, else `b`. The reference form of selection.
            #[inline]
            #[must_use]
            pub const fn select(condition: bool, a: $t, b: $t) -> $t {
                if condition { a } else { b }
            }

            /// Same result as [`select`], computed as `b ^ ((a ^ b) & mask)` where `mask` is
            /// all ones when `condition` holds. Written without a conditional; this is a
            /// source-level property only (see the module docs).
            #[inline]
            #[must_use]
            pub const fn select_masked(condition: bool, a: $t, b: $t) -> $t {
                let mask = (condition as $t).wrapping_neg();
                b ^ ((a ^ b) & mask)
            }

            /// Iterator over the indices of the set bits of `x`, lowest first.
            #[inline]
            #[must_use]
            pub const fn set_bits(x: $t) -> SetBits {
                SetBits { rest: x }
            }

            /// Iterator over set-bit indices, lowest first. See [`set_bits`].
            ///
            /// Yields exactly `pop_count(x)` items; each `next` is O(1) work.
            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub struct SetBits {
                rest: $t,
            }

            impl Iterator for SetBits {
                type Item = u32;

                #[inline]
                fn next(&mut self) -> Option<u32> {
                    let index = first_set(self.rest)?;
                    self.rest = clear_lowest(self.rest);
                    Some(index)
                }

                #[inline]
                fn size_hint(&self) -> (usize, Option<usize>) {
                    let n = self.rest.count_ones() as usize;
                    (n, Some(n))
                }
            }

            impl ExactSizeIterator for SetBits {}
            impl core::iter::FusedIterator for SetBits {}
        }
    };
}

width_module!(
    /// Bit operations on `u8`.
    u8,
    u8
);
width_module!(
    /// Bit operations on `u16`.
    u16,
    u16
);
width_module!(
    /// Bit operations on `u32`.
    u32,
    u32
);
width_module!(
    /// Bit operations on `u64`.
    u64,
    u64
);
width_module!(
    /// Bit operations on `u128`.
    u128,
    u128
);
width_module!(
    /// Bit operations on `usize`.
    usize,
    usize
);
