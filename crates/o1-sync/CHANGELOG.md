# Changelog — o1-sync

## 0.1.0 (2026-11-18)

First release.

* `spsc::Ring<T, N>` with `Producer` / `Consumer` handles from `split`: wait-free `push`,
  `pop`, `free_slots`, `available`; release/acquire orderings documented in the `spsc`
  module; execution contracts `PUSH_CONTRACT` and `POP_CONTRACT`.
* `CachePadded<T>`.
* Feature `access-counting` (off by default, `std`): per-thread counts of the ring's atomic and
  slot accesses, for verification tooling only.
* `#![no_std]`, no `alloc`, no OS calls; depends only on `o1-contract` 0.1; MSRV 1.85.
