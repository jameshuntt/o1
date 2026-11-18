# Execution contracts and resource envelopes

Design of the second o1 unit. Written after reading the unit-1 code and before implementing
(commit `ae6a2f9`); afterwards only aligned with the built names and given the computed
numbers and §17. Where it departs from `docs/UNIT2_BRIEF.md` it says so and why (§15 lists
every departure). Evidence levels for every claim are in `O1_EXECUTION_CONTRACTS_REPORT.md`.

```
Primitive + execution intent ──► ExecutionContract ──► ResourceEnvelope        (this unit)
                                                          │
                                         (later, not here) ▼
                     MachineProfile ──► PredictedCost ──► verification ──► admission / quota / placement
```

Built: everything up to and including `ResourceEnvelope`. Everything to the right of it is
a documented seam (§11–§13); nothing of it exists in code.

## 1. Why "O(1)" is not enough for compute accounting

O(1) says the work of one call does not grow with any input size. It says nothing about
*which* work. Read from the unit-1 code:

| operation | O(1)? | what one call structurally does |
|---|---|---|
| `bitops::u64::trailing_zero_count` | yes | one register instruction (codegen record); touches no memory |
| `GenerationalArena::get` | yes | one bounds check, one indexed entry read, one generation compare; returns a reference, copies no payload; no atomics |
| `spsc::Producer::push` | yes | ≤ 1 Acquire load of the consumer's `head` (only when the ring looks full), 1 write of `size_of::<T>()` bytes into a slot the other thread will read, 1 Release store of `tail` which the other thread reads |
| `FrameDecoder::filled` | yes per call | state-machine step; ≤ 4-byte header decode; copies no payload — but a *frame* of `len` bytes takes between 1 and `4 + len` calls |

All four are "O(1)" (per call) and they are not comparable: the second touches a footprint
the size of the arena; the third does two writes that another core must observe (coherence
traffic) and up to one read of a line another core writes; the fourth is O(1) per call and
linear per frame. The unit-1 benchmark showed two threads writing adjacent counters were
1.9–2.8× slower than the same writes on separate padded lines on one machine: the
*structure* (who writes which line) changes the cost while the algorithmic class stays O(1).
That ratio is evidence that structure belongs in the model; it is **not** a constant anywhere
in o1 and never will be (§11).

So a planner needs, per operation: the algorithmic class, the progress class, the access
and ownership structure, and a set of countable quantities — and for a job, those
quantities multiplied out by a known number of bounded executions.

## 2. Semantic execution modifiers

A modifier states how an operation is *meant* to execute. It never promises a hardware
effect. Five categories are kept apart throughout the code and docs:

| category | meaning | example in o1 |
|---|---|---|
| **semantic request** | the caller's statement of intent; a backend may use it to choose an implementation; the result is identical whatever is chosen | `.reuse_once()`, `.non_temporal_write()` |
| **structural guarantee** | a property the code or the type system enforces | SPSC `Handoff` (one producer, one consumer, by `split(&mut self)`); `&mut` output ⇒ `Exclusive`; a block pass walks blocks in order ⇒ `Sequential` |
| **architecture capability** | what a target can do | `CachePadded` alignment per target; pointer-width atomics |
| **implementation hint** | something a backend may do with a request | non-temporal stores for `NonTemporal` writes (no backend in o1 does this yet) |
| **benchmark observation** | a measurement on one machine | padded vs adjacent counters, 1.9–2.8× |

### The vocabulary (smallest that the code justifies)

Four axes. Each axis holds at most one value, so an axis cannot contradict itself.

