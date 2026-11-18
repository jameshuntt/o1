# Changelog — o1-contract

## 0.1.0 (2026-11-18)

First release.

* `Policy` (zero-sized, `const fn` setters, contradictions rejected at compile time) and its
  plain-data mirror `PolicyDesc` with the same validation.
* `ExecutionContract` with `Progress`, `Isolation`, `Cost` and the derived `Bound`.
* `ResourceEnvelope` over eleven `Quantity` counts with checked arithmetic and the composition
  rules `then`, `repeat`, `alongside`, `repeat_retained`, compared with `within`.
* Size multipliers `split_blocks`, `lines_touched`, `Cost::for_extent`.
* `#![no_std]`, no `alloc`, no dependencies, `#![forbid(unsafe_code)]`; MSRV 1.85.
