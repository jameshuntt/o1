# Safety model

## Rules

1. **Safe by default.** `unsafe` exists only where a primitive cannot be expressed
   otherwise: `MaybeUninit` slot storage (`FixedRing`, `spsc::Ring`), dereferencing a tagged
   pointer, the cross-thread slot handoff (`unsafe impl Sync for Ring`), and the Linux syscall
   boundary. `FreeList`, `GenerationalArena`, `SparseSet`, `frame`, `bitops` contain none.
2. **Containment.** `o1` (façade) is `#![forbid(unsafe_code)]`; so are `tools/xtask` and
   `tools/codegen-probe`. All `asm!` is in one private module, `o1_linux::syscall`, never
   exported; every public `o1-linux` function is safe except `BorrowedFd::borrow_raw`
   (I/O safety, std's convention).
3. **Documentation at the site.** Every `unsafe` block/impl has a `// SAFETY: [unsafe:<id>]`
   comment naming the invariant it relies on; every `unsafe fn` has a `# Safety` section.
   Enforced by `clippy::undocumented_unsafe_blocks = deny`, `clippy::missing_safety_doc = deny`
   (workspace lints) and by the inventory checker (`cargo run -p xtask -- unsafe-inventory`),
   which also requires each id to appear in `docs/UNSAFE_INVENTORY.md`.
4. **`unsafe_op_in_unsafe_fn = deny`**: the body of an `unsafe fn` is not an implicit
   unsafe block.
5. **Strict provenance only.** No `as` casts between pointers and integers, no
   `expose_provenance` / `with_exposed_provenance`; addresses are manipulated with
   `map_addr` / `with_addr`; integers become pointers only via `without_provenance(_mut)`
   (syscall integer arguments). Miri runs with `-Zmiri-strict-provenance`.
6. **Compile-time rejection over run-time checks** where the type carries the information:
   ring capacities (power of two), tag width vs alignment, `u32` index capacity. These are
   inline/associated `const` assertions (post-monomorphisation errors), tested by
   `compile_fail,E0080` doc-tests. Value-dependent conditions (tag too wide, misaligned raw
   pointer, full container) are `Result`s that give the value back.

## Invariants by type

| type | invariant | how it is maintained |
|---|---|---|
| `TaggedPtr<T, B>` | `B ≤ align_of::<T>().trailing_zeros()`; address with tag cleared ≠ 0; tag in low `B` bits | const assert in every constructor; checked constructors; tag only changed through `map_addr` with masked values |
| `AtomicTaggedPtr<T, B>` | holds only words produced by `TaggedPtr::into_raw` | all stores take `TaggedPtr` |
| `FixedRing<T, N>` | slots `head..tail` (mod `N`) initialised, others not; `tail − head ≤ N` | private fields; every mutation keeps it; `Drop`/`clear` mark empty before running destructors |
| `spsc::Ring<T, N>` | as `FixedRing`, plus: only one producer and one consumer (`split(&mut self)`); producer writes outside `head..tail`, consumer reads inside | borrow checker; Release/Acquire pairs (see `o1_sync::spsc` docs) |
| `GenerationalArena<T, N>` | a slot's generation never decreases and never wraps | `checked_add`; exhausted slots become `Retired` |
| `FrameDecoder` | never writes past its buffer; `spare()` is within the buffer; an error is sticky | state machine; `filled(n)` rejects `n` larger than offered (`Overfill`) |
| `IoSlice`, `MsgHdr`, `Timespec` | kernel ABI layout | `#[repr(C)]` + `const` size/offset assertions |

## Panic safety

* `FixedRing::clear` pops one element at a time; a panicking destructor leaves the ring
  consistent with the remaining elements still owned (tested).
* `FixedRing` and `spsc::Ring` `Drop`: if a destructor panics, the remaining elements are
  leaked, never dropped twice (tested for `FixedRing`).
* `GenerationalArena`/`FreeList` `Drop` is the compiler's array drop glue, which continues
  dropping the remaining elements after a panic (tested: all three destructors ran).
* No o1 function panics on any input except where documented; `bitops` functions are
  total (no overflowing shifts in any build profile).

## What the verification does and does not cover

* Miri (strict provenance; Stacked Borrows and, separately, Tree Borrows) runs the portable
  code of `o1-core` and `o1-sync`. Miri finds undefined behaviour **on the executions it
  runs**; it does not prove absence of UB.
* Loom explores interleavings of the SPSC models (C11 memory model as Loom implements it);
  each of the four orderings weakened to `Relaxed` is caught (`scripts/loom_mutations.sh`).
  Loom does not model every hardware behaviour, and the models are small (≤ 3 items).
* `asm!` is not checked by Miri or Loom. The syscall wrappers are checked by review against
  `linux-raw-sys`/`rustix`, by `const` layout assertions, by the codegen record
  (`docs/codegen/o1-linux-*.s`), and — only when CI runs on Linux — by the Linux tests.
