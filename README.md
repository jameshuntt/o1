# o1

A small Rust substrate for fixed-capacity, bounded-work, pointer-sensitive and
concurrency-sensitive primitives, so crates above it can be `#![deny(unsafe_code)]` and
delegate the unsafe they genuinely need. MSRV 1.85, edition 2024.

## Published crates

| crate | `no_std` | contents |
|---|---|---|
| [`o1-contract`](crates/o1-contract) | yes, no alloc, `forbid(unsafe_code)` | execution-intent `Policy` (zero-sized), `ExecutionContract`, multidimensional `ResourceEnvelope` (checked counts, no weights), size multipliers |
| [`o1-core`](crates/o1-core) | yes, no alloc, no OS | `bitops`, `ptr` (strict-provenance `TaggedPtr`, `AtomicTaggedPtr`), `collections` (`FixedRing`, `SparseSet`), `arena` (`FreeList`, `GenerationalArena`), `frame` (sans-IO length-prefixed framing), `stream` (`BlockPass`) |
| [`o1-sync`](crates/o1-sync) | yes, no alloc | `spsc::Ring` (fixed-capacity SPSC ring, wait-free `push`/`pop`, Loom-modelled), `CachePadded` |
| [`o1-predict`](crates/o1-predict) | yes, alloc, `forbid(unsafe_code)` | operation-class prediction from calibration evidence: exact class matching, no extrapolation, intervals or machine-readable refusals; optional `serde` |

Each is released to crates.io at 0.1.0, `o1-contract` first (the other three depend only on
it). Each crate folder has its own `README.md` and `CHANGELOG.md`; the repository-wide
changelog is [`CHANGELOG.md`](CHANGELOG.md).

## Not published

* [`o1-linux`](crates/o1-linux) — raw Linux syscalls (x86_64, aarch64), futex wait/wake,
  framed Unix-domain stream I/O. It compiles for both Linux targets, but its runtime
  behaviour is NOT YET VERIFIED: its Linux tests have never been run.
* [`o1`](crates/o1) — the façade re-exporting the substrate crates. It re-exports `o1-linux`,
  so it waits on it.
* [`tools/`](tools) — `xtask` (unsafe inventory check) and `codegen-probe` (non-generic
  wrappers compiled to assembly for the codegen record in [`docs/codegen`](docs/codegen)).

The measurement laboratories, machine profiles, calibration and verification tooling and the
research evidence are kept in a private research tree. They are not part of this repository.

## Example

```rust
use o1_core::arena::GenerationalArena;

let mut a = GenerationalArena::<&str, 64>::new();
let h = a.insert("x").unwrap();
assert_eq!(a.remove(h), Some("x"));
assert_eq!(a.get(h), None); // stale handle rejected
```

## Documents

See [`docs/README.md`](docs/README.md).

## Checks

```sh
cargo test --workspace
cargo +nightly test --workspace --doc     # also checks compile_fail error codes
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo run -p xtask -- unsafe-inventory
RUSTFLAGS="--cfg loom" cargo test -p o1-sync --release --lib
PROPTEST_CASES=8 MIRIFLAGS="-Zmiri-strict-provenance -Zmiri-disable-isolation" cargo +nightly miri test -p o1-contract -p o1-core -p o1-sync
```

CI (`.github/workflows/ci.yml`) runs the gates of the four published crates; releases are
published by tag (`docs/RELEASING.md`).

## License

The published crates are licensed under either of Apache License, Version 2.0
([`LICENSE-APACHE`](LICENSE-APACHE)) or MIT license ([`LICENSE-MIT`](LICENSE-MIT)) at your
option.