| axis | values | where the code uses it |
|---|---|---|
| access | `Sequential`, `Random` | block pass and frame decoder walk in order (Sequential); arena operations address a slot chosen by a handle (Random); SPSC slots are used in ring order (Sequential) |
| reuse | `Once`, `Reused` | the caller of a block pass knows whether the extent is read again; it changes the working set the contract reports (§5.4) |
| sharing | `Exclusive`, `SharedRead`, `Handoff` | arena `insert`/`remove` take `&mut` (Exclusive); arena `get` takes `&` (SharedRead); SPSC is Handoff |
| write | `Ordinary`, `NonTemporal` | the block transform accepts a `NonTemporal` request (semantic seam, §9) |

`streaming()` is the readable shorthand for `Sequential` + `Once`.

Rejected or deferred, each with the reason from the code:

| candidate | decision | reason |
|---|---|---|
| `Strided` | rejected | no primitive walks with a stride |
| `Temporal` (as a modifier) | folded | it is `WritePolicy::Ordinary`, the default; no separate marker |
| `Hot` | rejected | indistinguishable from `Reused` at this level; "how hot" would need a number, i.e. a weight (§11) |
| `ReuseLow` | rejected | a third reuse level needs a threshold; no primitive or planner question needs it |
| `Private` | rejected | identical to `Exclusive` |
| `Spsc` | renamed | `Handoff`: the property (each written location is written by one side and read by the other) is what matters, not the queue |
| `Aligned<N>` | rejected | alignment is a layout fact the types already carry (`CachePadded`, `align_of`); a modifier could only repeat or contradict it |
| `Batch<N>` | rejected | nothing in o1 batches publications; the unit-1 report notes batched publication as untried |
| `Block<N>` as a *modifier* | moved | block size is a structural parameter of the primitive (`BlockPass<N, …>`), not an intent; it appears as the cost unit `Unit::Block` |
| `.exclusive()`, `.shared_read()`, `.handoff()` as *static* modifiers | rejected | in Rust the borrow in an operation's signature already states ownership and the compiler proves it (`&mut` = exclusive, `&` = shared read, `split(&mut self)` = handoff). A static modifier could only repeat or contradict a proven fact. Sharing stays in the dynamic descriptor, where a primitive reports it as a structural fact and a planner describes a job with it |
| `.no_cache()`-style names | rejected | they promise hardware effects nothing can guarantee |

## 3. Static vs dynamic policy

**Static form**: `Policy<A, R, W>` — a zero-sized type built by `const fn` typestate
methods:

```rust
use o1_contract::Policy;
let p = Policy::new().streaming().non_temporal_write();
// p: Policy<Sequential, ReuseOnce, NonTemporalWrite>, size 0
```

* An axis can be set once: the setter exists only while the axis is `Unset`, so setting it
  twice is error `E0599` (no such method).
* Real contradictions (§15.1) are compile-time errors `E0080`: every setter evaluates the
  resulting type's `DESC` constant, and `DESC` runs the same `const fn validate` the dynamic
  form uses — one rule, one place, so the static and dynamic checks cannot diverge.
* A marker in the wrong axis position (`Policy<Reused, …>`) is `E0277`: each position is
  bounded by a one-method sealed trait `Mark<Axis>`.
* The policy is consumed by an operation as a type parameter (`BlockPass<N, P>`); the
  operation's code never reads it at run time. Erasure is checked in the codegen record
  (§16 of the report).

**Dynamic form**: `PolicyDesc { access, reuse, sharing, write }`, plain `Option`-valued enums
(`None` = no statement). Reachable from the static form as `P::DESC`. `validate()` returns
an explicit `PolicyError` for the same contradictions; `validate_against(structure)` also
rejects a request that contradicts what the operation structurally does. This is the form a
planner reads and writes, and the form a later verifier compares.

## 4. ExecutionContract

A small composition, each part meaningful alone:

```rust
pub struct ExecutionContract {
    pub progress: Progress,   // NoSynchronization | WaitFree | LockFree | Blocking
    pub policy: PolicyDesc,   // access / reuse / sharing / write (structural facts + requests)
    pub isolation: Isolation, // NoClaim | Padded { unit_bytes } — written control state on its own padded unit
    pub cost: Cost,           // fixed + per-unit ResourceEnvelope, unit, optional max units, peak growth
}
```

