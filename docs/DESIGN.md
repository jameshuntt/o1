# o1 — design

Written in Phase 1, before any implementation code, after the audit recorded below.
Where this differs from `docs/ARCHITECTURE_BRIEF.md`, this document wins and says why.

## 1. Purpose and non-goals

o1 is a small substrate for fixed-capacity, bounded-work, pointer-, concurrency- and
OS-ABI-sensitive primitives. Every low-level assumption (representation, provenance,
memory ordering, architecture, syscall ABI) is stated next to the code that relies on it
and is checked by the cheapest mechanism that can check it: the type system first, then
`const` assertions, then tests (unit, property, Miri, Loom), then codegen inspection.

Non-goals: "assembly everywhere"; general-purpose containers; async runtimes; wrapping
all of Linux. A primitive is in o1 only if a higher crate would otherwise need its own
`unsafe` or its own copy of a subtle invariant.

## 2. Audit of the installed toolchain (rustc 1.97.1, std source under the sysroot)

Checked against `$(rustc --print sysroot)/lib/rustlib/src/rust/library/core/src`, not memory.

| API | stable since | used by o1? |
|---|---|---|
| `<*T>::addr`, `with_addr`, `map_addr`; `ptr::without_provenance(_mut)`, `ptr::dangling(_mut)` | 1.84 | yes (`ptr`) |
| `NonNull::addr`, `NonNull::with_addr`, `NonNull::map_addr` | 1.84 | yes |
| `expose_provenance` / `with_exposed_provenance` | 1.84 | **never** (o1 forbids exposed provenance; strict only) |
| `NonNull::without_provenance`, `NonNull::expose_provenance` | 1.89 | no (above MSRV; not needed) |
| `AtomicPtr::fetch_or` / `fetch_and` / `fetch_ptr_add` (strict-provenance atomic ptr ops) | 1.91 | no (above MSRV; `AtomicTaggedPtr` uses CAS on whole pointers instead) |
| `uN::unbounded_shl` / `unbounded_shr` | 1.87 | no (above MSRV; `mask_low` handles width ≥ BITS explicitly) |
| `uN::is_multiple_of` | 1.87 | no |
| `core::hint::select_unpredictable` | 1.88 | no (above MSRV; see bitops §5.1) |
| `uN::isolate_lowest_one` / `highest_one` / `lowest_one` | 1.97 | no (above MSRV; equivalent written in plain arithmetic) |
| inline `const { … }` blocks (incl. referencing generics) | 1.79 | yes (compile-time capacity / tag checks) |
| `[const { MaybeUninit::uninit() }; N]` | 1.79 | yes |
| `ptr::is_aligned` | 1.79 | yes |
| `core::arch::asm!` | 1.59 | only `o1-linux::syscall` |
| edition 2024 (`unsafe extern`, `unsafe(no_mangle)`, `unsafe_op_in_unsafe_fn` warn-by-default) | 1.85 | yes |
| Cargo `resolver = "3"` (rust-version aware), `[workspace.lints]` | 1.84 / 1.74 | yes |

