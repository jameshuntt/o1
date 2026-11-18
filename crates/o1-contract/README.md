# o1-contract

A vocabulary for stating how an operation executes and which resources one execution needs,
as counts. It has no dependencies and contains no `unsafe`.

## Core abstractions

* `Policy` — a zero-sized execution intent built with `const fn` setters
  (`Policy::new().streaming().non_temporal_write()`); contradictory settings do not compile.
  `PolicyDesc` is its plain-data mirror with the same validation.
* `ExecutionContract` — progress class (`NoSynchronization`, `WaitFree`, `LockFree`,
  `Blocking`), policy, control-state isolation and a `Cost`. Its `Bound` (constant, per-unit,
  or capped by a fixed capacity) is derived from the cost.
* `ResourceEnvelope` — one `u64` count per `Quantity` (ops, bytes read and written, atomic
  loads / stores / read-modify-writes, shared reads and writes, kernel entries, allocations,
  working set). Totals add; the working-set peak combines by `max` (`then`, `repeat`) or by
  sum (`alongside`, `repeat_retained`). Arithmetic is checked: overflow is an error naming the
  quantity; nothing wraps or saturates.
* `Cost`, `split_blocks`, `lines_touched`, `Cost::for_extent` — turn a size into a count of
  units and an envelope.

These types describe structural work, resources and execution semantics, and how they compose.
A `ResourceEnvelope` does not by itself predict elapsed time: no quantity carries a weight, and
two envelopes are compared quantity by quantity, never reduced to one number.

## Example

```rust
use core::num::NonZeroU64;
use o1_contract::{Cost, PeakGrowth, Quantity::*, ResourceEnvelope as E, Unit};

let nz = |n| NonZeroU64::new(n).unwrap();
// 64 MiB through a 256-byte block transform with 4 KiB of hot state.
let job_a = Cost {
    fixed: E::ZERO.with(WorkingSet, 4096),
    per_unit: E::ZERO.with(Ops, 1).with(BytesRead, 256).with(BytesWritten, 256)
        .with(WorkingSet, 512),
    unit: Unit::Block { bytes: nz(256) },
    max_units: None,
    peak_growth: PeakGrowth::Fixed, // streaming: each block released before the next
}
.for_extent(0, 64 << 20)
.unwrap();

// 2 MiB of 64-byte records, one atomic read-modify-write each on a shared 64 KiB table.
let job_b = Cost {
    fixed: E::ZERO.with(WorkingSet, 64 << 10),
    per_unit: E::ZERO.with(Ops, 1).with(BytesRead, 64).with(AtomicRmws, 1)
        .with(SharedReads, 1).with(SharedWrites, 1).with(WorkingSet, 64),
    unit: Unit::Block { bytes: nz(64) },
    max_units: None,
    peak_growth: PeakGrowth::Fixed,
}
.for_extent(0, 2 << 20)
.unwrap();

assert_eq!(job_a.to_string(),
    "ops=262144, bytes_read=67108864, bytes_written=67108864, working_set=4608");
// Neither fits inside the other: they differ in kind, not only in size.
assert!(!job_a.within(&job_b) && !job_b.within(&job_a));
```

## Limitations

* Counts only: no weights, no "compute units", no time, energy or quota comparison.
* The working-set peak has two rules (released between units, or live together); choosing the
  one that matches a caller's loop is the caller's job.
* Not claimed by any type here: cryptographic constant time, bounded wall-clock latency.

## Platform

* MSRV: Rust 1.85 (edition 2024).
* `#![no_std]`, no `alloc`, no dependencies. Built for `thumbv7em-none-eabihf` (no OS, no
  allocator).

## Safety model

`#![forbid(unsafe_code)]`: 0 `unsafe` sites.

| what | evidence |
|---|---|
| envelope arithmetic against a `u128` reference, overflow reported per quantity | TESTED (property tests) |
| policy validation: every combination of settings | TESTED (exhaustive truth table) |
| contradictory or repeated policy settings rejected at compile time | TESTED (`compile_fail` doc-tests; error codes checked on nightly rustdoc) |
| no undefined behaviour on the executed test paths | TESTED (Miri, strict provenance) |

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