`bound()` is *derived* from `cost`, not stored (one rule, one place):

| `Bound` | meaning | condition |
|---|---|---|
| `Constant` | O(1): one execution's envelope does not depend on any size | `cost.per_unit` is zero |
| `PerUnit(unit)` | bounded, linear in a size the caller supplies (bytes, blocks, items, lines) | `per_unit` non-zero, no maximum |
| `Capacity { unit, max }` | linear, and the size cannot exceed a fixed limit (a type capacity or a buffer length) | `per_unit` non-zero, `max_units = Some(max)` |

The properties the brief requires kept apart, and where each lives:

| property | where |
|---|---|
| O(1) algorithmic work | `Bound::Constant` |
| bounded N-dependent work | `Bound::PerUnit` |
| fixed-capacity work | `Bound::Capacity` |
| wait-free / lock-free / blocking | `Progress` |
| external / kernel operation | `KernelEntries` quantity in the envelope (> 0) |
| allocation-free | `Allocations` quantity = 0 (and, for o1 itself, no `alloc` crate anywhere) |
| fixed working set | `WorkingSet` peak with `PeakGrowth::Fixed` (§5.4) |
| cryptographic constant time | **no field; not claimable.** O(1) never implies it |
| bounded wall-clock latency | **no field; not claimable.** No o1 operation claims it |
| unbounded work (spin/park loops, kernel waits) | **no contract can be built**: a `Cost` is finite. `futex::wait_while_equal`'s slow path gets none |

Rejected fields: an "architecture requirements" field (an o1 type that needs pointer-width
atomics does not exist on targets without them — a build fact, not a runtime one); a
separate "allocation behaviour" enum (derivable from the `Allocations` count); a
"constant-time" or "latency" field (not claimable).

## 5. ResourceEnvelope

A vector of countable quantities, each with a named combination rule. No universal compute
unit; no weights.

### 5.1 Quantities (v1)

| quantity | kind | definition (what is counted) |
|---|---|---|
| `Ops` | total | executions of bounded primitive steps (the only "CPU work" o1 can define structurally) |
| `BytesRead` | total | bytes of caller data read: payload moved out of a primitive, or data a pass hands to its callback to read. A primitive's own control fields are not counted here |
| `BytesWritten` | total | bytes of caller data written: payload moved into a primitive, or output a pass hands to its callback to write |
| `AtomicLoads` | total | atomic load instructions |
| `AtomicStores` | total | atomic store instructions |
| `AtomicRmws` | total | atomic read-modify-write instructions (swap, compare-exchange, fetch-op) |
| `SharedReads` | total | reads (plain or atomic) of memory another thread writes by design |
| `SharedWrites` | total | writes (plain or atomic) to memory another thread reads by design — the coherence-sensitive writes |
| `KernelEntries` | total | system calls / kernel entries |
| `Allocations` | total | heap allocations |
| `WorkingSet` | **peak** | bytes the operation may address that must stay resident for its stated reuse (the structure it indexes, plus the data unit in flight) |

`AtomicLoads` and `SharedReads` overlap on purpose (an Acquire load of the other side's
counter counts in both): they answer different questions — instruction kind vs coherence
exposure. An uncontended RMW on private memory is atomic but not shared; a plain slot write
in the SPSC ring is shared but not atomic.

Rejected or deferred quantities:

* **Cache lines** as a stored quantity: the count depends on the line size, a machine
  parameter. The envelope stores bytes; `lines_touched(offset, len, line)` converts at
  planning time with a line size the caller (later: a `MachineProfile`) supplies.
* **Scratch bytes**: a subset of the working set; no primitive needs it reported separately.
* **Instructions / cycles / "compute units"**: not structural; machine-dependent.
* **Separate per-item / per-byte / per-block work fields**: not quantities but cost *shapes*;
  they are `Cost { fixed, per_unit, unit }` (§6).

