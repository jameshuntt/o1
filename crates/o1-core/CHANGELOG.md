# Changelog — o1-core

## 0.1.0 (2026-11-18)

First release.

* `bitops`: bit scans, masks and selection with defined results for zero input and
  out-of-range widths.
* `ptr`: `TaggedPtr`, `AtomicTaggedPtr` (strict provenance, compile-time tag-width check) and
  alignment helpers.
* `collections`: `FixedRing`, `SparseSet`.
* `arena`: `FreeList`, `GenerationalArena`, `Handle`, with execution contracts for arena
  insert, remove and get.
* `frame`: sans-IO `FrameDecoder` / `FrameEncoder` (4-byte little-endian length prefix).
* `stream`: `BlockPass` with read and transform contracts.
* `#![no_std]`, no `alloc`, no OS calls; depends only on `o1-contract` 0.1; MSRV 1.85.
