use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use ts4::{
    Block, SolveMode, TS4, Trace, blocks_l1_gt, blocks_l1_gt_mask, left_divide_trace,
    left_divide_ts4, left_divide_ts4_solve, left_divide_ts4_unique, odot_kappa, otimes_kappa,
    parallel_kappa, phi_kappa, sum_l1_blocks,
};

// 7950X policy: treat benchmark results as affinity-class samples, not as a
// synthetic average. Same-CCD, cross-CCD, and unpinned runs should be compared
// separately because the two CCDs own separate L3 domains.
// Paired fast-path vs split-heavy workloads keep recovery waves comparable.
// Compare these pairs inside one artifact class at a time; do not mix
// unpinned, same-CCD-proxy, and cross-CCD-proxy outputs as one baseline.
const KAPPA: u32 = 11;
const SIZES: &[usize] = &[256, 4096, 16384];
const FAST_BLOCK: Block = Block::new(1, 1, 1);
const FAST_PARALLEL_RIGHT_BLOCK: Block = Block::new(1, 1, 1);
const FAST_ODOT_RIGHT_BLOCK: Block = Block::new(1, 0, 1);
const FAST_OTIMES_RIGHT_BLOCK: Block = Block::new(1, 0, 1);
const SPLIT_BLOCK: Block = Block::new(4, 4, 4);
const TRACE_LAYERS: usize = 1024;
const ODOT_LEFT_LAYERS: usize = 512;
const ODOT_RIGHT_LAYERS: usize = 513;
const OTIMES_LEFT_LAYERS: usize = 512;
const OTIMES_RIGHT_LAYERS: usize = 9;
const OTIMES_MATERIAL_LEFT_LAYERS: usize = 512;
const OTIMES_MATERIAL_RIGHT_LAYERS: &[usize] = &[3, 33];
const MATERIAL_KAPPA: u32 = 1_000_000;

fn make_blocks(len: usize) -> Vec<Block> {
    (0..len)
        .map(|i| Block::new((i % 7) as u32, (i % 5) as u32, (i % 3) as u32))
        .collect()
}

fn make_uniform_blocks(len: usize, block: Block) -> Vec<Block> {
    vec![block; len]
}

fn make_zero_blocks(len: usize) -> Vec<Block> {
    vec![Block::zero(); len]
}

fn make_trace(len: usize) -> Trace {
    Trace::new(
        (0..len)
            .map(|i| Block::new((i % 9) as u32, ((i * 3) % 7) as u32, ((i * 5) % 11) as u32))
            .collect(),
    )
}

fn make_uniform_trace(len: usize, block: Block) -> Trace {
    Trace::new(vec![block; len])
}

fn make_ts4_pair() -> (TS4, TS4) {
    let left = TS4::from_trace(make_trace(4), 3)
        .add(&TS4::from_trace(make_trace(6), 2))
        .add(&TS4::from_trace(make_trace(2), 1));
    let right = TS4::from_trace(make_trace(5), 4)
        .add(&TS4::from_trace(make_trace(3), 5))
        .add(&TS4::from_trace(make_trace(1), 7));
    (left, right)
}

fn make_solver_pair() -> (TS4, TS4) {
    let ax = TS4::from_trace(Trace::new(vec![Block::new(1, 0, 0)]), 1);
    let ay = TS4::from_trace(Trace::new(vec![Block::new(0, 1, 0)]), 1);
    let a = ax.add(&ay);
    let b = TS4::from_trace(Trace::new(vec![Block::new(1, 1, 0)]), 2);
    (a, b)
}