### 5.2 Representation

`ResourceEnvelope` is an array of `u64`, one per `Quantity`, private, read with
`get(Quantity)` and built with the `const fn with(Quantity, u64)`. All arithmetic is
`const fn`, so primitives publish their contracts as associated constants computed from
`size_of::<T>()`. Counts are `u64` on every target (a 32-bit target can describe a job larger
than its address space).

### 5.3 Combination rules

| operation | totals | peaks | use |
|---|---|---|---|
| `a.then(b)` | `a + b` | `max(a, b)` | `b` runs after `a`, and `a`'s footprint is no longer needed |
| `a.alongside(b)` | `a + b` | `a + b` | both footprints live at once: parallel threads, or an interleaved loop |
| `a.repeat(n)` | `n × a` | `a` (n ≥ 1); zero for n = 0 | n executions over the same footprint (n arena lookups) |
| `a.repeat_retained(n)` | `n × a` | `n × a` | n executions whose footprints all stay live (a reused extent) |

Peaks never add under `then` and `repeat` — the model must not add a peak where the brief
says it combines by max. When liveness is unknown, `alongside` is the conservative choice
(sum ≥ max), and the contracts in o1 use it for a fixed part that stays live through a loop.

### 5.4 Working set and reuse

The reuse axis has a measurable consequence: for a block pass over `len` bytes,

* `Once` (streaming): each block's footprint is released before the next — `PeakGrowth::Fixed`,
  working set = one block in (plus one out for a transform).
* `Reused` or unstated: the extent is expected to be touched again — `PeakGrowth::PerUnit`,
  working set = the whole extent. Unstated reuse takes the upper bound.

This is how "a 64 MiB streaming job with a small working set" becomes distinguishable from
"a 64 MiB job that needs 64 MiB resident" without a single machine number.

### 5.5 Comparisons

`within(&limit) -> bool` and `check_within(&limit) -> Result<(), Exceeded>` compare every
quantity, componentwise `≤` (a partial order). The same relation serves both later users:
`observed.within(&declared)` (a verifier) and `job.within(&allowance)` (a planner). The
brief proposed two names (`within`, `fits`) for this one relation; one name is kept.

## 6. Fixed and per-unit costs

```rust
pub struct Cost {
    pub fixed: ResourceEnvelope,     // once per execution (setup, or the whole of an O(1) op)
    pub per_unit: ResourceEnvelope,  // per unit of size
    pub unit: Unit,                  // Item | Byte | Block { bytes } | Line { bytes }
    pub max_units: Option<u64>,      // a fixed capacity or buffer bound, if any
    pub peak_growth: PeakGrowth,     // Fixed | PerUnit (§5.4)
}
cost.at(units) = fixed.alongside(per_unit.repeat(units))            // PeakGrowth::Fixed
               = fixed.alongside(per_unit.repeat_retained(units))   // PeakGrowth::PerUnit
```

`at` returns `Err(AboveBound)` when `units > max_units`, `Err(Overflow{quantity})` when a
count does not fit `u64`. The fixed part is combined with `alongside` because setup state
stays live through the loop. An O(1) operation has a zero `per_unit`; n executions of it are
`cost.at(0)?.repeat(n)?`.

## 7. Size multipliers

A bounded block primitive does not make processing `n` bytes O(1). Conversions (all `const
fn`, all total — none can overflow, proved by the bounds in their docs):

* `split_blocks(len, block) -> Blocks { full, remainder }`: `full = len / block`,
  `remainder = len % block`. `Blocks::covering() = full + (remainder > 0)`: the remainder is
  costed as one more full block (upper bound). It is never dropped.
