//! Property tests: every o1-core primitive against an obviously-correct reference model.

use o1_core::arena::{FreeList, GenerationalArena, Handle};
use o1_core::bitops;
use o1_core::collections::{FixedRing, SparseSet};
use o1_core::frame::{DecodeError, FrameDecoder, FrameEncoder, Status};
use proptest::prelude::*;
use std::collections::{BTreeSet, HashMap, VecDeque};

// ---------- bitops: naive bit-by-bit reference ----------

fn ref_tz(x: u64) -> u32 {
    (0..64).find(|&i| x >> i & 1 == 1).unwrap_or(64)
}
fn ref_lz(x: u64) -> u32 {
    (0..64).find(|&i| x >> (63 - i) & 1 == 1).unwrap_or(64)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    #[test]
    fn bitops_u64_match_reference(x in any::<u64>(), i in 0u32..200, c in any::<bool>(), a in any::<u64>()) {
        use bitops::u64 as b;
        prop_assert_eq!(b::trailing_zero_count(x), ref_tz(x));
        prop_assert_eq!(b::leading_zero_count(x), ref_lz(x));
        prop_assert_eq!(b::first_set(x), (x != 0).then(|| ref_tz(x)));
        prop_assert_eq!(b::last_set(x), (x != 0).then(|| 63 - ref_lz(x)));
        prop_assert_eq!(b::pop_count(x), (0..64).filter(|&k| x >> k & 1 == 1).count() as u32);
        let mask: u64 = (0..i.min(64)).fold(0, |m, k| m | 1 << k);
        prop_assert_eq!(b::mask_low(i), mask);
        prop_assert_eq!(b::bit_test(x, i), i < 64 && x >> i & 1 == 1);
        prop_assert_eq!(b::bit_set(x, i), if i < 64 { x | 1 << i } else { x });
        prop_assert_eq!(b::bit_clear(x, i), if i < 64 { x & !(1 << i) } else { x });
        prop_assert_eq!(b::clear_lowest(x), if x == 0 { 0 } else { x & !(1 << ref_tz(x)) });
        prop_assert_eq!(b::isolate_lowest(x), if x == 0 { 0 } else { 1 << ref_tz(x) });
        // Differential: the masked form against the reference form.
        prop_assert_eq!(b::select_masked(c, a, x), b::select(c, a, x));
        let bits: Vec<u32> = b::set_bits(x).collect();
        let expect: Vec<u32> = (0..64).filter(|&k| x >> k & 1 == 1).collect();
        prop_assert_eq!(bits, expect);
    }

    #[test]
    fn bitops_other_widths_agree_with_u64(x in any::<u8>(), y in any::<u16>(), z in any::<u32>(), w in any::<u128>(), i in 0u32..300) {
        prop_assert_eq!(bitops::u8::trailing_zero_count(x), if x == 0 { 8 } else { ref_tz(x as u64) });
        prop_assert_eq!(bitops::u16::leading_zero_count(y), ref_lz(y as u64) - 48);
        prop_assert_eq!(bitops::u32::first_set(z), (z != 0).then(|| ref_tz(z as u64)));
        prop_assert_eq!(bitops::u128::mask_low(i), if i >= 128 { u128::MAX } else { (1u128 << i) - 1 });
        prop_assert_eq!(bitops::u128::bit_test(w, i), i < 128 && w >> i & 1 == 1);
        prop_assert_eq!(bitops::usize::select_masked(i % 2 == 0, 1, 2), if i % 2 == 0 { 1 } else { 2 });
        prop_assert_eq!(bitops::u128::last_set(w), (w != 0).then(|| 127 - w.leading_zeros()));
    }
}

// ---------- FixedRing vs VecDeque ----------

#[derive(Debug, Clone)]
enum RingOp {
    Push(u16),
    Pop,
    Get(usize),
    Clear,
}

fn ring_op() -> impl Strategy<Value = RingOp> {
    prop_oneof![
        4 => any::<u16>().prop_map(RingOp::Push),
        3 => Just(RingOp::Pop),
        1 => (0usize..10).prop_map(RingOp::Get),
        1 => Just(RingOp::Clear),
    ]
}