fn bench_block_kernels(c: &mut Criterion) {
    let mut group = c.benchmark_group("hot-kernels");

    for &len in SIZES {
        let blocks = make_blocks(len);
        group.throughput(Throughput::Elements(len as u64));

        group.bench_with_input(
            BenchmarkId::new("sum_l1_blocks", len),
            &blocks,
            |b, blocks| {
                b.iter(|| {
                    let result = sum_l1_blocks(black_box(blocks.as_slice()));
                    black_box(result);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("blocks_l1_gt", len),
            &blocks,
            |b, blocks| {
                b.iter(|| {
                    let mask = blocks_l1_gt(black_box(blocks.as_slice()), black_box(KAPPA));
                    black_box(mask);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("blocks_l1_gt_mask", len),
            &blocks,
            |b, blocks| {
                b.iter(|| {
                    let mask = blocks_l1_gt_mask(black_box(blocks.as_slice()), black_box(KAPPA));
                    black_box(mask);
                });
            },
        );

        if len <= 4096 {
            let split_blocks = make_uniform_blocks(len, SPLIT_BLOCK);
            group.bench_with_input(
                BenchmarkId::new("blocks_l1_gt", format!("split-heavy/{len}")),
                &split_blocks,
                |b, blocks| {
                    b.iter(|| {
                        let mask = blocks_l1_gt(black_box(blocks.as_slice()), black_box(KAPPA));
                        black_box(mask);
                    });
                },
            );
            group.bench_with_input(
                BenchmarkId::new("blocks_l1_gt_mask", format!("split-heavy/{len}")),
                &split_blocks,
                |b, blocks| {
                    b.iter(|| {
                        let mask =
                            blocks_l1_gt_mask(black_box(blocks.as_slice()), black_box(KAPPA));
                        black_box(mask);
                    });
                },
            );
        }
    }

    group.finish();
}

fn bench_trace_kernels(c: &mut Criterion) {
    let mut group = c.benchmark_group("trace-physics");
    let fast_phi = make_uniform_trace(TRACE_LAYERS, FAST_BLOCK);
    let split_phi = make_uniform_trace(TRACE_LAYERS, SPLIT_BLOCK);
    let fast_parallel_left = make_uniform_trace(TRACE_LAYERS, FAST_BLOCK);
    let fast_parallel_right = make_uniform_trace(TRACE_LAYERS + 8, FAST_PARALLEL_RIGHT_BLOCK);
    let split_parallel_left = make_uniform_trace(TRACE_LAYERS, SPLIT_BLOCK);
    let split_parallel_right = make_uniform_trace(TRACE_LAYERS + 8, SPLIT_BLOCK);
    let fast_odot_left = make_uniform_trace(ODOT_LEFT_LAYERS, FAST_BLOCK);
    let fast_odot_right = make_uniform_trace(ODOT_RIGHT_LAYERS, FAST_ODOT_RIGHT_BLOCK);
    let split_odot_left = make_uniform_trace(ODOT_LEFT_LAYERS, SPLIT_BLOCK);
    let split_odot_right = make_uniform_trace(ODOT_RIGHT_LAYERS, SPLIT_BLOCK);
    let fast_otimes_left = make_uniform_trace(OTIMES_LEFT_LAYERS, FAST_BLOCK);
    let fast_otimes_right = make_uniform_trace(OTIMES_RIGHT_LAYERS, FAST_OTIMES_RIGHT_BLOCK);
    let split_otimes_left = make_uniform_trace(OTIMES_LEFT_LAYERS, SPLIT_BLOCK);
    let split_otimes_right = make_uniform_trace(OTIMES_RIGHT_LAYERS, SPLIT_BLOCK);
    let left = make_trace(1024);
    let right = make_trace(1032);
    let ts4_pair = make_ts4_pair();

    group.throughput(Throughput::Elements(left.len_blocks() as u64));

    group.bench_with_input(
        BenchmarkId::new("phi_kappa", "fast-path"),
        &fast_phi,
        |b, trace| {
            b.iter(|| {
                let normalized = phi_kappa(black_box(trace), black_box(KAPPA));
                black_box(normalized);
            });
        },
    );

    group.bench_with_input(
        BenchmarkId::new("phi_kappa", "split-heavy"),
        &split_phi,
        |b, trace| {
            b.iter(|| {
                let normalized = phi_kappa(black_box(trace), black_box(KAPPA));
                black_box(normalized);
            });
        },
    );

    group.bench_with_input(
        BenchmarkId::new("parallel_kappa", "fast-path"),
        &fast_parallel_left,
        |b, left| {
            b.iter(|| {
                let out = parallel_kappa(
                    black_box(left),
                    black_box(&fast_parallel_right),
                    black_box(KAPPA),
                );
                black_box(out);
            });
        },
    );

    group.bench_with_input(
        BenchmarkId::new("parallel_kappa", "split-heavy"),
        &split_parallel_left,
        |b, left| {
            b.iter(|| {
                let out = parallel_kappa(
                    black_box(left),
                    black_box(&split_parallel_right),
                    black_box(KAPPA),
                );
                black_box(out);
            });
        },
    );

    group.bench_with_input(
        BenchmarkId::new("odot_kappa", "fast-path"),
        &fast_odot_left,
        |b, left| {
            b.iter(|| {
                let out = odot_kappa(
                    black_box(left),
                    black_box(&fast_odot_right),
                    black_box(KAPPA),
                );
                black_box(out);
            });
        },
    );

    group.bench_with_input(
        BenchmarkId::new("odot_kappa", "split-heavy"),
        &split_odot_left,
        |b, left| {
            b.iter(|| {
                let out = odot_kappa(
                    black_box(left),
                    black_box(&split_odot_right),
                    black_box(KAPPA),
                );
                black_box(out);
            });
        },
    );

    group.bench_with_input(
        BenchmarkId::new("otimes_kappa", "fast-path"),
        &fast_otimes_left,
        |b, left| {
            b.iter(|| {
                let out = otimes_kappa(
                    black_box(left),
                    black_box(&fast_otimes_right),
                    black_box(KAPPA),
                );
                black_box(out);
            });
        },
    );

    group.bench_with_input(
        BenchmarkId::new("otimes_kappa", "split-heavy"),
        &split_otimes_left,
        |b, left| {
            b.iter(|| {
                let out = otimes_kappa(
                    black_box(left),
                    black_box(&split_otimes_right),
                    black_box(KAPPA),
                );
                black_box(out);
            });
        },
    );

    group.bench_function("trace_compose", |b| {
        b.iter(|| {
            let out = black_box(&left).compose(black_box(&right));
            black_box(out);
        });
    });

    group.bench_function("ts4_compose", |b| {
        b.iter(|| {
            let out = black_box(&ts4_pair.0).compose(black_box(&ts4_pair.1));
            black_box(out);
        });
    });

    group.finish();
}

fn bench_otimes_materialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("otimes-materialization");
    let left = make_uniform_trace(OTIMES_MATERIAL_LEFT_LAYERS, FAST_BLOCK);

    for &v_len in OTIMES_MATERIAL_RIGHT_LAYERS {
        let right = make_uniform_trace(v_len, FAST_OTIMES_RIGHT_BLOCK);
        let expected_out_len =
            otimes_kappa(black_box(&left), black_box(&right), black_box(MATERIAL_KAPPA))
                .len_blocks();
        group.throughput(Throughput::Elements(expected_out_len as u64));

        group.bench_with_input(
            BenchmarkId::new("otimes_kappa", format!("material/v_len={v_len}")),
            &right,
            |b, right| {
                b.iter(|| {
                    // Very large kappa keeps this split-free; the dominant cost should be
                    // output materialization and memory traffic.
                    let out = otimes_kappa(
                        black_box(&left),
                        black_box(right),
                        black_box(MATERIAL_KAPPA),
                    );
                    black_box(out);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("blocks_fill", format!("uniform/material/v_len={v_len}")),
            &expected_out_len,
            |b, &out_len| {
                // Measures the Vec alloc+fill cost (kept separate from Trace::new packing).
                b.iter(|| {
                    let blocks = make_uniform_blocks(out_len, FAST_BLOCK);
                    black_box(blocks);
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("trace_new", format!("pack/material/v_len={v_len}")),
            &expected_out_len,
            |b, &out_len| {
                // Measures only the TraceStorage packing path (build_packed_chunks) on the
                // same output length as otimes_kappa materialization, excluding Vec fill.
                b.iter_batched(
                    || make_uniform_blocks(out_len, FAST_BLOCK),
                    |blocks| black_box(Trace::new(blocks)),
                    BatchSize::SmallInput,
                );
            },
        );

        group.bench_with_input(
            BenchmarkId::new("trace_new", format!("pack-almost-uniform/material/v_len={v_len}")),
            &expected_out_len,
            |b, &out_len| {
                // Worst-case for the uniform fast-path probe: mid/last match the first block,
                // but an early element differs, forcing the uniform scan to run and then fail.
                b.iter_batched(
                    || {
                        let mut blocks = make_uniform_blocks(out_len, FAST_BLOCK);
                        if out_len > 1 {
                            blocks[1] = Block::new(2, 1, 1);
                        }
                        blocks
                    },
                    |blocks| black_box(Trace::new(blocks)),
                    BatchSize::LargeInput,
                );
            },
        );

        group.bench_with_input(
            BenchmarkId::new("trace_new", format!("pack-pattern/material/v_len={v_len}")),
            &expected_out_len,
            |b, &out_len| {
                // Same as trace_new/pack, but uses a non-uniform deterministic pattern.
                // This avoids the long uniform/zero fast-paths and isolates the non-zero
                // long packing cost on mixed data.
                b.iter_batched(
                    || make_blocks(out_len),
                    |blocks| black_box(Trace::new(blocks)),
                    BatchSize::LargeInput,
                );
            },
        );

        group.bench_with_input(
            BenchmarkId::new("trace_new", format!("pack-zero/material/v_len={v_len}")),
            &expected_out_len,
            |b, &out_len| {
                // Measures the TraceStorage packing path on an all-zero input of the same
                // length, exercising the dedicated zero-fast-path in build_packed_chunks.
                b.iter_batched(
                    || make_zero_blocks(out_len),
                    |blocks| black_box(Trace::new(blocks)),
                    BatchSize::LargeInput,
                );
            },
        );
    }

    group.finish();
}

fn bench_divisibility(c: &mut Criterion) {
    let mut group = c.benchmark_group("divisibility");
    let trace_left = Trace::new(vec![Block::new(1, 0, 0), Block::zero()]);
    let trace_right = Trace::new(vec![Block::new(1, 0, 0), Block::new(0, 1, 0)]);
    let (solver_a, solver_b) = make_solver_pair();

    group.bench_function("left_divide_trace", |b| {
        b.iter(|| {
            let out = left_divide_trace(black_box(&trace_left), black_box(&trace_right));
            black_box(out);
        });
    });

    group.bench_function("left_divide_ts4_unique", |b| {
        let a = TS4::from_trace(Trace::new(vec![Block::new(1, 0, 0)]), 2);
        let b_ts4 = TS4::from_trace(
            Trace::new(vec![Block::new(1, 0, 0), Block::new(0, 1, 0)]),
            4,
        );
        b.iter(|| {
            let out = left_divide_ts4_unique(black_box(&a), black_box(&b_ts4));
            black_box(out);
        });
    });

    group.bench_function("left_divide_ts4_solve", |b| {
        b.iter(|| {
            let out = left_divide_ts4_solve(black_box(&solver_a), black_box(&solver_b), 8, 1);
            black_box(out);
        });
    });

    group.bench_function("left_divide_ts4", |b| {
        b.iter(|| {
            let out = left_divide_ts4(
                black_box(&solver_a),
                black_box(&solver_b),
                black_box(SolveMode::Unbounded { max_solutions: 1 }),
            );
            black_box(out);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_block_kernels,
    bench_trace_kernels,
    bench_otimes_materialization,
    bench_divisibility
);
criterion_main!(benches);
