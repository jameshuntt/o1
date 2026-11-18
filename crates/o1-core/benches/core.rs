//! o1-core micro-benchmarks. `cargo bench -p o1-core`. Numbers: docs/BENCHMARKS.md.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use o1_core::arena::{FreeList, GenerationalArena};
use o1_core::bitops::u64 as b;
use o1_core::collections::{FixedRing, SparseSet};
use o1_core::frame::{FrameDecoder, FrameEncoder, Status};
use std::hint::black_box;
use std::time::Duration;

/// Deterministic xorshift so every run sees the same inputs.
fn xorshift(n: usize, mut s: u64) -> Vec<u64> {
    (0..n)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            s
        })
        .collect()
}

fn bitops(c: &mut Criterion) {
    let xs = xorshift(1024, 0x9e37_79b9_7f4a_7c15);
    let mut g = c.benchmark_group("bitops");
    g.throughput(Throughput::Elements(xs.len() as u64));
    g.bench_function("trailing_zero_count_x1024", |bn| {
        bn.iter(|| {
            xs.iter()
                .map(|&x| b::trailing_zero_count(black_box(x)) as u64)
                .sum::<u64>()
        })
    });
    g.bench_function("first_set_x1024", |bn| {
        bn.iter(|| {
            xs.iter()
                .filter_map(|&x| b::first_set(black_box(x)))
                .map(u64::from)
                .sum::<u64>()
        })
    });
    g.bench_function("set_bits_iterate_x1024", |bn| {
        bn.iter(|| {
            xs.iter()
                .map(|&x| b::set_bits(black_box(x)).map(u64::from).sum::<u64>())
                .sum::<u64>()
        })
    });
    // Selection with unpredictable (random) and predictable (all-true) conditions.
    let random: Vec<bool> = xs.iter().map(|x| x & 1 == 1).collect();
    let fixed = vec![true; xs.len()];
    for (name, conds) in [("random", &random), ("predictable", &fixed)] {
        g.bench_with_input(
            BenchmarkId::new("select_x1024", name),
            conds,
            |bn, conds| {
                bn.iter(|| {
                    conds.iter().zip(&xs).fold(0u64, |acc, (&c, &x)| {
                        acc.wrapping_add(b::select(black_box(c), x, !x))
                    })
                })
            },
        );
        g.bench_with_input(
            BenchmarkId::new("select_masked_x1024", name),
            conds,
            |bn, conds| {
                bn.iter(|| {
                    conds.iter().zip(&xs).fold(0u64, |acc, (&c, &x)| {
                        acc.wrapping_add(b::select_masked(black_box(c), x, !x))
                    })
                })
            },
        );
    }
    g.finish();
}

fn collections(c: &mut Criterion) {
    let mut g = c.benchmark_group("collections");
    g.bench_function("fixed_ring_push_pop_pair", |bn| {
        let mut r = FixedRing::<u64, 1024>::new();
        for i in 0..512 {
            r.push_back(i).unwrap();
        }
        bn.iter(|| {
            r.push_back(black_box(7)).unwrap();
            black_box(r.pop_front())
        })
    });
    g.bench_function("generational_arena_insert_remove_pair", |bn| {
        let mut a = Box::new(GenerationalArena::<u64, 1024>::new());
        for i in 0..512 {
            a.insert(i).unwrap();
        }
        bn.iter(|| {
            let h = a.insert(black_box(7)).unwrap();
            black_box(a.remove(h))
        })
    });
    g.bench_function("generational_arena_get_live", |bn| {
        let mut a = Box::new(GenerationalArena::<u64, 1024>::new());
        let hs: Vec<_> = (0..1024).map(|i| a.insert(i).unwrap()).collect();
        let mut k = 0usize;
        bn.iter(|| {
            k = (k + 389) & 1023;
            black_box(a.get(hs[k]))
        })
    });
    g.bench_function("free_list_insert_remove_pair", |bn| {
        let mut f = Box::new(FreeList::<u64, 1024>::new());
        bn.iter(|| {
            let i = f.insert(black_box(7)).unwrap();
            black_box(f.remove(i))
        })
    });
    g.bench_function("sparse_set_insert_contains_remove", |bn| {
        let mut s = Box::new(SparseSet::<4096>::new());
        let mut k = 0usize;
        bn.iter(|| {
            k = (k + 1597) & 4095;
            let _ = s.insert(black_box(k));
            let hit = s.contains(black_box(k ^ 1));
            s.remove(k);
            hit
        })
    });
    g.bench_function("sparse_set_clear_full_4096", |bn| {
        let mut s = Box::new(SparseSet::<4096>::new());
        bn.iter(|| {
            for x in 0..4096 {
                let _ = s.insert(x);
            }
            s.clear();
        })
    });
    g.finish();
}

fn frame(c: &mut Criterion) {
    // 256 frames of 64 bytes, decoded from one contiguous byte stream with whole reads.
    let payload = [0x5au8; 64];
    let mut wire = Vec::new();
    for _ in 0..256 {
        let mut e = FrameEncoder::new(&payload).unwrap();
        let (h, p) = e.pending();
        wire.extend_from_slice(h);
        wire.extend_from_slice(p);
        let n = e.remaining();
        e.advance(n).unwrap();
    }
    let mut g = c.benchmark_group("frame");
    g.throughput(Throughput::Bytes(wire.len() as u64));
    g.bench_function("decode_256x64B", |bn| {
        let mut buf = [0u8; 64];
        bn.iter(|| {
            let mut d = FrameDecoder::new(&mut buf);
            let mut rest = &wire[..];
            let mut frames = 0;
            loop {
                let spare = d.spare().unwrap();
                let n = spare.len().min(rest.len());
                spare[..n].copy_from_slice(&rest[..n]);
                rest = &rest[n..];
                match d.filled(n).unwrap() {
                    Status::Ready(_) => {
                        frames += 1;
                        d.release();
                    }
                    Status::Eof => break,
                    _ => {}
                }
            }
            black_box(frames)
        })
    });
    g.finish();
}

fn config() -> Criterion {
    Criterion::default()
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(2))
        .sample_size(40)
}

criterion_group! { name = benches; config = config(); targets = bitops, collections, frame }
criterion_main!(benches);