* `lines_touched(offset, len, line)`: lines an extent starting `offset` bytes into a line
  touches: `0` for `len = 0`, else `(offset % line + len + line − 1) / line`, computed in
  `u128`. An unaligned start touches one more line than an aligned one when it crosses a
  boundary. The line size is a parameter, never a constant 64 or 128.
* `Cost::for_extent(offset, len)` converts by the cost's unit: `Byte` → `len`; `Block` →
  `covering` blocks (upper bound); `Line` → `lines_touched`; `Item` → `Err(NotAnExtentUnit)`
  (items are not derivable from bytes).

An *exact* remainder (rather than the upper bound) is the caller's choice: a block pass
processes full blocks only and hands back the remainder; the job envelope is
`pass.at(full)` plus whatever the caller does with `remainder` bytes, with its own envelope.

Worked size multiplier (a doc-test): primitive = one 256-byte block; job = 4 MiB; count =
16 384 executions; total = setup + 16 384 × per-block envelope.

### Overflow

Every scaling and sum is **checked** and returns `EnvelopeError::Overflow { quantity }`.
Nothing wraps; nothing saturates. A saturating `u64::MAX` would compare as "within" an
allowance of `u64::MAX` and would hide which quantity broke; an explicit error cannot be
mistaken for a small estimate. `max` (peaks under `then`) cannot overflow.

## 8. Cache and coherence semantics

o1 does not model MESI or any protocol. It represents what software controls and what the
code structurally does:

| software-controlled fact | representation |
|---|---|
| ownership separation (producer-owned / consumer-owned state) | `Sharing::Handoff`, plus `SharedReads`/`SharedWrites` counts per operation |
| false-sharing avoidance / control-line isolation | `Isolation::Padded { unit_bytes }` — the written control words sit on separate units of `align_of::<CachePadded<_>>()` bytes (128 / 64 / 32 by target). Tested by field offsets |
| read-mostly / shared-read access | `Sharing::SharedRead` (from `&` borrows) |
| sequential streaming | `Access::Sequential` + `Reuse::Once` |
| working-set size | `WorkingSet` peak |
| reuse expectation | `Reuse`, and its effect on `WorkingSet` (§5.4) |
| atomic / coherence-sensitive operations | `AtomicLoads/Stores/Rmws`, `SharedReads/Writes` |

What `Isolation::Padded` does **not** cover is stated in the SPSC contract docs: the slots
are not padded; adjacent slots can share a line, so a producer writing slot *k* and a
consumer reading slot *k − 1* can still contend for one line. The counts say *that* a write
is shared; the price of a shared write on a given machine is a calibration question (§11).

## 9. Streaming semantics

Order of construction, as the brief asks:

1. **Semantic behaviour** — `BlockPass<N, P>` (in `o1_core::stream`): walk an extent in
   order, in full `N`-byte blocks, each block handed to the callback exactly once; the
   remainder (`len % N` bytes) is not processed and is reported back. `read` hands
   `&[u8; N]`; `transform` hands `(&[u8; N], &mut [u8; N])` over equal-length input and
   output. No allocation, no `unsafe` (`split_first_chunk`, Rust 1.77).
2. **Portable reference implementation** — that walker is the only implementation. It uses
   ordinary loads and stores whatever the policy requests.
3. **Resource contract** — `READ_CONTRACT` / `TRANSFORM_CONTRACT`: per block 1 op, `N` bytes
   read (and `N` written), working set per §5.4; the callback's own work is the caller's
   envelope, added per block. A property test counts callback calls and bytes and compares
   them with the formula exactly.
4. **Backend seam** — `P::DESC.write` is visible to the implementation at compile time. A
   later x86_64 backend could use non-temporal stores (`MOVNTDQ` family), an aarch64 backend
   `STNP`, for `NonTemporal` requests on blocks of a suitable size and alignment, with the
   portable walker kept as the semantic reference and a differential test between them. None
   is written: no measurement in this repository shows a benefit, and the brief forbids
   handwritten SIMD without evidence. Callers never name an instruction. A non-temporal store
   is a *hint* on both architectures: it does not guarantee that the data bypasses every cache
   level, and o1 will never say it does.

