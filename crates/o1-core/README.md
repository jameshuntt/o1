# o1-core

OS-independent, allocation-free primitives: bit operations, strict-provenance tagged pointers,
fixed-capacity collections, a generational arena, sans-IO length-prefixed framing and a
single sequential pass over fixed-size blocks. Nothing here allocates or calls the operating
system.

## Core abstractions

| module | contents |
|---|---|
| `bitops` | bit scans, masks and selection with an ISA-independent contract (zero input and out-of-range widths defined, no panics) |
| `ptr` | `TaggedPtr` and `AtomicTaggedPtr`: one word, tag in the alignment bits, provenance kept through every tag change; alignment helpers |
| `collections` | `FixedRing` (power-of-two capacity ring), `SparseSet` |
| `arena` | `FreeList` (index-addressed slots), `GenerationalArena` and `Handle` (stale handles rejected) |
| `frame` | `FrameDecoder` / `FrameEncoder`: 4-byte little-endian length prefix over caller-owned buffers; partial reads and writes resume where they stopped |
| `stream` | `BlockPass`: one sequential pass over whole `N`-byte blocks; the remainder is reported, not dropped |

Capacities are const generics. A full container returns the value (`Err(value)`); invalid
capacities and tag widths are compile-time errors. `GenerationalArena`, `BlockPass` and
`FrameDecoder` publish `o1-contract` execution contracts (what one call structurally does, as
counts).

## Example

```rust
use o1_core::arena::GenerationalArena;
use o1_core::collections::FixedRing;

let mut a = GenerationalArena::<u32, 4>::new();
let h = a.insert(10).unwrap();
assert_eq!(a.remove(h), Some(10));
let h2 = a.insert(20).unwrap();
assert_eq!(h2.index(), h.index()); // same slot ...
assert_eq!(a.get(h), None); // ... but the old handle is stale
assert_eq!(a.get(h2), Some(&20));

let mut r = FixedRing::<u32, 4>::new();
for i in 0..4 { r.push_back(i).unwrap(); }
assert_eq!(r.push_back(9), Err(9)); // full: value returned
assert_eq!(r.pop_front(), Some(0));
```

## What the bounds mean

Operations documented as worst-case O(1) or bounded by a fixed capacity do a bounded amount
of algorithmic work and never allocate. Allocation-free and bounded algorithmic work are not
bounded wall-clock latency: cache misses, contention and preemption are outside any
guarantee. No operation claims cryptographic constant time (secret-independent timing).

## Limitations

* Handles from another arena of the same element type are not detected (no branding).
* `GenerationalArena` retires a slot whose generation is exhausted; capacity shrinks by one
  for each retired slot.
* `BlockPass` accepts non-temporal write requests in its policy and records them in its
  contract; the only implementation uses ordinary stores.
* Single-threaded containers: `Send` / `Sync` follow the element type. For a cross-thread
  queue see `o1-sync`.

## Platform

* MSRV: Rust 1.85 (edition 2024).
* `#![no_std]`, never uses `alloc`, no OS calls. Built for `thumbv7em-none-eabihf` (no OS,
  no allocator, no 64-bit atomics). Depends only on `o1-contract`.

## Safety model

12 `unsafe` sites, each with a documented invariant: 8 in library code (`FixedRing` slot
reads, references and drop; `TaggedPtr::as_ref` / `as_mut` and their bodies) and 4 in unit
tests. `bitops`, `arena`, `frame`, `stream` and `SparseSet` contain no `unsafe`. Pointer
arithmetic uses `map_addr` only; there is no pointer-to-integer-to-pointer round trip and no
exposed provenance.

| what | evidence |
|---|---|
| containers behave as a std model under random operation sequences; drop exactly once, also with panicking destructors | TESTED (unit and property tests) |
| framing round trip under arbitrary partial reads and writes; arbitrary input never panics or writes outside the buffer | TESTED (property tests) |
| no undefined behaviour on the executed test paths, tag round trips included | TESTED (Miri, strict provenance, Stacked and Tree Borrows) |
| invalid capacities, tag widths, `TaggedPtr` not `Send`/`Sync` | TESTED (`compile_fail` doc-tests; error codes checked on nightly rustdoc) |
| bit scans and tag accessors compile to the expected instructions without branches (x86_64, aarch64) | CODEGEN-VERIFIED |
| O(1) work for arena insert / remove / lookup | ASSUMED from code structure (no loop on those paths) |
| arena contract numbers | ASSUMED (read from the code, not counted); `BlockPass` and `FrameDecoder` contracts TESTED by counting |

Source: <https://github.com/jameshuntt/o1>. Design notes, the guarantee matrix, the safety model and the unsafe
inventory are in its `docs/` folder. Verification runs, machine profiles and measurement
evidence are kept in a private research tree and are not part of that repository.

## Changelog

See `CHANGELOG.md` in this package.

## License

Licensed under either of

* Apache License, Version 2.0 (`LICENSE-APACHE`)
* MIT license (`LICENSE-MIT`)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion
in this crate by you, as defined in the Apache-2.0 license, shall be dual licensed as above,
without any additional terms or conditions.
