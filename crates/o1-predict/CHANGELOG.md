# Changelog — o1-predict

## 0.1.0 (2026-11-18)

First release.

* Class identity: `OperationId`, `Implementation`, `ClassKey`, `KeyPart`, `OperationClass`;
  exact equality only.
* `Geometry` and `Parameter`; placement inside a measured curve, no extrapolation.
* Plain evidence types `Evidence`, `ProfileEvidence`, `CalibrationEvidence`, `Now`.
* The four-step selector (applicability, class selection, placement, interval), `Interval`
  and its compositions, `Plan` with exact / expected / worst-case / unknown counts.
* `predict` and `predict_paths` returning `Prediction` or a machine-readable `PlanRefusal`
  (22 `Refusal` variants); selection revisions `Options::CLASS_A` and `Options::CLASS_B`.
* Feature `serde`: serialize / deserialize every public type except the validation errors
  `KeyError`, `GeometryError` and `EvidenceError` (derives only, no `std`).
* `#![no_std]` + `alloc`, `#![forbid(unsafe_code)]`; depends only on `o1-contract` 0.1;
  MSRV 1.85.
