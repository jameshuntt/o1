//! Non-generic, non-inlined wrappers around o1-core operations, so `scripts/codegen.sh` can
//! compile this crate to assembly and record what each operation becomes on each target.
//! Each function's name is its probe id; the script finds it by that name in the mangled
//! symbol.
//!
//! With the feature `access-counting` (forwarded to `o1-sync`) the SPSC probes are built with
//! the ring's verification counters: the positive control for the expectation that the
//! default build has none.
#![no_std]
#![forbid(unsafe_code)]
#![allow(missing_docs)]

use core::ptr::NonNull;
use o1_contract::Policy;
use o1_core::bitops::{u32 as b32, u64 as b64};
use o1_core::ptr::TaggedPtr;
use o1_core::stream::BlockPass;
use o1_sync::spsc::{Consumer, Producer};

#[inline(never)]
pub fn probe_trailing_zero_count_u64(x: u64) -> u32 {
    b64::trailing_zero_count(x)
}
#[inline(never)]
pub fn probe_trailing_zero_count_u32(x: u32) -> u32 {
    b32::trailing_zero_count(x)
}
#[inline(never)]
pub fn probe_leading_zero_count_u64(x: u64) -> u32 {
    b64::leading_zero_count(x)
}
#[inline(never)]
pub fn probe_first_set_u64(x: u64) -> Option<u32> {
    b64::first_set(x)
}
#[inline(never)]
pub fn probe_last_set_u64(x: u64) -> Option<u32> {
    b64::last_set(x)
}
#[inline(never)]
pub fn probe_pop_count_u64(x: u64) -> u32 {
    b64::pop_count(x)
}
#[inline(never)]
pub fn probe_mask_low_u64(w: u32) -> u64 {
    b64::mask_low(w)
}
#[inline(never)]
pub fn probe_bit_test_u64(x: u64, i: u32) -> bool {
    b64::bit_test(x, i)
}
#[inline(never)]
pub fn probe_isolate_lowest_u64(x: u64) -> u64 {
    b64::isolate_lowest(x)
}
#[inline(never)]
pub fn probe_select_u64(c: bool, a: u64, b: u64) -> u64 {
    b64::select(c, a, b)
}
#[inline(never)]
pub fn probe_select_masked_u64(c: bool, a: u64, b: u64) -> u64 {
    b64::select_masked(c, a, b)
}
#[inline(never)]
pub fn probe_tagged_tag(p: TaggedPtr<u64, 3>) -> usize {
    p.tag()
}
#[inline(never)]
pub fn probe_tagged_untag(p: TaggedPtr<u64, 3>) -> NonNull<u64> {
    p.as_non_null()
}
#[inline(never)]
pub fn probe_tagged_as_ptr(p: TaggedPtr<u64, 3>) -> *mut u64 {
    p.as_ptr()
}

// Policy erasure: the same block pass with and without a zero-sized execution policy. The
// policy changes the published contract only; these pairs must compile to identical code.

#[inline(always)]
fn xor_block(i: &[u8; 64], o: &mut [u8; 64]) {
    for (a, b) in i.iter().zip(o.iter_mut()) {
        *b = a ^ 0x5a;
    }
}

#[inline(always)]
fn sum_block(acc: &mut u64, b: &[u8; 64]) {
    for &x in b {
        *acc = acc.wrapping_add(x as u64);
    }
}

// A closure, not the fn item: the item goes through its `FnMut` shim, which LLVM did not
// inline on aarch64, and the loop body would not be in the record.
#[allow(clippy::redundant_closure)]
#[inline(never)]
pub fn probe_block_transform_plain(input: &[u8], output: &mut [u8]) -> u64 {
    match BlockPass::<64>::new().transform(input, output, |i, o| xor_block(i, o)) {
        Ok(p) => p.blocks,
        Err(_) => u64::MAX,
    }
}
// A closure, not the fn item: the item goes through its `FnMut` shim, which LLVM did not
// inline on aarch64, and the loop body would not be in the record.
#[allow(clippy::redundant_closure)]
#[inline(never)]
pub fn probe_block_transform_policy(input: &[u8], output: &mut [u8]) -> u64 {
    let pass = BlockPass::<64>::with(Policy::new().streaming().non_temporal_write());
    match pass.transform(input, output, |i, o| xor_block(i, o)) {
        Ok(p) => p.blocks,
        Err(_) => u64::MAX,
    }
}
#[inline(never)]
pub fn probe_block_read_plain(input: &[u8]) -> u64 {
    let mut acc = 0u64;
    let _ = BlockPass::<64>::new().read(input, |b| sum_block(&mut acc, b));
    acc
}
#[inline(never)]
pub fn probe_block_read_policy(input: &[u8]) -> u64 {
    let mut acc = 0u64;
    let pass = BlockPass::<64>::with(Policy::new().streaming());
    let _ = pass.read(input, |b| sum_block(&mut acc, b));
    acc
}

// SPSC push/pop (unit 3): the default build must contain no access counters (no thread-local
// access); the `access-counting` build is the positive control.
#[inline(never)]
pub fn probe_spsc_push_u64(p: &mut Producer<'_, u64, 8>, v: u64) -> bool {
    p.push(v).is_ok()
}
#[inline(never)]
pub fn probe_spsc_pop_u64(c: &mut Consumer<'_, u64, 8>) -> u64 {
    c.pop().unwrap_or(u64::MAX)
}