**MSRV decision: 1.85** (the brief's position stands). Strict provenance (1.84) and
edition 2024 (1.85) are the two hard floors. Every newer API in the table above is avoided
deliberately and the substitute is noted. MSRV applies to the library code; the dev-only
tools need newer compilers (`proptest` 1.12 declares 1.88, `criterion` 0.8 declares 1.86),
so the test suite itself runs on current stable. `cargo +1.85 check --workspace` is the
MSRV gate.

## 3. Audit of crates.io prior art (versions checked 2026-11-17 on crates.io)

| crate | version | decision | reason |
|---|---|---|---|
| `heapless` | 0.9.3 | prior art only | `spsc::Queue<T, N>` holds N-1 items, indexes with modulo, no padding between head/tail, no cached opposite index, no Loom model; pulls `hash32` + `stable_deref_trait`. o1 wants full power-of-two capacity, mask indexing, free-running counters and a Loom model of exactly the code that ships. |
| `rtrb` | 0.4.1 | prior art only | Good SPSC design (cached indices, `CachePadded`), but heap-allocated (`Arc`, runtime capacity) and not usable without `alloc`. The cached-index idea is adopted. |
| `crossbeam-utils` | 0.8.23 | prior art only | `CachePadded` alignment table (128 on x86_64/aarch64, 64 elsewhere, 32 on arm/thumb…) is the reference for o1's own 30-line `CachePadded`; a dependency for one struct is sprawl. |
| `portable-atomic` | 1.15.0 | reject | o1-sync only needs `AtomicUsize` load/store, which every target with `target_has_atomic = "ptr"` has, including thumbv7em. No polyfill needed. |
| `rustix` | 1.1.6 | prior art only | Its `linux_raw` backend is the reference for the asm register conventions and `options(nostack, preserves_flags)`; as a dependency it is far larger than the five syscalls o1 needs. |
| `linux-raw-sys` | 0.12.1 | prior art only (cross-check) | Every syscall number, errno, flag and struct layout o1-linux defines was cross-checked against `linux-raw-sys` 0.12.1 source for both x86_64 and aarch64 (table in §7). A dependency would add ~3 MB of generated source for ~20 constants. |
| `syscalls` | 0.8.1 | reject | Same asm options as rustix; brings `serde`/`serde_repr` as optional deps and a full syscall table. |
| `linux-syscall` | 1.2.0 | not read | Brief listed it; rustix and syscalls already cover the prior-art question. |
| `slotmap` | 1.1.1 (1.0.7 read) | prior art only | Heap-backed; `version` wraps with `wrapping_add` (ABA after 2^31 reuses of one slot). o1 retires exhausted slots instead. |
| `tagptr` | 0.2.0 | reject | Edition 2018, `as usize` casts (pre-strict-provenance), no align-derived tag capacity. |
| `sptr` | 0.3.2 | reject | Polyfill for strict provenance; stable since 1.84 ≤ MSRV. |
| `static_assertions` | 1.1.0 | reject | Inline `const { assert!(…) }` plus a 5-line local helper covers it. |
| `loom` | 0.7.3 | dev-dependency (o1-sync, behind `cfg(loom)`) | Model checker for the SPSC orderings. |
| `proptest` | 1.12.0 | dev-dependency | Property tests: reference models for ring/arena/sparse set/framing, bitops identities. |
| `criterion` | 0.8.2 | dev-dependency (benches only) | Benchmarks. |

**Runtime dependencies: none** in any o1 crate.

## 4. Audit of the owner's related crates (read-only; none modified)

Sources: local clones `~/James/Code/big-bad-bool` (ef017e2), `~/James/Code/scope` (088cb43);
cargo registry: petr-core 0.2.0, petr-collections 0.2.0, petr-dispatch 0.2.0, waker_tables 0.4.0,
flaga_solver 0.1.1, mine 0.3.2, abut 0.2.2; downloaded into `target/reference/`: petr 0.1.0,
petr-spatial 0.2.0, flaga 0.3.2, flaga_core 0.3.1, allot 0.2.0; local experiment
`~/James/Code/attempts2/tagged_ptr_kit` (listed, not studied in depth).

| crate | what was found that bears on o1 |
|---|---|
| petr-core 0.2.0 | `TaggedNonNull<T, LOW_BITS>` stores the clean `NonNull<T>` **and** the tag as two fields (two words). `LOW_BITS` is chosen by the caller and capped at 12; it is **not** derived from `align_of::<T>()`, so a misaligned pointer is a runtime error, not a type error. Address work uses `ptr as usize` (exposes provenance) and `from_word` builds provenance-free pointers. `AtomicTaggedWord` is an `AtomicUsize` — the pointer's provenance is lost when stored. |
| petr-collections 0.2.0 | `KeyedArena`, `TaggedSlab`, `PagedKeyedArena`: `Vec`/`Box`-backed, one global `next_generation: u32` that wraps (`wrapping_add(1).max(1)`), so a handle can be re-validated after 2^32 inserts. No fixed-capacity variant. Keys/selectors/prophecy bits are petr's own concepts layered on top of a slot + generation core. |
| petr-dispatch 0.2.0 / petr-spatial 0.2.0 / petr 0.1.0 | Dispatch tables and Morton routing over `SelectorWord`; `petr` is a re-export façade. No substrate-level unsafe; nothing for o1. |
| big-bad-bool (core, derive) | The derive macro emits the bit math; runtime surface is `u128` masks and schema metadata. No `trailing_zeros`/`leading_zeros`/`count_ones` anywhere in the tree. |
| scope 0.4.0 | `#![deny(unsafe_code)]`; mailboxes are `Arc<Mutex<VecDeque>>` and are MPSC (many senders). Bounded mailbox waiters need wake-ups, not a lock-free queue. Uses its own `cfg(loom)` switch module — same pattern o1-sync adopts. |
| waker_tables 0.4.0 | Hand-built `RawWakerVTable`s around `Arc`; `std`, registry behind `RwLock`. No ring/queue. |
| flaga 0.3.2 / flaga_core 0.3.1 | `ModuleFlags { flags: u64 }` with `1 << index` (index ≥ 64 panics in debug / is masked in release) — exactly the out-of-range case o1's `bit_set`/`bit_test` define. |
| flaga_solver 0.1.1 | `#![no_std] #![forbid(unsafe_code)]`, checks big-bad-bool schemas, `Vec<Refusal>` output. No bit scanning. |
| abut 0.2.2 | Wire format: little-endian `u32` length prefix + payload. Blocking `read_exact`/`write_all` over `std::io`; oversize frames refused and optionally drained so the stream stays aligned; `recv_into(&mut Vec)` resizes to the frame length (allocation bounded by `max_frame_len`). Optional `ring-heapless` = `heapless::spsc` of whole frames. `#![forbid(unsafe_code)]`. |
| mine 0.3.2 | Unix-socket path hygiene (refuse non-socket files, chmod, guard) and peer credentials; framing delegated to abut. |
| allot 0.2.0 | `std` memory domains, bump `Arena` with drop records (`Vec`), `RawBuf`, page pools; its own unsafe around `Layout`. Heap-oriented; overlaps o1 only in vocabulary. |
| tagged_ptr_kit (attempts2) | Earlier experiment that petr-core grew from (same `layout.rs`/`residual.rs` split). Listed, not read line by line. |
| `tagged_ptr_core` | Not on crates.io and no local source found under `~/James/Code`: **not read**. |

Consequences for the boundary (details in `docs/ECOSYSTEM_INTEGRATION.md`):

* o1 provides the **single-word, provenance-carrying, align-derived** tagged pointer that
  petr-core does not; petr keeps keys, selectors, prophecy bits and dispatch.
* o1 provides a **fixed-capacity slot + generation core that never re-validates a stale
  handle** (exhausted slots retire); petr keeps keyed check words and paging.
* o1's framing uses **abut's wire format** (LE `u32` prefix) so the two interoperate on
  one socket; o1 adds what abut does not have: a sans-IO state machine that survives
  partial reads/writes and `EAGAIN` without allocation, and a clean-vs-mid-frame EOF split.
  Socket path hygiene and peer credentials stay in mine.
* No o1 primitive is justified by big-bad-bool or waker_tables today.

## 5. Architecture

```
o1/
├── crates/o1-core    #![no_std], no alloc, no OS. bitops, ptr, collections, arena, frame
├── crates/o1-sync    #![no_std], no alloc. spsc ring, CachePadded. cfg(loom) switch.
├── crates/o1-linux   #![no_std]. errno (pure, every host); syscall, futex, uds, fd (Linux x86_64/aarch64)
├── crates/o1         #![no_std] #![forbid(unsafe_code)] façade (re-exports)
└── tools/
    ├── xtask             unsafe-inventory checker (std, no deps, forbid(unsafe_code))
    └── codegen-probe     #![no_std] functions wrapped for asm inspection (scripts/codegen.sh)
```

Dependency graph: `o1 → {o1-core, o1-sync, o1-linux}`, `o1-linux → o1-core` (framing),
`o1-sync` has no o1 dependency. Nothing depends on `std` at runtime.

*Unit 2 change:* a fifth crate, `o1-contract` (`no_std`, no dependencies,
`forbid(unsafe_code)`), now sits under `o1-core` and `o1-sync`, which depend on it only to
publish the execution contracts of their own primitives; `o1` re-exports it as `contract`.
See `docs/EXECUTION_CONTRACTS.md`.

### 5.1 o1-core::bitops

Per-width modules (`bitops::u8 … u128, usize`) of `const fn`s, generated by one macro so
the contract is written once. Per-width `const fn` rather than a generic trait because
generic trait methods cannot be `const` on stable, and the tag masks in `ptr` are computed
at compile time from them.

Contract (ISA-independent; `BITS` = width):

| fn | result | zero / out-of-range |
|---|---|---|
| `trailing_zero_count(x)` | number of trailing zero bits | `x == 0` → `BITS` |
| `leading_zero_count(x)` | number of leading zero bits | `x == 0` → `BITS` |
| `first_set(x)` | `Some(index of lowest set bit)` | `x == 0` → `None` |
| `last_set(x)` | `Some(index of highest set bit)` | `x == 0` → `None` |
| `pop_count(x)` | number of set bits | — |
| `mask_low(w)` | low `w` bits set | `w ≥ BITS` → all ones (saturating; never a shift overflow) |
| `bit_test(x, i)` | bit `i` | `i ≥ BITS` → `false` |
| `bit_set(x, i)` / `bit_clear(x, i)` | `x` with bit `i` set/cleared | `i ≥ BITS` → `x` unchanged |
| `clear_lowest(x)` / `isolate_lowest(x)` | `x & (x-1)` / `x & x.wrapping_neg()` | `0` → `0` |
| `select(c, a, b)` | `if c { a } else { b }` | — |
| `select_masked(c, a, b)` | same value, computed as `b ^ ((a ^ b) & mask(c))` | — |
| `SetBits` iterator | indices of set bits, ascending | `0` → empty |

Implementation is ordinary `core` (`trailing_zeros`, `leading_zeros`, `count_ones`,
`checked_shl`). `core`'s `trailing_zeros(0) == BITS` is already the semantic contract, so
the portable code needs no zero branch; whether the backend needs one (x86 `bsf` leaves the
destination undefined on zero; `tzcnt` returns the width; AArch64 has no ctz and uses
`rbit`+`clz`, where `clz(0) == 64`) is LLVM's job, and `scripts/codegen.sh` records what it
does for x86_64 baseline, x86_64 `+bmi1,+lzcnt`, and aarch64. **No `asm!` and no
`core::arch` in bitops**: the codegen check is how that decision is kept honest.

`select` vs `select_masked`: both are kept, `select` is the reference, `select_masked` is
the branch-free arithmetic form, differential-tested against `select` and benchmarked.
Neither is claimed to compile to `cmov`/`csel`; the codegen record says what each became.
`core::hint::select_unpredictable` (1.88) is above MSRV. Neither is "constant-time"
in the cryptographic sense; o1 makes no secret-independence claim for any operation.

### 5.2 o1-core::ptr

* `TaggedPtr<T, const BITS: u32>` — **one word**: a `NonNull<T>` whose low `BITS` address
  bits hold the tag. All address arithmetic is `NonNull::map_addr` (strict provenance; the
  pointer keeps the provenance of the pointer it was built from). No ptr→int→ptr round
  trip exists anywhere in o1; `expose_provenance`/`with_exposed_provenance` are never used.
* Tag capacity is `align_of::<T>().trailing_zeros()`. `BITS` above that is a
  **compile-time error** (inline `const { assert!(…) }`, post-monomorphisation) — tested
  with `compile_fail` doc-tests. `BITS` larger than `usize::BITS - 1` is also rejected.
* Constructors are **safe**: `TaggedPtr` is a pointer value, not a reference, so its only
  invariants are "the clean address is non-zero" and "the tag fits" — both checked or
  structurally true. `new(NonNull<T>, tag)` returns `Err` for a tag that does not fit or a
  pointer whose low bits are not clear (a raw `NonNull` may be misaligned);
  `from_ref(&T, tag)` cannot be misaligned so only checks the tag. Dereference
  (`as_ref`, `as_mut`) is `unsafe` with the same contract as `NonNull::as_ref`/`as_mut`.
* ZSTs: allowed. `TaggedPtr<(), 0>` works; `[u64; 0]` (size 0, align 8) gets 3 tag bits;
  the dangling address (`align`) is non-zero after masking. Tested under Miri.
* `T: Sized` only: `align_of::<T>()` needs a sized `T`.
* `AtomicTaggedPtr<T, BITS>` over `AtomicPtr<T>`: load / store / swap /
  compare_exchange(_weak) of whole tagged pointers. Justified because petr-core's
  `AtomicTaggedWord` (an `AtomicUsize`) loses provenance; `AtomicPtr` keeps it. No
  `fetch_or`-style tag-only RMW (needs 1.91 strict-provenance atomics); callers that need
  it write a CAS loop — lock-free, not wait-free, and documented as such.
* `align_down` / `align_up` for raw pointers by a power-of-two `Align`, via `map_addr`.
* `Send`/`Sync`: neither, like `NonNull` (static negative assertions in tests).

### 5.3 o1-core::collections and o1-core::arena

| type | storage | unsafe? | notes |
|---|---|---|---|
| `FixedRing<T, N>` | `[MaybeUninit<T>; N]`, free-running `usize` head/tail, mask indexing | yes (MaybeUninit reads, drop) | N a power of two, `N ≥ 1`, checked at compile time. FIFO push_back/pop_front, `Err(value)` when full. |
| `FreeList<T, N>` | `[Slot<T>; N]`, `enum Slot { Occupied(T), Vacant { next } , Unused }`, bump + free list | **no** | Vacant slots hold the free-list link **in the payload position** (enum layout); this eliminates a separate link array, not per-slot metadata (the enum discriminant remains, often absorbed by a niche). Index-addressed; no stale detection (that is `GenerationalArena`). |
| `GenerationalArena<T, N>` | `enum Entry { Occupied{gen, value}, Vacant{gen, next}, Unused, Retired }` | **no** | `Handle<T> { index: u32, generation: u32 }`. Stale handle rejected in O(1). Generation exhaustion: a slot whose generation would wrap past `u32::MAX` is **retired** (never reused) — capacity shrinks by one rather than risking ABA. |
| `SparseSet<N>` | `dense: [u32; N]`, `sparse: [u32; N]`, `len` | **no** | Members in `0..N`. insert/remove/contains worst-case O(1); `clear` O(1); iteration O(len). Both arrays zero-initialised (reading uninitialised `sparse` — the classic trick — is UB in Rust and is not done). |

Why the arena/free list are written without `unsafe`: an untagged `union` free list needs
occupancy bits anyway to drop occupied values and to make `remove(index)` safe; a bitmap
sized `N.div_ceil(usize::BITS)` needs `generic_const_exprs`. The enum gets the same link
reuse with zero unsafe and Miri-trivial soundness. Index width is `u32` (capacity ≤
`u32::MAX`, compile-time checked) to keep handles 8 bytes.

All four: no heap, capacity fixed by the type, explicit failure returning the value on
exhaustion, no hidden resizing, `const fn new()`. Drop drops exactly the live elements.
Large `N` on the stack is the caller's problem and is documented (`static` or caller-boxed).

### 5.4 o1-core::frame (sans-IO length-prefixed framing)

Wire format: 4-byte little-endian `u32` length, then that many payload bytes (= abut 0.2.2).

* `FrameDecoder<'buf>` over a caller-owned `&'buf mut [u8]`. The I/O layer asks
  `spare()` for the slice to read into (the remaining header bytes, or the remaining
  payload bytes **inside the caller's buffer**) and reports `filled(n)`. Payload bytes go
  from the read call into their final place once: "copy-avoiding userspace framing".
  States: `Header{have}`, `Body{len, have}`, `Discard{remaining}`, `Ready{len}`, `Failed`.
  `filled(0)` is EOF: `Header{have: 0}` → `Eof::Clean`, anywhere else →
  `DecodeError::UnexpectedEof`. Length > `max_len` (≤ buffer length) →
  `DecodeError::Oversized` unless `len ≤ discard_limit`, in which case the payload is read
  into the buffer as scratch and dropped (`Status::Discarded`), keeping the stream aligned.
  Errors poison the decoder (every later call returns the same error): a desynchronised
  stream must not be silently resynchronised.
* `FrameEncoder<'a>` borrows the payload; `pending()` returns `(header_rest, payload_rest)`
  for one vectored write; `advance(n)` handles a write that ends anywhere, including inside
  the header. Payload length > `u32::MAX` is refused at construction.
* No allocation, no attacker-controlled sizing: every byte count is bounded by the
  caller's buffer.

### 5.5 o1-sync::spsc

`Ring<T, N>`: `[UnsafeCell<MaybeUninit<T>>; N]`, `head` (consumer) and `tail` (producer)
free-running `AtomicUsize`s, each in its own `CachePadded`. N a power of two, compile-time.
`split(&mut self) -> (Producer<'_>, Consumer<'_>)`: the `&mut` borrow makes a second
producer or consumer impossible in safe code while the pair lives. Handles cache their own
index and the last seen opposite index (rtrb's idea), so the common case touches only one
shared line.

Orderings (each is the minimum that is correct):

* producer `push`: `slot.write(v)`; `tail.store(t+1, Release)` — publishes the slot write.
* producer full-check refresh: `head.load(Acquire)` — the consumer's move-out of a slot must
  happen-before the producer overwrites it.
* consumer `pop` refresh: `tail.load(Acquire)` — pairs with the producer's Release.
* consumer `pop`: `slot.read()`; `head.store(h+1, Release)` — pairs with the producer's Acquire.
* own-index loads at `split`: `Relaxed` under `&mut self` (no concurrent access exists).

Every `push`/`pop` is a fixed number of steps with no loop and no CAS: **wait-free**,
allocation-free, worst-case O(1) work. Not "bounded latency": cache misses and preemption
are outside any guarantee. Index counters wrap at `usize::MAX`; because `N` divides
`2^usize::BITS`, `tail.wrapping_sub(head)` is the length and `idx & (N-1)` the slot, before
and after the wrap — tested by starting counters near `usize::MAX`.

`CachePadded` alignment: 128 on x86_64 and aarch64 (adjacent-line prefetch on Intel; 128-byte
lines on Apple M-series), 64 elsewhere, 32 on 32-bit arm/thumb — crossbeam-utils' table. The
effect of the padding type is benchmarked (adjacent vs padded counters, `docs/BENCHMARKS.md`).

No owned (`Arc`) handles: o1-sync has no `alloc`; `std::thread::scope`, a `static`, or a
leaked box gives the borrowed handles the lifetime they need. A `parking` abstraction is
**not built** (see §8): no caller needs a blocking SPSC yet.

Loom: `--cfg loom` swaps `AtomicUsize` and `UnsafeCell` for loom's (one switch module,
`sync.rs`, as in scope). Models: push/pop handoff, fill-to-full then drain across threads,
wraparound across the capacity boundary, and drop of an unconsumed item.

### 5.6 o1-linux

* `errno` (every host, pure): `Errno(u16)`, `decode(ret: usize) -> Result<usize, Errno>`
  with Linux's convention (`-4095..=-1` is an error). Tested on macOS.
* `syscall` (Linux x86_64/aarch64 only, private): `unsafe fn syscall3` and `syscall4` (the arities used). x86_64: `syscall`,
  `rax` = number/return, args `rdi rsi rdx r10 r8 r9`, `rcx`/`r11` clobbered (`lateout`).
  aarch64: `svc 0`, `x8` = number, `x0..x5` args, `x0` return. `options(nostack,
  preserves_flags)` — not `nomem`/`readonly`/`pure`: these syscalls read and write user memory
  and have side effects. Linux on any other architecture is `compile_error!`; non-Linux
  targets compile the crate with only `errno` (documented, not a stub).
* `fd::BorrowedFd<'a>`: `#[repr(transparent)] i32` + lifetime; `unsafe fn borrow_raw`
  (I/O safety, as std's).
* `io`: `read`, `write`, `sendmsg`-based `send_vectored` with `MSG_NOSIGNAL` (no `SIGPIPE`
  on a closed peer — the reason uds uses `sendmsg` instead of `writev`).
* `futex`: `wait(&AtomicU32, expected, Option<Duration>)` (one kernel attempt; outcome
  `Woken | ValueMismatch | Interrupted | TimedOut`), `wake(&AtomicU32, n)`, and
  `wait_while_equal(&AtomicU32, expected)` which separates the userspace fast path (a
  single `Acquire` load, bounded) from the kernel slow path and loops over spurious wakes
  and `EINTR`. `FUTEX_PRIVATE_FLAG` always (process-local futexes only).
* `uds`: drives `o1_core::frame` over a stream-socket fd: `recv_frame`, `send_frame`,
  nonblocking-aware (`EAGAIN` → `WouldBlock`, state kept), `EINTR` retried.

## 6. Verification architecture

```
            portable semantic reference (plain Rust)
              /                       \
     Miri (strict provenance,     optimised backend / other form
     tree borrows) + Loom          (select_masked, codegen record)
              \                       /
               differential + property tests
```

Details and exact commands: `docs/VERIFICATION.md`.

## 7. Linux constants (cross-checked against linux-raw-sys 0.12.1 source)

| name | x86_64 | aarch64 |
|---|---|---|
| `read` | 0 | 63 |
| `write` | 1 | 64 |
| `sendmsg` | 46 | 211 |
| `futex` | 202 | 98 |
| `FUTEX_WAIT` / `FUTEX_WAKE` / `FUTEX_PRIVATE_FLAG` | 0 / 1 / 128 | same |
| `MSG_NOSIGNAL` | 16384 | same |
| `EINTR` `EAGAIN` `EBADF` `EFAULT` `EINVAL` `EPIPE` `ENOSYS` `ECONNRESET` `ETIMEDOUT` | 4 11 9 14 22 32 38 104 110 | same |
| `struct msghdr` | name, namelen(c_int), iov, iovlen(usize), control, controllen(usize), flags(c_uint) — 56 bytes | same |
| `struct iovec` | base, len(usize) — 16 bytes | same |
| `struct __kernel_timespec` | `i64` sec, `i64` nsec | same |

## 8. Rejected alternatives

* **One crate.** Rejected: the OS layer must be droppable for embedded users, and the
  Loom switch belongs to o1-sync only.
* **Framing inside o1-linux.** Rejected (brief position 5 kept): an OS-independent
  state machine can be Miri- and property-tested on any host; the Linux part is a thin driver.
* **`asm!` or `core::arch` backends for bitops.** Rejected until codegen shows LLVM failing;
  the record in `docs/codegen/` shows it does not.
* **Two-field tagged pointer (petr-core style).** Rejected for o1: the point of a tagged
  pointer at substrate level is one word and atomics over it.
* **`union` free list with a separate occupancy bitmap.** Rejected: needs
  `generic_const_exprs` for the bitmap size or a runtime-sized external bitmap, and adds
  unsafe; the enum form is safe and stores the link in the vacant payload.
* **Wrapping generations.** Rejected: retire the slot instead (ABA-free by construction).
* **Owned `Arc` SPSC handles behind an `alloc` feature.** Rejected for now: no caller,
  and scoped threads cover the borrowed case.
* **A parking/`Park` trait in o1-sync.** Deferred: with no blocking queue in o1 there is
  no caller; a trait without a second implementation is speculation. futex stays narrow.
* **libc / rustix / linux-raw-sys as dependencies.** Rejected (§3). The reason for raw
  syscalls is architectural — no libc in a `no_std` Linux build, and an ABI layer whose
  every register and constant is in this repo — not speed.
* **`writev` for frame sends.** Rejected for sockets in favour of `sendmsg(MSG_NOSIGNAL)`
  so a closed peer is an `EPIPE` error, not a process-killing signal.
* **Publishing.** Out of scope; every crate is `publish = false`. Name notes are in the report.
