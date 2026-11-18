# Unsafe inventory

Every `unsafe` block, `unsafe fn` and `unsafe impl` in the repository's code (`crates/`,
`tools/`, since unit 3 `o1-custodian/`, since unit 4 `o1-machine-profile/`, and since unit 5
`o1-predict-lab/`), with the
invariant it relies on. Checked by:

```
cargo run -p xtask -- unsafe-inventory          # fails on any mismatch
cargo run -p xtask -- unsafe-inventory --list   # prints every site with file:line
```

The checker (`tools/xtask/src/main.rs`) finds `unsafe` in code (comments and string literals
skipped), requires an `[unsafe:<id>]` tag in the comment lines directly above each block/impl
and a `# Safety` doc section on each `unsafe fn` (id `fn:<file>:<name>`), and requires the
set of ids to equal the first-column ids of the tables below. A new unsafe site without an
entry here, or an entry here without a site, fails the check.

Not counted: `unsafe` inside rustdoc examples (one, in the `o1_core::ptr` module docs, which
dereferences a `TaggedPtr` built from a live reference), and `unsafe` inside third-party crates.

Counts printed by the checker at the time of writing (32 sites; unchanged by units 2, 3, 4 and 5,
which added no unsafe): **o1-core 12** (6 blocks + 2
`unsafe fn` in library code, 4 blocks in unit tests), **o1-sync 6** (1 `unsafe impl`, 3
library blocks, 2 Loom-model blocks), **o1-linux 14** (3 `unsafe fn`, 4 `asm!` blocks, 5 other
library blocks, 2 Linux-test blocks), **o1 0** (`#![forbid(unsafe_code)]`), **o1-contract 0**
(`#![forbid(unsafe_code)]`, added in unit 2), **o1-custodian 0** (`unsafe_code = "forbid"`
in its manifest; scanned since unit 3, which added no unsafe), **o1-machine-profile 0**
(`unsafe_code = "forbid"` in its manifest; scanned since unit 4, which added no unsafe),
**o1-predict 0** (`#![forbid(unsafe_code)]`, unit 5), **o1-predict-lab 0**
(`unsafe_code = "forbid"`; scanned since unit 5), **tools 0**
(`xtask` is `#![forbid(unsafe_code)]`; `o1-codegen-probe` has no unsafe). The four `asm!`
blocks are two per architecture; only one architecture's pair is compiled into any build.

## o1-core — library

| id | file | kind | invariant relied on |
|---|---|---|---|
| `ring.pop-read` | `crates/o1-core/src/collections/ring.rs` | block | Ring non-empty ⇒ slot at `head` is in `head..tail` and initialised; `head` advances immediately after, so the value is moved out exactly once. |
| `ring.get-ref` | same | block | `i < len` ⇒ slot `head+i` initialised; `&self` borrow keeps it so for the returned lifetime. |
| `ring.get-mut` | same | block | As `ring.get-ref`; `&mut self` makes the reference unique. |
| `ring.drop` | same | block | Slots `head..head+len` initialised when `Drop` starts; ring marked empty first; each slot dropped once (a panicking destructor leaks the rest, never double-drops). |
| `fn:crates/o1-core/src/ptr.rs:as_ref` | `crates/o1-core/src/ptr.rs` | unsafe fn | Caller contract = `NonNull::as_ref` on the tag-cleared pointer. |
| `ptr.as-ref` | same | block | Forwards `as_ref`'s contract verbatim; the tag-cleared pointer keeps the original provenance (`map_addr`). |
| `fn:crates/o1-core/src/ptr.rs:as_mut` | same | unsafe fn | Caller contract = `NonNull::as_mut`, plus write permission in the provenance (built from `&mut`). |
| `ptr.as-mut` | same | block | Forwards `as_mut`'s contract verbatim. |

## o1-core — unit tests (`crates/o1-core/src/ptr.rs`)

| id | file | kind | invariant relied on |
|---|---|---|---|
| `ptr-test.write-through` | `crates/o1-core/src/ptr.rs` | block | Pointer built from `&mut x`; `x` live and not otherwise borrowed. |
| `ptr-test.read-after-retag` | same | block | Same allocation; the mutable use has ended. |
| `ptr-test.zst-ref` | same | block | Zero-sized read through a non-null, aligned pointer. |
| `ptr-test.atomic-roundtrip-read` | same | block | Provenance carried through `AtomicPtr`; `x` live. |

