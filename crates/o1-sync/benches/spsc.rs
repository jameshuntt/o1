//! o1-sync benchmarks. `cargo bench -p o1-sync`. Numbers: docs/BENCHMARKS.md.

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use o1_sync::CachePadded;
use o1_sync::spsc::Ring;
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

const ITEMS: u64 = 1 << 20;

fn spsc(c: &mut Criterion) {
    let mut g = c.benchmark_group("spsc");
    g.bench_function("push_pop_same_thread", |bn| {
        let mut ring = Box::new(Ring::<u64, 1024>::new());
        let (mut p, mut q) = ring.split();
        bn.iter(|| {
            p.push(black_box(1)).unwrap();
            black_box(q.pop())
        })
    });
    g.throughput(Throughput::Elements(ITEMS));
    g.bench_function("two_threads_transfer_1M_u64_cap1024", |bn| {
        bn.iter(|| {
            let mut ring = Box::new(Ring::<u64, 1024>::new());
            let (mut p, mut q) = ring.split();
            std::thread::scope(|s| {
                s.spawn(move || {
                    for i in 0..ITEMS {
                        let mut v = i;
                        while let Err(back) = p.push(v) {
                            v = back;
                            std::hint::spin_loop();
                        }
                    }
                });
                let mut sum = 0u64;
                let mut got = 0;
                while got < ITEMS {
                    if let Some(v) = q.pop() {
                        sum = sum.wrapping_add(v);
                        got += 1;
                    } else {
                        std::hint::spin_loop();
                    }
                }
                black_box(sum)
            })
        })
    });
    g.bench_function("baseline_std_sync_channel_1M_u64_cap1024", |bn| {
        bn.iter(|| {
            let (tx, rx) = std::sync::mpsc::sync_channel::<u64>(1024);
            std::thread::scope(|s| {
                s.spawn(move || {
                    for i in 0..ITEMS {
                        tx.send(i).unwrap();
                    }
                });
                let mut sum = 0u64;
                for _ in 0..ITEMS {
                    sum = sum.wrapping_add(rx.recv().unwrap());
                }
                black_box(sum)
            })
        })
    });
    g.finish();
}

/// What `CachePadded` is for: two threads each hammering their own counter, with the two
/// counters adjacent (same line) versus padded apart.
fn false_sharing(c: &mut Criterion) {
    const N: usize = 1 << 20;
    let mut g = c.benchmark_group("cache_padding");
    g.throughput(Throughput::Elements(2 * N as u64));
    g.bench_function("two_counters_adjacent", |bn| {
        let pair = [AtomicUsize::new(0), AtomicUsize::new(0)];
        bn.iter(|| {
            std::thread::scope(|s| {
                for a in &pair {
                    s.spawn(move || {
                        for _ in 0..N {
                            a.store(a.load(Ordering::Relaxed) + 1, Ordering::Release);
                        }
                    });
                }
            })
        })
    });
    g.bench_function("two_counters_cache_padded", |bn| {
        let pair = [
            CachePadded::new(AtomicUsize::new(0)),
            CachePadded::new(AtomicUsize::new(0)),
        ];
        bn.iter(|| {
            std::thread::scope(|s| {
                for a in &pair {
                    s.spawn(move || {
                        for _ in 0..N {
                            a.store(a.load(Ordering::Relaxed) + 1, Ordering::Release);
                        }
                    });
                }
            })
        })
    });
    g.finish();
}

fn config() -> Criterion {
    Criterion::default()
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(3))
        .sample_size(20)
}

criterion_group! { name = benches; config = config(); targets = spsc, false_sharing }
criterion_main!(benches);