The pass is kept (not deferred) because it is ~60 lines without `unsafe`, it is the one
place the size-multiplier semantics can be checked end to end against an observed count,
and it carries the write-policy request to the place a backend would read it.

## 10. Architecture-independent intent

Every public name is semantic (`Sequential`, `ReuseOnce`, `NonTemporalWrite`, `Handoff`).
No x86 or ARM instruction name appears in a public API. Per-target facts that exist today
(`CachePadded` alignment) enter contracts as numbers computed by `align_of` on the target
being compiled, so the same source states the right value on every target.

## 11. Deferred machine calibration (seam only)

A later `MachineProfile` attaches weights to quantities — e.g. a measured cost per shared
write, per atomic RMW, per byte streamed, the line size for `lines_touched`. Its input is a
`ResourceEnvelope` (or a `Cost` plus a size), its output a predicted cost with its own
evidence level. o1 contains **no weight**: no "atomic = 11.4", no cache-miss constant, no
padding ratio. The quantities are chosen so that each is countable by construction and has
an obvious calibration experiment.

## 12. Future Custodian verification (seam only)

o1 states claims; a later verifier checks them. The interface is plain data, no Custodian
dependency:

| check | o1 provides |
|---|---|
| allocations == 0 | `Allocations` quantity; o1 crates have no `alloc` |
| bounded iterations ≤ K | `Ops`; `Cost::max_units`; `Bound` |
| working set ≤ W | `WorkingSet` |
| atomic operations ≤ A | `AtomicLoads/Stores/Rmws` |
| envelope formula matches observed structural counts | `observed.check_within(&declared)`; the counting tests in this unit are the reference adapter shape |
| portable and optimised backends equivalent | `BlockPass` is the portable reference; a backend must equal it (differential test) |
| streaming specialisation preserves semantics | the pass's semantics are independent of `P`; erasure checked by codegen |
| codegen property still holds | `scripts/codegen.sh` expectations |
| performance regression within declared limits | not provided: o1 declares no performance limit |

## 13. Future fleet quota / admission / placement use (seam only)

A planner keeps the envelope multidimensional and compares it per quantity against an
allowance. Worked pair (a doc-test in `o1-contract`):

* **Job A** — 64 MiB sequential streaming transform in 256-byte blocks, 4 KiB of hot state,
  no synchronisation: 262 144 ops, 64 MiB read, 64 MiB written, 0 atomics, 0 shared
  accesses, working set 4 KiB + 512 B.
* **Job B** — 2 MiB of 64-byte records, each one atomic RMW on a 64 KiB shared table:
  32 768 ops, 2 MiB read, 32 768 atomic RMWs, 32 768 shared reads and writes, working set
  64 KiB + 64 B.

Neither is `within` the other: A dominates in bytes, B in atomics and shared writes. A single
compute number would have to rank them, which needs exactly the machine weights o1 refuses
to invent.

### Archive-style example (abstract; no archive types in o1)

A large sequential extent (say 4 GiB) is read from a file in 1 MiB reads, cut into 256-byte
blocks, each block transformed once into an output buffer of the same size, which is
written out in 1 MiB writes:

```
pass   = BlockPass::<256, Policy<Sequential, ReuseOnce, NonTemporalWrite>>::TRANSFORM_CONTRACT
blocks = split_blocks(4 GiB, 256)            → 16 777 216 full, 0 remainder
io     = ZERO.with(KernelEntries, 2)          per 1 MiB chunk (one read, one write) — caller's envelope
job    = pass.cost.at(blocks.full)?          → ops 16 777 216, read 4 GiB, written 4 GiB, WS 512 B
           .alongside(io.repeat(4096)?)?     → + 8192 kernel entries
           .alongside(hot_state)?            → + 4 KiB of the caller's control state
       = "ops=16777216, bytes_read=4294967296, bytes_written=4294967296,
          kernel_entries=8192, working_set=4608"
```