## o1-sync

| id | file | kind | invariant relied on |
|---|---|---|---|
| `spsc.sync` | `crates/o1-sync/src/spsc.rs` | impl `Sync for Ring<T: Send, N>` | Slots are only reached through the single `Producer`/`Consumer` pair (`split(&mut self)`); producer writes only outside `head..tail`, consumer reads only inside; Release/Acquire on `tail`/`head` order write→read and read→rewrite. `T` moves between threads (`Send`); no `&T` is shared (no `Sync` bound). |
| `spsc.push-write` | same | block | `tail − head < N` ⇒ slot is outside `head..tail`, not accessed by the consumer until the Release store of `tail`; previous occupant's move-out ordered by the Acquire load of `head`. |
| `spsc.pop-read` | same | block | `head ≠ tail` (observed with Acquire) ⇒ slot initialised and published; producer won't rewrite it until the Release store of `head`; read once. |
| `spsc.drop` | same | block | `&mut self` ⇒ no handles alive; slots `head..tail` initialised; each dropped once. |
| `spsc.loom-reborrow` | same (Loom model, `cfg(all(test, loom))`) | block | `raw` from `Box::leak`; the only live reference until the handles drop. |
| `spsc.loom-unleak` | same (Loom model) | block | Handles and spawned thread gone; `raw` from `Box::leak(Box::new(..))`, re-boxed once. |

## o1-linux — library

| id | file | kind | invariant relied on |
|---|---|---|---|
| `fn:crates/o1-linux/src/fd.rs:borrow_raw` | `crates/o1-linux/src/fd.rs` | unsafe fn | I/O safety: `fd` open, usable by the caller, for the borrow's lifetime (memory safety does not depend on it). |
| `fn:crates/o1-linux/src/syscall.rs:syscall3` | `crates/o1-linux/src/syscall.rs` | unsafe fn | Caller: the syscall's memory accesses through pointer args are valid for the call; its side effects break no Rust invariant. |
| `fn:crates/o1-linux/src/syscall.rs:syscall4` | same | unsafe fn | As `syscall3`. |
| `linux.asm3-x86_64` | same | block (`asm!`) | Linux x86_64 ABI: `rax` nr/ret, `rdi rsi rdx` args, `rcx`/`r11` clobbered; `nostack`, `preserves_flags`; memory effects visible to the compiler. Not Miri-checked (asm). |
| `linux.asm3-aarch64` | same | block (`asm!`) | Linux aarch64 ABI: `x8` nr, `x0..x2` args, `x0` ret; `nostack`, `preserves_flags`. Not Miri-checked. |
| `linux.asm4-x86_64` | same | block (`asm!`) | As above, 4th arg in `r10` (not `rcx`). |
| `linux.asm4-aarch64` | same | block (`asm!`) | As above, 4th arg in `x3`. |
| `linux.read` | `crates/o1-linux/src/io.rs` | block | `buf` exclusively borrowed, valid for `buf.len()` writes; kernel writes at most that many. |
| `linux.write` | same | block | `buf` valid for `buf.len()` reads; kernel only reads. |
| `linux.sendmsg` | same | block | `msghdr` on this frame with kernel layout (const-asserted 56 bytes, field offsets); `iov` → `IoSlice`s layout-identical to `iovec`, each borrowing a live slice; kernel only reads; no name/control buffers. |
| `linux.futex-wait` | `crates/o1-linux/src/futex.rs` | block | `&AtomicU32` valid and 4-aligned for the call; timeout pointer null or a live `__kernel_timespec`-layout value; private futex. |
| `linux.futex-wake` | same | block | `&AtomicU32` used only as a key; 4th arg null. |

## o1-linux — Linux-only integration tests (`crates/o1-linux/tests/linux.rs`)

| id | file | kind | invariant relied on |
|---|---|---|---|
| `linux-test.borrow` | `crates/o1-linux/tests/linux.rs` | block | The `BorrowedFd` borrows the owning `UnixStream`, so the descriptor stays open. |
| `linux-test.bad-fd` | same | block | Deliberately violates I/O safety's "open descriptor" with `-2` to test `EBADF`; `-2` can never name a descriptor, so no other owner's I/O is touched; memory safety unaffected. |
