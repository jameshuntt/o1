# o1-sync

Allocation-free bounded synchronisation: a fixed-capacity single-producer single-consumer ring
(`spsc::Ring`) and a cache-line padding wrapper (`CachePadded`).

## Core abstractions

* `spsc::Ring<T, N>` — `N` slots inline (`N` a power of two, checked at compile time).
  `Ring::split(&mut self)` gives one `Producer` and one `Consumer`; the borrow checker rules
  out a second pair, and neither handle is `Clone`. A full ring makes `push` return
  `Err(value)`; an empty ring makes `pop` return `None`. Waiting for space or data (spinning,
  parking) is the caller's choice.
* `CachePadded<T>` — aligns and pads a value (128 bytes on x86_64, aarch64 and powerpc64;
  32 on 32-bit arm, mips, riscv32 and a few others; 64 elsewhere) so the producer's and the consumer's indices do
  not share a cache line.
* `Ring::PUSH_CONTRACT`, `Ring::POP_CONTRACT` — `o1-contract` execution contracts stating the
  worst case of one call as counts.

## Example

```rust
use o1_sync::spsc::Ring;

let mut ring = Ring::<u64, 8>::new();
let (mut tx, mut rx) = ring.split();
std::thread::scope(|s| {
    s.spawn(move || {
        for i in 0..1000 {
            let mut v = i;
            while let Err(back) = tx.push(v) { v = back; std::hint::spin_loop(); }
        }
    });
    let mut expect = 0;
    while expect < 1000 {
        if let Some(v) = rx.pop() { assert_eq!(v, expect); expect += 1; }
    }
});
```

## Progress guarantees, per operation

| operation | progress | work |
|---|---|---|
| `Producer::push` | wait-free | ≤ 1 acquire load, 1 slot write, 1 release store |
| `Consumer::pop` | wait-free | ≤ 1 acquire load, 1 slot read, 1 release store |
| `Producer::free_slots`, `Consumer::available` | wait-free | 1 acquire load |
| `Ring::split` | — (needs `&mut`) | O(1) |
| `Drop for Ring` | — | O(items left) |

Wait-free here means each of these calls finishes in a bounded number of its own steps
whatever the other thread does: there is no loop and no compare-exchange on these paths. This
is the claim for exactly the operations in the table; nothing else in the crate is called
wait-free. Wait-free is stronger than lock-free (lock-free only guarantees that some thread
makes progress; a lock-free operation may retry indefinitely). Allocation-free is a separate
property and does not imply wait-free. Neither is a wall-clock bound: cache-coherence traffic
on the two index lines and preemption by the OS are outside any guarantee. The caller's retry
loop around a full `push` or an empty `pop` (as in the example) is not wait-free.

## Memory orderings

Documented in the `spsc` module documentation (section "Memory orderings"): each side
publishes its counter with one `Release` store and reads the other side's counter with
`Acquire` only when its cached copy says full (producer) or empty (consumer). No `SeqCst`.

## Limitations

* One producer and one consumer; handles borrow the ring (use `std::thread::scope`, a
  `static`, or memory the caller owns). No `Arc`-owned handles.
* Slots are not padded: adjacent slots can share a cache line.
* No blocking or parking.
* Feature `access-counting` (off by default, needs `std`) is verification tooling: it adds a
  thread-local counter update to every counted access. Not for production.

## Platform

* MSRV: Rust 1.85 (edition 2024).
* `#![no_std]`, no `alloc`, no OS calls. Needs `AtomicUsize` load and store only, so it
  builds for `thumbv7em-none-eabihf` (32-bit atomics, no 64-bit atomics). Depends only on
  `o1-contract` (and on `loom` under `--cfg loom`, for model checking).

## Safety model

6 `unsafe` sites, each with a documented invariant: 1 `unsafe impl Sync for Ring` (requires
`T: Send`), 3 blocks in library code (slot write, slot read, drop of remaining items) and 2 in
the Loom models.

| what | evidence |
|---|---|
| no data race and correct visibility with the chosen orderings | TESTED (Loom: 4 models with ≤ 3 items and capacities 1–2, covering handoff, full/empty, slot reuse, counter wrap at `usize::MAX`, drop after concurrent use) |
| each of the four `Release` / `Acquire` orderings is needed | TESTED (each weakened to `Relaxed` in turn; Loom catches every one) |
| no undefined behaviour on the executed test paths, including a two-thread test | TESTED (Miri, strict provenance) |
| `push` / `pop` have no loop and no compare-exchange | ASSUMED from reading the code |
| one producer and one consumer only; `Ring<T>` is `Sync` only if `T: Send` | TESTED (`compile_fail` doc-tests; error codes checked on nightly rustdoc) |
| declared contracts equal the counted atomic and slot accesses | TESTED (counting unit tests) |
| Loom models larger than the four above | NOT YET VERIFIED |

Miri and Loom find faults on the executions they run; they do not prove their absence.

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
