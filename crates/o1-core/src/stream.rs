//! One sequential pass over an extent in fixed-size blocks.
//!
//! [`BlockPass<N, P>`](BlockPass) walks a byte extent from its start in whole `N`-byte
//! blocks and hands each block to a callback exactly once, in order. The bytes after the
//! last whole block (`len % N` of them) are **not** processed; their count is reported in
//! [`Pass::remainder`] so the caller handles them with its own code — no hidden padding,
//! nothing dropped silently.
//!
//! * [`read`](BlockPass::read): `&[u8; N]` per block (a checksum, a scan).
//! * [`transform`](BlockPass::transform): `(&[u8; N], &mut [u8; N])` per block over an
//!   input and an output of equal length (a one-pass transform into an output buffer).
//!
//! `P` is a zero-sized [`Policy`] stating the caller's intent (`streaming()`,
//! `non_temporal_write()`, …). It changes the published contract (§ working set below), never
//! the code: this is the only, portable, implementation, and it uses ordinary loads and
//! stores whatever `P` requests. `docs/codegen/` records that a policied and an unpolicied
//! pass compile to identical machine code. A later architecture backend (non-temporal
//! stores for `NonTemporal` writes) would be selected from `P` at compile time and checked
//! against this walker; none exists, and a non-temporal store is in any case a hint, not a
//! guarantee that data bypasses the caches.
//!
//! # Contract
//!
//! Per whole block: 1 op; `N` bytes handed to the callback to read; for `transform`, `N`
//! bytes handed to it to write. Working set per block: `N` (read) or `2N` (transform). With
//! `Reuse::Once` (e.g. `streaming()`) each block is released before the next
//! ([`PeakGrowth::Fixed`]); with `Reused` or no statement, the whole extent is assumed to stay
//! resident ([`PeakGrowth::PerUnit`], an upper bound). The callback's own work is the
//! caller's envelope, added per block. Structure: access `Sequential`; `read` is `SharedRead`
//! (it takes `&[u8]`), `transform` is `Exclusive` on its output (it takes `&mut [u8]`).
//!
//! ```
//! use o1_contract::{Policy, Quantity::*};
//! use o1_core::stream::BlockPass;
//!
//! type Pass256 = BlockPass<256, o1_contract::Policy<
//!     o1_contract::policy::Sequential, o1_contract::policy::ReuseOnce,
//!     o1_contract::policy::NonTemporalWrite>>;
//! let pass = BlockPass::<256>::with(Policy::new().streaming().non_temporal_write());
//! let len = if cfg!(miri) { 4096 } else { 4 << 20 }; // 4 MiB natively; small under Miri
//! let input = vec![7u8; len];
//! let mut output = vec![0u8; len];
//! let done = pass.transform(&input, &mut output, |i, o| {
//!     for (a, b) in i.iter().zip(o.iter_mut()) { *b = a ^ 0x5a; }
//! }).unwrap();
//! assert_eq!((done.blocks, done.remainder), (len as u64 / 256, 0)); // 16 384 for 4 MiB
//! assert!(output.iter().all(|&b| b == 7 ^ 0x5a));
//!
//! let job = Pass256::TRANSFORM_CONTRACT.envelope(done.blocks).unwrap();
//! assert_eq!(job.get(Ops), len as u64 / 256);
//! assert_eq!(job.get(BytesRead), len as u64);
//! assert_eq!(job.get(BytesWritten), len as u64);
//! assert_eq!(job.get(WorkingSet), 512); // streaming: one block in, one block out
//! ```
//!
//! Compile-time checks:
//!
//! ```compile_fail,E0080
//! let _ = o1_core::stream::BlockPass::<0>::new(); // a block has at least one byte
//! ```
//! ```compile_fail,E0080
//! // A read-only pass writes nothing: a write policy on it is a contradiction.
//! use o1_contract::Policy;
//! let pass = o1_core::stream::BlockPass::<64>::with(Policy::new().non_temporal_write());
//! let _ = pass.read(&[0u8; 64], |_| {});
//! ```
//! ```compile_fail,E0080
//! // The pass is sequential by structure; a request for random access contradicts it.
//! use o1_contract::Policy;
//! let pass = o1_core::stream::BlockPass::<64>::with(Policy::new().random());
//! let mut out = [0u8; 64];
//! let _ = pass.transform(&[0u8; 64], &mut out, |_, _| {});
//! ```

use core::fmt;
use core::num::NonZeroU64;

use o1_contract::{
    Access, Cost, ExecutionContract, Isolation, PeakGrowth, Policy, PolicyDesc, Progress, Quantity,
    ResourceEnvelope, Reuse, Sharing, StaticPolicy, Unit,
};

/// A pass over fixed `N`-byte blocks with execution policy `P`. Zero-sized.
/// See the [module docs](self).
#[derive(Debug, Clone, Copy)]
pub struct BlockPass<const N: usize, P = Policy> {
    _policy: P,
}

