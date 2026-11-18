# Platform support

MSRV: **Rust 1.85** for all library crates (edition 2024). Dev-dependencies need newer
compilers (proptest 1.12 → 1.88, criterion 0.8 → 1.86), so the test suite runs on current
stable; `cargo +1.85 check --workspace` is the MSRV gate.

| crate | requirement | what exists where |
|---|---|---|
| `o1-contract` | `#![no_std]`, no `alloc`, no OS, no atomics, `forbid(unsafe_code)` | every target; counts are `u64` on every target (a 32-bit target can describe a job larger than its address space); `lines_touched` uses `u128` arithmetic. |
| `o1-core` | `#![no_std]`, no `alloc`, no OS | every target. Uses `AtomicPtr` (`AtomicTaggedPtr`), which needs `target_has_atomic = "ptr"`. |
| `o1-sync` | `#![no_std]`, no `alloc`, `AtomicUsize` load/store | every target with pointer-width atomics. No 64-bit atomics needed. |
| `o1-linux` | `#![no_std]` | `errno`: every target. `fd`, `io`, `futex`, `uds`: Linux x86_64 and Linux aarch64 only. Linux on any other architecture: `compile_error!`. Other OSes: only `errno`. |
| `o1` | façade | re-exports whatever the crates above provide on that target |

## What was built where (this unit)

| target | command | result |
|---|---|---|
| aarch64-apple-darwin (host) | `cargo test --workspace` and the rest of `docs/VERIFICATION.md` | built and tested |
| thumbv7em-none-eabihf (no OS, no allocator, 32-bit atomics, **no 64-bit atomics**) | `cargo check -p o1-core -p o1-sync --target thumbv7em-none-eabihf`; unit 2: `-p o1-contract -p o1-core -p o1-sync -p o1-linux -p o1`, stable and 1.85 | checked (compiles); not executed |
| x86_64-unknown-linux-gnu | `cargo check -p o1-linux --tests --target …`; `scripts/codegen.sh` | compiled and inspected; **not executed** |
| aarch64-unknown-linux-gnu | same | compiled and inspected; **not executed** |
| wasm32-unknown-unknown | `cargo check -p o1-core -p o1-sync --target wasm32-unknown-unknown` | checked; not executed |

Linux runtime behaviour (syscalls, futex, sockets) has **not** been executed: the
development machine has no Linux runtime. `.github/workflows/ci.yml` would run the Linux
tests on x86_64 and arm64 GitHub runners; it has **not yet run** (nothing was pushed).

## Architecture notes

* `CachePadded` alignment: 128 on x86_64/aarch64/powerpc64, 32 on 32-bit arm/mips/riscv32 and
  a few others, 64 elsewhere.
* The syscall layer assumes the Linux syscall ABI of x86_64 (`syscall`) and aarch64 (`svc 0`),
  LP64 struct layouts (`msghdr` 56 bytes, `iovec` 16, `__kernel_timespec` 16). x32 and ILP32
  aarch64 are not supported (they would compile as `x86_64`/`aarch64` with different layouts;
  the `const` size assertions in `o1_linux::io` would then fail to compile rather than misbehave).
