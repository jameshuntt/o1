# Guarantees per public operation

Vocabulary (never interchanged):
**O(1)** = worst-case O(1) algorithmic work · **BS** = bounded-step / fixed-capacity ·
**AF** = allocation-free · **WF** = wait-free · **LF** = lock-free · **BL** = branchless
(machine code, per the codegen record) · **ISA** = ISA/intrinsic-backed ·
**CT** = constant-time in the cryptographic sense (secret-independent timing) ·
**LAT** = bounded wall-clock latency.

**CT is claimed for nothing in o1. LAT is claimed for nothing in o1.** "BL" is only stated
where `docs/codegen/RESULTS` checks it, and only for the listed targets and flags.
WF/LF apply only to operations that can run concurrently with another thread.

Legend: ✓ = holds (evidence in `O1_SUBSTRATE_IMPLEMENTATION_REPORT.md`), — = not applicable,
✗ = does not hold.

## o1-core::bitops (every width)

| operation | O(1) | BS | AF | BL | ISA | notes |
|---|---|---|---|---|---|---|
| `trailing_zero_count`, `leading_zero_count` | ✓ | ✓ | ✓ | ✓ (x86_64 base/BMI, aarch64, u64/u32) | single scan instruction on those targets (`rep bsf`/`tzcnt`, `bsr`/`lzcnt`, `rbit`+`clz`, `clz`) | from ordinary `core` calls; no intrinsics in source |
| `first_set`, `last_set` | ✓ | ✓ | ✓ | ✓ (u64, those targets) | as above | `None` for 0 |
| `pop_count` | ✓ | ✓ | ✓ | ✓ | `popcnt` only with `+popcnt`; baseline x86_64 is a bit-twiddling sequence; aarch64 `cnt` (SIMD) | |
| `mask_low`, `bit_test` | ✓ | ✓ | ✓ | ✓ (u64, those targets) | — | out-of-range defined (saturate / false) |
| `bit_set`, `bit_clear`, `clear_lowest`, `isolate_lowest` | ✓ | ✓ | ✓ | not checked except `isolate_lowest` (BMI `blsi`) | — | |
| `select` | ✓ | ✓ | ✓ | ✓ (`cmov` / `csel`) | — | |
| `select_masked` | ✓ | ✓ | ✓ | ✓ (folded by LLVM into the same code as `select`) | — | kept as the differential reference form only |
| `SetBits::next` | ✓ per item | ✓ | ✓ | not checked | — | iteration total O(popcount) |

## o1-core::ptr

| operation | O(1) | AF | WF | LF | BL | notes |
|---|---|---|---|---|---|---|
| `TaggedPtr::new/from_ref/from_mut/with_tag/from_raw` | ✓ | ✓ | — | — | not checked | return `Err`/`None` on invalid input |
| `tag`, `as_ptr`, `into_raw`, `with_tag_truncated` | ✓ | ✓ | — | — | ✓ `tag`, `as_ptr` (one `and`) | |
| `as_non_null`, `addr` | ✓ | ✓ | — | — | ✓ but with one extra `cmov`/`csel` for the safe non-zero fallback | see codegen |
| `as_ref`, `as_mut` (unsafe) | ✓ | ✓ | — | — | — | |
| `AtomicTaggedPtr::load/store/swap` | ✓ | ✓ | ✓ where `AtomicPtr` is lock-free | ✓ | — | |
| `AtomicTaggedPtr::compare_exchange(_weak)` | ✓ | ✓ | ✓ (single CAS, no internal retry) | ✓ | — | a caller's retry loop is LF, not WF |
| `align_down`, `align_up` | ✓ | ✓ | — | — | not checked | `align_up` → `None` on overflow |

## o1-core::collections, o1-core::arena

| operation | O(1) | BS | AF | notes |
|---|---|---|---|---|
| `FixedRing::push_back/pop_front/front/get/get_mut/len` | ✓ | ✓ (N) | ✓ | full → `Err(value)` |
| `FixedRing::iter` | ✗ O(len) | ✓ | ✓ | |
| `FixedRing::clear`, `Drop` | ✗ O(len) | ✓ | ✓ | |
| `SparseSet::insert/remove/contains/len/clear` | ✓ | ✓ (N) | ✓ | `clear` is O(1) |
| `SparseSet::new` | ✗ O(N) | ✓ | ✓ | zero-fills both arrays |
| `SparseSet::iter/as_slice` iteration | ✗ O(len) | ✓ | ✓ | |
| `FreeList::insert/remove/get/get_mut/len` | ✓ | ✓ (N) | ✓ | full → `Err(value)` |
| `GenerationalArena::insert/remove/get/get_mut/contains/len` | ✓ | ✓ (N) | ✓ | stale handle → `None` in O(1) |
| `FreeList/GenerationalArena::new` | ✗ O(N) | ✓ | ✓ | |
| `FreeList/GenerationalArena::iter`, `Drop` | ✗ O(N) | ✓ | ✓ | visits every slot up to the bump mark |

None of these is thread-safe for concurrent mutation; they are `Send`/`Sync` exactly when
`T` is (auto traits).

## o1-core::frame

