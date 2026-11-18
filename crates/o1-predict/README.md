# o1-predict

Operation-class prediction. Given an operation plan and one or more machine-profile runs
(calibration evidence), `o1-predict` selects only the calibrations that apply to each step of
the plan and returns a time interval with the evidence it rests on, or refuses with a
machine-readable reason.

## Core abstractions

* `OperationClass` = `OperationId` (package, path) + `ClassKey` (sorted typed parts: block
  size, atomic operation and ordering, sharing, access pattern, …). Classes match by exact
  equality of the whole key. There is no nearest-class substitution: when a key part differs,
  the refusal names the parts that differ from the closest calibrated class, and nothing is
  priced from it.
* `Geometry` — one typed parameter per curve (`ExtentBytes`, `WorkingSetBytes`, `Items`, …).
  A point between two measured points of one regime is interpolated; a point outside the
  measured range is refused (`OutOfRange`). There is no extrapolation.
* `Evidence` / `ProfileEvidence` / `CalibrationEvidence` — plain data converted from a profile;
  `Now` — what the caller states about the machine and the code now. Stale or mismatched
  evidence (machine, method, build, implementation) is refused.
* `Plan` — leaves (class, geometry, count) composed by sequence, repetition, parallel and
  branch nodes; counts are exact, expected, worst case or unknown (unknown is refused).
* `predict` → `Prediction` (an `Interval` `[lower, central, upper]` in ns, per-leaf evidence,
  the runs used, a `Grade`) or `PlanRefusal` (`Refusal` with data, `Refusal::variant()` gives
  its name).
* Two selection revisions: `Options::CLASS_A` refuses a regime boundary; `Options::CLASS_B`
  brackets it and the result is graded informational.

## Example

```rust
use o1_predict::class::{ClassKey, KeyPart, OperationClass};
use o1_predict::evidence::{
    BuildFacts, CalibrationEvidence, CalibrationStatus, CurrentImplementation, Evidence, Now,
    ProfileEvidence,
};
use o1_predict::geometry::{Geometry, Parameter};
use o1_predict::identity::{Implementation, OperationId};
use o1_predict::plan::{Count, Leaf, Plan};
use o1_predict::{Options, predict};

let class = OperationClass::new(
    OperationId::new("o1-core", "stream::BlockPass::transform"),
    ClassKey::new(vec![KeyPart::BlockBytes(256)]).unwrap(),
);
let implementation = Implementation::new("0.1.0", "sha256:aa");
let point = |extent: u64, ns_per_byte: f64| CalibrationEvidence {
    id: format!("transform.e{extent}"),
    class: class.clone(),
    implementation: implementation.clone(),
    parameter: Parameter::ExtentBytes,
    value: extent,
    unit: "byte".into(),
    median: ns_per_byte,
    spread: 0.01,
    round_low: ns_per_byte * 0.995,
    round_high: ns_per_byte * 1.005,
    status: CalibrationStatus::Stable,
    conditions: "example".into(),
};
let run = ProfileEvidence {
    schema: "o1.machine-profile.v2".into(),
    identity_digest: "sha256:machine".into(),
    environment_digest: "sha256:env".into(),
    method_digest: "sha256:method".into(),
    revision: "r".into(),
    clean: true,
    built_from_source: true,
    build: BuildFacts::default(),
    performance_cores: None,
    conditions: "example".into(),
    calibrations: vec![point(16_384, 0.0086), point(65_536, 0.0088)],
};
let evidence = Evidence { runs: vec![run.clone(), run] };
let now = Now {
    schema: "o1.machine-profile.v2".into(),
    identity_digest: "sha256:machine".into(),
    environment_digest: "sha256:env".into(),
    method_digest: "sha256:method".into(),
    build: BuildFacts::default(),
    implementations: vec![CurrentImplementation {
        id: class.id.clone(),
        implementation: implementation.clone(),
    }],
};
let leaf = |extent: u64| Plan::Leaf(Leaf {
    name: "pass".into(),
    class: class.clone(),
    geometry: Geometry::new(Parameter::ExtentBytes, extent).unwrap(),
    count: Count::Exact(extent),
});

// At a measured point of the curve: an interval in ns.
let p = predict(&leaf(65_536), &evidence, &now, Options::CLASS_A).unwrap();
assert!(p.interval.lower <= p.interval.central && p.interval.central <= p.interval.upper);

// Outside it: refused, never extrapolated.
let r = predict(&leaf(1 << 20), &evidence, &now, Options::CLASS_A).unwrap_err();
assert_eq!(r.refusal.variant(), "OutOfRange");
```

## What a prediction is, and is not

A prediction is an interval computed from the supplied calibrations of the same class on the
same machine, method, build and implementation. It is only as good as that evidence, and the
evidence comes from the caller.

Prediction does not by itself imply fleet/admission suitability. `o1-predict` supplies the
prediction mechanism. Whether a particular prediction is appropriate for admission/placement
is a separate evidence question. `Grade::Admission` is a label for structural criteria of the
evidence (at least two agreeing runs, stable calibrations, exact counts, no parallel or
branch node, relative width ≤ 0.20); it is not evidence that a prediction is suitable for
admission or placement. No fleet scheduling, admission control or placement is implemented in
this crate.

## Limitations

* Every class without its own calibration is refused: other orderings, access patterns,
  thread shapes, kernel entries, allocation, callbacks, unbounded or data-dependent counts,
  history-dependent paths, other machines.
* Interactions between composed steps are not modelled; `Seq`, `Repeat`, `Par` and `Branch`
  compose intervals by stated rules.
* The crate does not measure anything. Profiles come from a separate calibration tool that is
  not published.

## Platform

* MSRV: Rust 1.85 (edition 2024).
* `#![no_std]` with `alloc` (strings and vectors in plans, evidence and refusals). Built for
  `thumbv7em-none-eabihf`, with and without the `serde` feature.
* Dependencies: `o1-contract` only; `serde` (derives only, `alloc`, no `std`) behind the
  optional `serde` feature.

## Safety model

`#![forbid(unsafe_code)]`: 0 `unsafe` sites. No dependency on a measurement, system
information or verification runtime. Counts are integers with checked arithmetic; the only
conversion to `f64` is the final `count × ns per unit`.

| what | evidence |
|---|---|
| selector steps (applicability, class selection, placement, interval), every refusal variant, composition rules | TESTED (unit tests; property tests for the checked arithmetic and the intervals) |
| `serde` round trip of the public types | TESTED (with the `serde` feature) |
| accuracy of predictions on real hardware | outside this crate; measured for one machine only, in a private research tree |

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
