# Changelog

Released crates of this repository. Each crate's package also carries its own `CHANGELOG.md`.

## o1-contract 0.1.0 (2026-11-18)

First release. `Policy` / `PolicyDesc` (execution intent, contradictions rejected),
`ExecutionContract` (progress, policy, isolation, `Cost`, derived `Bound`), `ResourceEnvelope`
(eleven checked counts and their composition rules) and the size multipliers. `no_std`, no
`alloc`, no dependencies, no `unsafe`. MSRV 1.85.

## o1-core 0.1.0 (2026-11-18)

First release. `bitops`, `ptr` (`TaggedPtr`, `AtomicTaggedPtr`), `collections` (`FixedRing`,
`SparseSet`), `arena` (`FreeList`, `GenerationalArena`), `frame` (sans-IO length-prefixed
framing), `stream` (`BlockPass`), with execution contracts for the arena, the block pass and
the frame decoder. `no_std`, no `alloc`, no OS calls; depends on `o1-contract` 0.1. MSRV 1.85.

## o1-sync 0.1.0 (2026-11-18)

First release. `spsc::Ring` with wait-free `push`, `pop`, `free_slots` and `available`, its
execution contracts, and `CachePadded`. Feature `access-counting` is verification tooling only.
`no_std`, no `alloc`; depends on `o1-contract` 0.1. MSRV 1.85.

## o1-predict 0.1.0 (2026-11-18)

First release. Exact-class operation prediction from calibration evidence: interval results
with their evidence, or one of 22 machine-readable refusals; no nearest-class substitution, no
extrapolation. Optional `serde`. `no_std` + `alloc`, no `unsafe`; depends on `o1-contract` 0.1.
MSRV 1.85.