(Computed by the test `archive_style_example_numbers` in `crates/o1-core/tests/contracts.rs`.)
The planner sees: 4 GiB streamed each way, a working set of 4.5 KiB plus the caller's I/O
buffers, no atomics, 8 192 kernel entries. Retention, replicas and what the records mean
stay above o1 (§14).

## 14. o1 scope boundaries

In o1: primitive representation, bounded work, fixed capacities, access intent, ownership
semantics relevant to cache coherence, atomic/synchronisation characteristics, backend
selection, resource quantity descriptions.

Not in o1: archive retention, replica placement, record meaning, organisational policy,
governance, billing, quotas themselves, node selection, scheduling algorithms, experiment
orchestration, machine weights. No archive-domain type exists in o1.

## 15. Decisions, departures from the brief, and rejected designs

### 15.1 Contradictions

| combination | decision | why |
|---|---|---|
| two values on one axis (`Sequential`+`Random`, `Once`+`Reused`, `Hot`+`ReuseOnce`, `Private`+`SharedRead`) | impossible by construction | one slot per axis; setting twice is `E0599` |
| `NonTemporal` + `Reused` | contradiction (`E0080` / `PolicyError::NonTemporalReused`) | a non-temporal write *is* the statement that the written bytes will not be reused soon; the reuse axis covers the bytes the operation touches |
| `NonTemporal` + `SharedRead` | contradiction (`PolicyError::WriteOnSharedRead`) | `SharedRead` means nobody writes the data during the operation; a write policy on it is meaningless |
| a request that differs from the operation's structure (`.random()` on a sequential pass; `NonTemporal` on a read-only pass, whose structure is `SharedRead`) | contradiction (`PolicyError::ContradictsStructure` / the `SharedRead` rule) | the structure is proven; a request cannot change it |
| `NonTemporal` + `Handoff` | allowed | a large message consumed later by another thread is a legitimate non-temporal case |
| `Random` + `Once` | allowed | one pass of random probes into a large table is real |
| `Sequential` + `Reused` | allowed | repeated scans of a small buffer |

### 15.2 Departures from the brief's starting positions

