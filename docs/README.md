# Documents

| file | contents |
|---|---|
| [`DESIGN.md`](DESIGN.md) | design of the substrate crates, with the audit it was written after |
| [`EXECUTION_CONTRACTS.md`](EXECUTION_CONTRACTS.md) | execution policies, contracts and resource envelopes |
| [`GUARANTEES.md`](GUARANTEES.md) | per-operation guarantee matrix |
| [`PLATFORM_SUPPORT.md`](PLATFORM_SUPPORT.md) | targets, MSRV, `no_std` status |
| [`SAFETY_MODEL.md`](SAFETY_MODEL.md) | unsafe policy |
| [`UNSAFE_INVENTORY.md`](UNSAFE_INVENTORY.md) | every unsafe site and the invariant it relies on (checked by `cargo run -p xtask -- unsafe-inventory`) |
| [`RELEASING.md`](RELEASING.md) | how a crate is released |
| [`codegen/`](codegen) | recorded assembly of selected operations and the expectations checked against it |

These documents were written in a larger research tree. References in them to files that are
not in this repository (verification records, benchmarks, limitations, design briefs,
reports, evidence, scripts, the calibration and prediction laboratories) point into that
private research tree.