/// What a pass did: whole blocks processed, bytes left after them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub struct Pass {
    /// Whole blocks handed to the callback.
    pub blocks: u64,
    /// Bytes after the last whole block (`< N`), not processed.
    pub remainder: usize,
}

/// [`BlockPass::transform`] was given an input and an output of different lengths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LengthMismatch {
    /// Input length.
    pub input: usize,
    /// Output length.
    pub output: usize,
}

impl fmt::Display for LengthMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "input of {} bytes and output of {} bytes differ",
            self.input, self.output
        )
    }
}

impl core::error::Error for LengthMismatch {}

impl<const N: usize> BlockPass<N> {
    /// A pass with no stated policy.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self::with(Policy::new())
    }

    /// A pass with policy `policy`.
    #[inline]
    #[must_use]
    pub const fn with<P: StaticPolicy>(policy: P) -> BlockPass<N, P> {
        let () = BlockPass::<N, P>::VALID;
        BlockPass { _policy: policy }
    }
}

impl<const N: usize> Default for BlockPass<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize, P: StaticPolicy> BlockPass<N, P> {
    const VALID: () = assert!(N > 0, "BlockPass: N must be at least 1");

    const BLOCK: NonZeroU64 = match NonZeroU64::new(N as u64) {
        Some(b) => b,
        None => panic!("BlockPass: N must be at least 1"),
    };

    /// Structure of [`read`](Self::read): sequential, input borrowed shared.
    pub const READ_STRUCTURE: PolicyDesc = PolicyDesc {
        access: Some(Access::Sequential),
        reuse: None,
        sharing: Some(Sharing::SharedRead),
        write: None,
    };

    /// Structure of [`transform`](Self::transform): sequential, output borrowed exclusively.
    pub const TRANSFORM_STRUCTURE: PolicyDesc = PolicyDesc {
        access: Some(Access::Sequential),
        reuse: None,
        sharing: Some(Sharing::Exclusive),
        write: None,
    };

    /// Contract of [`read`](Self::read). A policy that contradicts its structure is a
    /// compile-time error where `read` is used.
    pub const READ_CONTRACT: ExecutionContract = Self::contract(Self::READ_STRUCTURE, false);

    /// Contract of [`transform`](Self::transform).
    pub const TRANSFORM_CONTRACT: ExecutionContract =
        Self::contract(Self::TRANSFORM_STRUCTURE, true);

    const fn contract(structure: PolicyDesc, writes: bool) -> ExecutionContract {
        let policy = P::DESC.merged_or_fail(&structure);
        let n = Self::BLOCK.get();
        let per_block = ResourceEnvelope::ZERO
            .with(Quantity::Ops, 1)
            .with(Quantity::BytesRead, n)
            .with(Quantity::BytesWritten, if writes { n } else { 0 })
            .with(Quantity::WorkingSet, if writes { 2 * n } else { n });
        ExecutionContract {
            progress: Progress::NoSynchronization,
            policy,
            isolation: Isolation::NoClaim,
            cost: Cost {
                fixed: ResourceEnvelope::ZERO,
                per_unit: per_block,
                unit: Unit::Block { bytes: Self::BLOCK },
                max_units: None,
                peak_growth: match policy.reuse {
                    Some(Reuse::Once) => PeakGrowth::Fixed,
                    _ => PeakGrowth::PerUnit,
                },
            },
        }
        .assert_valid()
    }

    /// Hands each whole block of `input` to `f`, in order, once.
    #[inline]
    pub fn read(&self, input: &[u8], mut f: impl FnMut(&[u8; N])) -> Pass {
        let _checked: ExecutionContract = Self::READ_CONTRACT;
        let mut rest = input;
        while let Some((block, tail)) = rest.split_first_chunk::<N>() {
            f(block);
            rest = tail;
        }
        Pass {
            blocks: ((input.len() - rest.len()) / N) as u64,
            remainder: rest.len(),
        }
    }

    /// Hands each whole block of `input`, with the block at the same offset of `output`, to
    /// `f`, in order, once. The last `remainder` bytes of `output` are left untouched.
    ///
    /// # Errors
    ///
    /// [`LengthMismatch`] if the lengths differ; nothing is processed.
    #[inline]
    pub fn transform(
        &self,
        input: &[u8],
        output: &mut [u8],
        mut f: impl FnMut(&[u8; N], &mut [u8; N]),
    ) -> Result<Pass, LengthMismatch> {
        let _checked: ExecutionContract = Self::TRANSFORM_CONTRACT;
        if input.len() != output.len() {
            return Err(LengthMismatch {
                input: input.len(),
                output: output.len(),
            });
        }
        let mut inp = input;
        let mut out = output;
        while let Some((bi, ti)) = inp.split_first_chunk::<N>() {
            // Lengths are equal, so `out` has a whole block whenever `inp` does.
            let Some((bo, to)) = core::mem::take(&mut out).split_first_chunk_mut::<N>() else {
                break;
            };
            f(bi, bo);
            inp = ti;
            out = to;
        }
        Ok(Pass {
            blocks: ((input.len() - inp.len()) / N) as u64,
            remainder: inp.len(),
        })
    }
}