proptest! {
    #[test]
    fn fixed_ring_matches_vecdeque(ops in prop::collection::vec(ring_op(), 0..400)) {
        let mut r = FixedRing::<u16, 8>::new();
        let mut m = VecDeque::new();
        for op in ops {
            match op {
                RingOp::Push(v) => {
                    let got = r.push_back(v);
                    if m.len() == 8 { prop_assert_eq!(got, Err(v)); } else { prop_assert_eq!(got, Ok(())); m.push_back(v); }
                }
                RingOp::Pop => prop_assert_eq!(r.pop_front(), m.pop_front()),
                RingOp::Get(i) => prop_assert_eq!(r.get(i), m.get(i)),
                RingOp::Clear => { r.clear(); m.clear(); }
            }
            prop_assert_eq!(r.len(), m.len());
            prop_assert!(r.iter().eq(m.iter()));
            prop_assert!(r.iter().rev().eq(m.iter().rev()));
        }
    }

    // ---------- GenerationalArena / FreeList vs a HashMap model ----------

    #[test]
    fn generational_arena_matches_model(ops in prop::collection::vec((any::<bool>(), any::<u8>(), 0usize..64), 0..400)) {
        let mut a = GenerationalArena::<u8, 6>::new();
        let mut live: HashMap<Handle<u8>, u8> = HashMap::new();
        let mut issued: Vec<Handle<u8>> = Vec::new();
        for (insert, v, pick) in ops {
            if insert {
                match a.insert(v) {
                    Ok(h) => {
                        prop_assert!(live.len() < 6);
                        prop_assert!(!issued.contains(&h), "handle reissued: {:?}", h);
                        live.insert(h, v);
                        issued.push(h);
                    }
                    Err(back) => { prop_assert_eq!(back, v); prop_assert_eq!(live.len(), 6); }
                }
            } else if !issued.is_empty() {
                let h = issued[pick % issued.len()];
                prop_assert_eq!(a.remove(h), live.remove(&h));
            }
            prop_assert_eq!(a.len(), live.len());
            for h in &issued {
                prop_assert_eq!(a.get(*h), live.get(h));
            }
        }
    }

    #[test]
    fn free_list_matches_model(ops in prop::collection::vec((any::<bool>(), any::<u8>(), 0usize..8), 0..400)) {
        let mut f = FreeList::<u8, 5>::new();
        let mut live: HashMap<usize, u8> = HashMap::new();
        for (insert, v, idx) in ops {
            if insert {
                match f.insert(v) {
                    Ok(i) => { prop_assert!(i < 5); prop_assert!(live.insert(i, v).is_none()); }
                    Err(back) => { prop_assert_eq!(back, v); prop_assert_eq!(live.len(), 5); }
                }
            } else {
                prop_assert_eq!(f.remove(idx), live.remove(&idx));
            }
            for i in 0..8 { prop_assert_eq!(f.get(i), live.get(&i)); }
            prop_assert_eq!(f.len(), live.len());
        }
    }

    // ---------- SparseSet vs BTreeSet ----------

    #[test]
    fn sparse_set_matches_btreeset(ops in prop::collection::vec((0u8..3, 0usize..40), 0..400)) {
        let mut s = SparseSet::<32>::new();
        let mut m = BTreeSet::new();
        for (kind, x) in ops {
            match kind {
                0 => {
                    let got = s.insert(x);
                    if x >= 32 { prop_assert_eq!(got, Err(x)); } else { prop_assert_eq!(got, Ok(m.insert(x))); }
                }
                1 => prop_assert_eq!(s.remove(x), m.remove(&x)),
                _ => if x % 7 == 0 { s.clear(); m.clear(); },
            }
            for y in 0..40 { prop_assert_eq!(s.contains(y), m.contains(&y)); }
            let mut members: Vec<usize> = s.iter().collect();
            members.sort_unstable();
            prop_assert_eq!(members, m.iter().copied().collect::<Vec<_>>());
        }
    }

    // ---------- framing: encoder → arbitrary partial transport → decoder ----------

    #[test]
    fn frames_roundtrip_under_arbitrary_partial_io(
        frames in prop::collection::vec(prop::collection::vec(any::<u8>(), 0..40), 0..12),
        write_chunks in prop::collection::vec(1usize..9, 1..16),
        read_chunks in prop::collection::vec(1usize..9, 1..16),
    ) {
        // Encode with short writes of varying sizes.
        let mut wire = Vec::new();
        let mut wc = write_chunks.iter().cycle();
        for f in &frames {
            let mut e = FrameEncoder::new(f).unwrap();
            while !e.is_done() {
                let (h, p) = e.pending();
                let want = *wc.next().unwrap();
                let mut n = 0;
                for &byte in h.iter().chain(p.iter()).take(want) { wire.push(byte); n += 1; }
                e.advance(n).unwrap();
            }
        }
        // Decode with short reads of varying sizes.
        let mut buf = [0u8; 40];
        let mut d = FrameDecoder::new(&mut buf);
        let mut rc = read_chunks.iter().cycle();
        let mut rest = &wire[..];
        let mut got: Vec<Vec<u8>> = Vec::new();
        loop {
            let spare = d.spare().unwrap();
            let n = spare.len().min(rest.len()).min(*rc.next().unwrap());
            spare[..n].copy_from_slice(&rest[..n]);
            rest = &rest[n..];
            match d.filled(n).unwrap() {
                Status::NeedMore => {}
                Status::Ready(_) => { got.push(d.frame().unwrap().to_vec()); d.release(); }
                Status::Eof => break,
                Status::Discarded(_) => prop_assert!(false, "nothing should be discarded"),
            }
        }
        prop_assert_eq!(got, frames);
    }

    #[test]
    fn decoder_never_panics_or_overruns_on_arbitrary_bytes(
        wire in prop::collection::vec(any::<u8>(), 0..200),
        chunk in 1usize..13,
        cap in 0usize..20,
        discard in 0usize..64,
    ) {
        let mut buf = vec![0u8; cap];
        let mut d = FrameDecoder::new(&mut buf).with_discard_limit(discard);
        let mut rest = &wire[..];
        let mut steps = 0usize;
        loop {
            steps += 1;
            prop_assert!(steps <= wire.len() + 2, "no progress");
            let spare = match d.spare() { Ok(s) => s, Err(_) => break };
            prop_assert!(!spare.is_empty());
            prop_assert!(spare.len() <= cap.max(4));
            let n = spare.len().min(rest.len()).min(chunk);
            spare[..n].copy_from_slice(&rest[..n]);
            rest = &rest[n..];
            match d.filled(n) {
                Ok(Status::Ready(len)) => { prop_assert!(len <= cap); d.release(); }
                Ok(Status::Eof) => { prop_assert!(rest.is_empty()); break; }
                Ok(_) => {}
                Err(DecodeError::UnexpectedEof { .. }) => { prop_assert!(rest.is_empty()); break; }
                Err(DecodeError::Oversized { len, max }) => { prop_assert!(len as usize > max); break; }
                Err(e) => prop_assert!(false, "unexpected {:?}", e),
            }
        }
    }
}