| operation | O(1) | AF | notes |
|---|---|---|---|
| `FrameDecoder::spare/filled/frame/release` | ✓ (header bytes: ≤ 4-byte copy) | ✓ | bytes stored bounded by the caller's buffer; discard work per frame bounded by `discard_limit` |
| `FrameEncoder::new/pending/advance` | ✓ | ✓ | payload borrowed, never copied |

## o1-sync

| operation | O(1) | AF | WF | LF | LAT |
|---|---|---|---|---|---|
| `Producer::push`, `Consumer::pop` | ✓ | ✓ | ✓ (no loop, no CAS; ≤ 1 acquire load + 1 release store) | ✓ | ✗ |
| `Producer::free_slots`, `Consumer::available` | ✓ | ✓ | ✓ | ✓ | ✗ |
| `Ring::split` | ✓ | ✓ | — (`&mut`) | — | — |
| `Ring` `Drop` | ✗ O(items) | ✓ | — | — | — |

## o1-linux

| operation | O(1) | AF | WF | LAT | notes |
|---|---|---|---|---|---|
| `errno::decode`, `Errno::*` | ✓ | ✓ | — | — | pure |
| `io::read/write/send_vectored` | ✗ (kernel) | ✓ (userspace) | ✗ | ✗ | one syscall each |
| `futex::wait`, `futex::wake` | ✗ (kernel) | ✓ (userspace) | ✗ | ✗ | one syscall each; parking/waking is scheduler-dependent |
| `futex::wait_while_equal` fast path (value already differs) | ✓ (one Acquire load) | ✓ | ✓ | ✗ | |
| `futex::wait_while_equal` slow path | ✗ unbounded loop | ✓ | ✗ | ✗ | returns only after another thread changes the word |
| `uds::recv_frame`, `uds::send_frame` | ✗ | ✓ | ✗ | ✗ | `EINTR` retried (unbounded by code); `EAGAIN` → `WouldBlock` |

## o1-core::stream (unit 2)

| operation | O(1) | BS | AF | notes |
|---|---|---|---|---|
| `BlockPass::new`, `with` | ✓ | ✓ | ✓ | zero-sized; `N = 0` is a compile-time error |
| `BlockPass::read`, `transform` | ✗ O(len / N) whole blocks | ✓ | ✓ | each whole block handed to the callback once, in order; the `len % N` remainder is reported, not processed; `transform` with unequal lengths → `Err(LengthMismatch)`, nothing processed |

The policy type parameter changes the published contract, not the code (codegen record).

## o1-contract (unit 2)

Every function is `const fn` or a plain trait method, pure, `no_std`, allocation-free and
without `unsafe`. Envelope arithmetic is checked: overflow → `Err(Overflow { quantity })`,
never a wrapped or saturated value. `split_blocks` and `lines_touched` are total.

## Execution contracts published by primitives (unit 2)

Worst case per call; counts, no weights. Details in `docs/EXECUTION_CONTRACTS.md`.

| contract | bound | progress | ops | bytes R / W | atomic L / S / RMW | shared R / W | working set | sharing / access | evidence |
|---|---|---|---|---|---|---|---|---|---|
| `spsc::Ring::PUSH_CONTRACT` | Constant | WaitFree | 1 | 0 / `size_of::<T>()` | ≤1 / 1 / 0 | ≤1 / 2 | `size_of::<Ring<T, N>>()` | Handoff / Sequential; `Padded` control words | counted (TESTED) |
| `spsc::Ring::POP_CONTRACT` | Constant | WaitFree | 1 | `size_of::<T>()` / 0 | ≤1 / 1 / 0 | ≤2 / 1 | same | same | counted (TESTED) |
| `GenerationalArena::INSERT_CONTRACT` | Constant | NoSynchronization | 1 | 0 / `size_of::<T>()` | 0 | 0 | `size_of::<Self>()` | Exclusive / Random | read from code (ASSUMED) |
| `GenerationalArena::REMOVE_CONTRACT` | Constant | NoSynchronization | 1 | `size_of::<T>()` / 0 | 0 | 0 | same | Exclusive / Random | ASSUMED |
| `GenerationalArena::GET_CONTRACT` | Constant | NoSynchronization | 1 | 0 / 0 | 0 | 0 | same | SharedRead / Random | ASSUMED |
| `FrameDecoder::frame_contract()` (one frame of `len` bytes) | Capacity (Byte, `max(max_len, discard_limit)`) | NoSynchronization | ≤ 4 + len | 0 / 0 | 0 | 0 | 4 + buffer length | Exclusive / Sequential | counted (TESTED) |
| `BlockPass::READ_CONTRACT` (per whole block) | PerUnit (Block N) | NoSynchronization | 1 | N / 0 | 0 | 0 | N (`Reuse::Once`) or N × blocks | SharedRead / Sequential | counted (TESTED) |
| `BlockPass::TRANSFORM_CONTRACT` (per whole block) | PerUnit (Block N) | NoSynchronization | 1 | N / N | 0 | 0 | 2N (`Reuse::Once`) or 2N × blocks | Exclusive / Sequential | counted (TESTED) |

No contract, deliberately: `bitops`, `TaggedPtr`, `AtomicTaggedPtr`, `CachePadded`,
`FreeList`, `FixedRing`, `SparseSet`, `FrameEncoder`, all of `o1-linux`
(`docs/EXECUTION_CONTRACTS.md` §16). No contract claims CT or LAT; none can.