| position | outcome |
|---|---|
| 1 placement | kept: new crate `o1-contract`, `no_std`, no dependencies, `forbid(unsafe_code)`; `o1-core`/`o1-sync` depend on it only to publish their contracts |
| 2 axes | sharing kept in the descriptor but **not** in the static form (§2); `Block<N>` moved to the primitive and the cost unit; write default named `Ordinary` |
| 3 static form | `Policy<A, R, W>` (three axes, not four); contradictions are `E0080` via the shared `const fn validate`, double-set is `E0599` |
| 4 dynamic form | kept; adds `validate_against(structure)` |
| 5 contract | `Bound` derived from `Cost` instead of stored; `Isolation` added (the one structural coherence fact in the tree) |
| 6 quantities | kept, plus `Ops` and `AtomicRmws`; scratch deferred; atomic stores and RMWs split |
| 7 cost shape | kept, plus `max_units` and `PeakGrowth` (needed for reuse, §5.4) |
| 8 multipliers | kept |
| 9 overflow | kept: checked, explicit error naming the quantity |
| 10 streaming | kept: portable walker + seam; no backend |
| 11 comparisons | one relation, `within` / `check_within` (the brief's `fits` is the same relation) |
| 12, 13 | kept |

### 15.3 Rejected designs

* **A single "compute unit".** Ranks jobs A and B (§13) only by inventing weights.
* **A struct of many named `u64` fields with per-field arithmetic.** An indexed array with a
  `Quantity` enum keeps the combination rule in one place (`Quantity::kind`).
* **Saturating arithmetic.** Hides overflow as a large-but-plausible number (§7).
* **Integer bitflags for policy.** Unreadable, and contradictions become run-time checks.
* **A trait per modifier / trait towers.** One sealed generic trait `Mark<Axis>` and one
  `StaticPolicy` trait carry the static form.
* **Cargo features named after modifiers.** Features compile code in or out; a policy
  changes no code in this unit.
* **Contracts on every primitive.** See §16.

## 16. Which primitives get contracts

| primitive | contract? | reason |
|---|---|---|
| `bitops`, `TaggedPtr` | no | register-only; a planner gains nothing |
| `AtomicTaggedPtr` | no | one atomic instruction per call; whether the word is shared is the caller's fact, so the contract could only say "1 atomic", which the planner states directly |
| `CachePadded` | no (used *by* the SPSC contract's `Isolation`) | a layout fact |
| `spsc::Ring` push / pop | **yes** | bounded, wait-free, allocation-free, known atomics, ownership-separated — the one place coherence traffic is structural. Counted by an instrumented test build |
| `GenerationalArena` insert / remove / get | **yes** | the handle-addressed store a higher layer keys records by; O(1), random access, footprint = the arena. Numbers from reading |
| `FreeList`, `FixedRing`, `SparseSet` | no | same shape as the arena (or the SPSC ring without sharing); no planner question needs a second copy |
| `FrameDecoder` (per frame) | **yes** | explicitly size-dependent: ≤ 4 + len steps per frame, bounded by the buffer and discard limit. Counted by a property test |
| `FrameEncoder` | no | same shape as the decoder; no planning question |
| `o1-linux` (`io`, `futex`, `uds`) | no | kernel work is outside any bound and nothing in it has run (Linux runtime NOT YET VERIFIED). `KernelEntries` exists so a caller can describe it |
| `o1_core::stream::BlockPass` (new) | **yes** | the size-multiplier and streaming reference; counted exactly |

## 17. How each contract is checked

| contract | check | evidence |
|---|---|---|
| `spsc::Ring::{PUSH,POP}_CONTRACT` | In o1-sync's own unit tests the `sync` switch module counts every atomic load/store and every slot access (forwarding to the real atomics). The test drives fill, full, drain, empty and both refresh paths for four element types and compares each call's counts with the declared envelope; the two refresh paths equal it exactly. A planted extra `tail` store made the test fail. `Isolation::Padded`: unit = size = alignment of `CachePadded<AtomicUsize>`, both counters at unit-aligned offsets. | TESTED (also under Miri) |
| `BlockPass::{READ,TRANSFORM}_CONTRACT` | Property test: callback calls and bytes handed over equal `at(full)` exactly for N ∈ {1, 3, 64, 256} and lengths 0–3000; transform output equals a byte-wise reference, remainder untouched. | TESTED |
| `FrameDecoder::frame_contract` | Property test: `filled` calls per frame under random read sizes stay within the declared bound; one-byte reads reach it exactly (lengths 0–512, including discarded frames). | TESTED |
| `GenerationalArena::{INSERT,REMOVE,GET}_CONTRACT` | Shape test only; numbers read from the code (no loop, one indexed entry access). | ASSUMED |
| working-set numbers (all contracts) | A definition (§5.1), not an observation: no test can count residency. | ASSUMED (by definition) |
| policy erasure | `scripts/codegen.sh`: policied and unpolicied `BlockPass` probes are folded into one symbol on x86_64 (baseline, BMI) and aarch64. | CODEGEN-VERIFIED |
| contradictions | 20 `compile_fail` doc-tests in the workspace (o1-contract 6, o1-core 11 of which 3 new, o1-sync 3); their error codes are verified only by **nightly** rustdoc (`cargo +nightly test --workspace --doc`) — stable rustdoc runs them but ignores the code (checked by planting a wrong code: stable passed, nightly failed). | PROVEN (nightly-checked codes) |
