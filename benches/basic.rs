#![feature(test)]
extern crate test;

use std::hint::black_box;
use test::Bencher;
use ts4::{
    Block, TS4, Trace, blocks_l1_gt, blocks_l1_gt_mask, left_divide_trace, odot_kappa,
    otimes_kappa, parallel_kappa, phi_kappa, right_divide_trace, sum_l1_blocks,
};

// 7950X policy: keep the workloads fixed and compare affinity-pinned runs
// explicitly. Unpinned measurements mix both CCD/L3 domains and are still
// executable, but they should not be interpreted as a same-CCD baseline.
// The paired fast-path / split-heavy cases are intended to be compared within
// the same affinity class, not across unrelated wave artifacts.
const BASIC_BLOCKS: usize = 1024;
const BASIC_TRACE_LAYERS: usize = 256;
const SPLIT_TRACE_LAYERS: usize = 256;
const BASIC_KAPPA: u32 = 11;
const FAST_BLOCK: Block = Block::new(1, 1, 1);
const FAST_PARALLEL_RIGHT_BLOCK: Block = Block::new(1, 1, 1);
const FAST_ODOT_RIGHT_BLOCK: Block = Block::new(1, 0, 1);
const FAST_OTIMES_RIGHT_BLOCK: Block = Block::new(1, 0, 1);
const SPLIT_BLOCK: Block = Block::new(4, 4, 4);
const OTIMES_LEFT_LAYERS: usize = 128;
const OTIMES_RIGHT_LAYERS: usize = 9;
const OTIMES_MATERIAL_LEFT_LAYERS: usize = 256;
const OTIMES_MATERIAL_RIGHT_LAYERS_SMALL: usize = 3; // middle = 1
const OTIMES_MATERIAL_RIGHT_LAYERS_LARGE: usize = 33; // middle = 31
const MATERIAL_KAPPA: u32 = 1_000_000;

fn make_blocks(len: usize) -> Vec<Block> {
    (0..len)
        .map(|i| Block::new((i % 7) as u32, (i % 5) as u32, (i % 3) as u32))
        .collect()
}

fn make_zeroed_blocks(len: usize) -> Vec<Block> {
    let mut blocks = Vec::<Block>::with_capacity(len);
    unsafe {
        blocks.set_len(len);
        // Block is repr(C) with u32 fields + padding; zero bytes produce valid zero blocks.
        std::ptr::write_bytes(blocks.as_mut_ptr(), 0, len);
    }
    blocks
}

fn clone_blocks(src: &[Block]) -> Vec<Block> {
    let mut out = Vec::with_capacity(src.len());
    unsafe {
        out.set_len(src.len());
        std::ptr::copy_nonoverlapping(src.as_ptr(), out.as_mut_ptr(), src.len());
    }
    out
}

fn make_almost_uniform_blocks(len: usize, block: Block) -> Vec<Block> {
    let mut out = vec![block; len];
    if len > 1 {
        // Avoid touching mid/last so the uniform probe path has to scan.
        out[1] = Block::new(block.x().wrapping_add(1), block.y(), block.z());
    }
    out
}

fn make_uniform_trace(len: usize, block: Block) -> Trace {
    Trace::new(vec![block; len])
}

fn make_trace(len: usize) -> Trace {
    Trace::new(
        (0..len)
            .map(|i| Block::new((i % 9) as u32, ((i * 3) % 7) as u32, ((i * 5) % 11) as u32))
            .collect(),
    )
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

#[bench]
fn bench_sum_l1(b: &mut Bencher) {
    let blocks = make_blocks(BASIC_BLOCKS);
    b.iter(|| {
        let result = sum_l1_blocks(black_box(blocks.as_slice()));
        black_box(result);
    });
}

#[bench]
fn bench_blocks_l1_gt(b: &mut Bencher) {
    let blocks = make_blocks(BASIC_BLOCKS);
    b.iter(|| {
        let mask = blocks_l1_gt(black_box(blocks.as_slice()), black_box(7));
        black_box(mask);
    });
}

#[bench]
fn bench_blocks_l1_gt_mask(b: &mut Bencher) {
    let blocks = make_blocks(BASIC_BLOCKS);
    b.iter(|| {
        let mask = blocks_l1_gt_mask(black_box(blocks.as_slice()), black_box(7));
        black_box(mask);
    });
}

#[bench]
fn bench_blocks_l1_gt_split_heavy(b: &mut Bencher) {
    let blocks = vec![SPLIT_BLOCK; BASIC_BLOCKS];
    b.iter(|| {
        let mask = blocks_l1_gt(black_box(blocks.as_slice()), black_box(BASIC_KAPPA));
        black_box(mask);
    });
}

#[bench]
fn bench_phi_kappa(b: &mut Bencher) {
    let trace = make_uniform_trace(BASIC_TRACE_LAYERS, FAST_BLOCK);
    b.iter(|| {
        let normalized = phi_kappa(black_box(&trace), black_box(BASIC_KAPPA));
        black_box(normalized);
    });
}

#[bench]
fn bench_phi_kappa_split_heavy(b: &mut Bencher) {
    let trace = make_uniform_trace(SPLIT_TRACE_LAYERS, SPLIT_BLOCK);
    b.iter(|| {
        let normalized = phi_kappa(black_box(&trace), black_box(BASIC_KAPPA));
        black_box(normalized);
    });
}

#[bench]
fn bench_parallel_kappa(b: &mut Bencher) {
    let left = make_uniform_trace(BASIC_TRACE_LAYERS, FAST_BLOCK);
    let right = make_uniform_trace(BASIC_TRACE_LAYERS + 8, FAST_PARALLEL_RIGHT_BLOCK);
    b.iter(|| {
        let out = parallel_kappa(black_box(&left), black_box(&right), black_box(BASIC_KAPPA));
        black_box(out);
    });
}

#[bench]
fn bench_parallel_kappa_split_heavy(b: &mut Bencher) {
    let left = make_uniform_trace(BASIC_TRACE_LAYERS, SPLIT_BLOCK);
    let right = make_uniform_trace(BASIC_TRACE_LAYERS + 8, SPLIT_BLOCK);
    b.iter(|| {
        let out = parallel_kappa(black_box(&left), black_box(&right), black_box(BASIC_KAPPA));
        black_box(out);
    });
}

#[bench]
fn bench_odot_kappa(b: &mut Bencher) {
    let left = make_uniform_trace(BASIC_TRACE_LAYERS / 2, FAST_BLOCK);
    let right = make_uniform_trace(BASIC_TRACE_LAYERS / 2 + 1, FAST_ODOT_RIGHT_BLOCK);
    b.iter(|| {
        let out = odot_kappa(black_box(&left), black_box(&right), black_box(BASIC_KAPPA));
        black_box(out);
    });
}

#[bench]
fn bench_odot_kappa_split_heavy(b: &mut Bencher) {
    let left = make_uniform_trace(BASIC_TRACE_LAYERS / 2, SPLIT_BLOCK);
    let right = make_uniform_trace(BASIC_TRACE_LAYERS / 2 + 1, SPLIT_BLOCK);
    b.iter(|| {
        let out = odot_kappa(black_box(&left), black_box(&right), black_box(BASIC_KAPPA));
        black_box(out);
    });
}

#[bench]
fn bench_otimes_kappa(b: &mut Bencher) {
    let left = make_uniform_trace(OTIMES_LEFT_LAYERS, FAST_BLOCK);
    let right = make_uniform_trace(OTIMES_RIGHT_LAYERS, FAST_OTIMES_RIGHT_BLOCK);
    b.iter(|| {
        let out = otimes_kappa(black_box(&left), black_box(&right), black_box(BASIC_KAPPA));
        black_box(out);
    });
}

#[bench]
fn bench_otimes_kappa_split_heavy(b: &mut Bencher) {
    let left = make_uniform_trace(OTIMES_LEFT_LAYERS, SPLIT_BLOCK);
    let right = make_uniform_trace(OTIMES_RIGHT_LAYERS, SPLIT_BLOCK);
    b.iter(|| {
        let out = otimes_kappa(black_box(&left), black_box(&right), black_box(BASIC_KAPPA));
        black_box(out);
    });
}

#[bench]
fn bench_otimes_kappa_materialize_small_middle(b: &mut Bencher) {
    // Split-free path that primarily measures output materialization scaling.
    let left = make_uniform_trace(OTIMES_MATERIAL_LEFT_LAYERS, FAST_BLOCK);
    let right = make_uniform_trace(
        OTIMES_MATERIAL_RIGHT_LAYERS_SMALL,
        FAST_OTIMES_RIGHT_BLOCK,
    );
    b.iter(|| {
        let out = otimes_kappa(black_box(&left), black_box(&right), black_box(MATERIAL_KAPPA));
        black_box(out);
    });
}

#[bench]
fn bench_otimes_kappa_materialize_large_middle(b: &mut Bencher) {
    // Same as above, but forces a much larger repeated middle segment (`v_len - 2`).
    let left = make_uniform_trace(OTIMES_MATERIAL_LEFT_LAYERS, FAST_BLOCK);
    let right = make_uniform_trace(
        OTIMES_MATERIAL_RIGHT_LAYERS_LARGE,
        FAST_OTIMES_RIGHT_BLOCK,
    );
    b.iter(|| {
        let out = otimes_kappa(black_box(&left), black_box(&right), black_box(MATERIAL_KAPPA));
        black_box(out);
    });
}

#[bench]
fn bench_trace_new_pack_materialize_len_small_middle(b: &mut Bencher) {
    let left = make_uniform_trace(OTIMES_MATERIAL_LEFT_LAYERS, FAST_BLOCK);
    let right = make_uniform_trace(
        OTIMES_MATERIAL_RIGHT_LAYERS_SMALL,
        FAST_OTIMES_RIGHT_BLOCK,
    );
    let out_len = otimes_kappa(&left, &right, MATERIAL_KAPPA).len_blocks();
    b.iter(|| {
        let blocks = make_zeroed_blocks(out_len);
        let trace = Trace::new(blocks);
        black_box(trace);
    });
}

#[bench]
fn bench_trace_new_pack_pattern_materialize_len_small_middle(b: &mut Bencher) {
    let left = make_uniform_trace(OTIMES_MATERIAL_LEFT_LAYERS, FAST_BLOCK);
    let right = make_uniform_trace(
        OTIMES_MATERIAL_RIGHT_LAYERS_SMALL,
        FAST_OTIMES_RIGHT_BLOCK,
    );
    let out_len = otimes_kappa(&left, &right, MATERIAL_KAPPA).len_blocks();
    let pattern = make_blocks(out_len);
    b.iter(|| {
        let blocks = clone_blocks(black_box(pattern.as_slice()));
        let trace = Trace::new(blocks);
        black_box(trace);
    });
}

#[bench]
fn bench_clone_pattern_blocks_materialize_len_small_middle(b: &mut Bencher) {
    let left = make_uniform_trace(OTIMES_MATERIAL_LEFT_LAYERS, FAST_BLOCK);
    let right = make_uniform_trace(
        OTIMES_MATERIAL_RIGHT_LAYERS_SMALL,
        FAST_OTIMES_RIGHT_BLOCK,
    );
    let out_len = otimes_kappa(&left, &right, MATERIAL_KAPPA).len_blocks();
    let pattern = make_blocks(out_len);
    b.iter(|| {
        let blocks = clone_blocks(black_box(pattern.as_slice()));
        black_box(blocks);
    });
}

#[bench]
fn bench_make_zeroed_blocks_materialize_len_small_middle(b: &mut Bencher) {
    let left = make_uniform_trace(OTIMES_MATERIAL_LEFT_LAYERS, FAST_BLOCK);
    let right = make_uniform_trace(
        OTIMES_MATERIAL_RIGHT_LAYERS_SMALL,
        FAST_OTIMES_RIGHT_BLOCK,
    );
    let out_len = otimes_kappa(&left, &right, MATERIAL_KAPPA).len_blocks();
    b.iter(|| {
        let blocks = make_zeroed_blocks(out_len);
        black_box(blocks);
    });
}

#[bench]
fn bench_trace_new_pack_materialize_len_large_middle(b: &mut Bencher) {
    let left = make_uniform_trace(OTIMES_MATERIAL_LEFT_LAYERS, FAST_BLOCK);
    let right = make_uniform_trace(
        OTIMES_MATERIAL_RIGHT_LAYERS_LARGE,
        FAST_OTIMES_RIGHT_BLOCK,
    );
    let out_len = otimes_kappa(&left, &right, MATERIAL_KAPPA).len_blocks();
    b.iter(|| {
        let blocks = make_zeroed_blocks(out_len);
        let trace = Trace::new(blocks);
        black_box(trace);
    });
}

#[bench]
fn bench_trace_new_pack_pattern_materialize_len_large_middle(b: &mut Bencher) {
    let left = make_uniform_trace(OTIMES_MATERIAL_LEFT_LAYERS, FAST_BLOCK);
    let right = make_uniform_trace(
        OTIMES_MATERIAL_RIGHT_LAYERS_LARGE,
        FAST_OTIMES_RIGHT_BLOCK,
    );
    let out_len = otimes_kappa(&left, &right, MATERIAL_KAPPA).len_blocks();
    let pattern = make_blocks(out_len);
    b.iter(|| {
        let blocks = clone_blocks(black_box(pattern.as_slice()));
        let trace = Trace::new(blocks);
        black_box(trace);
    });
}

#[bench]
fn bench_clone_pattern_blocks_materialize_len_large_middle(b: &mut Bencher) {
    let left = make_uniform_trace(OTIMES_MATERIAL_LEFT_LAYERS, FAST_BLOCK);
    let right = make_uniform_trace(
        OTIMES_MATERIAL_RIGHT_LAYERS_LARGE,
        FAST_OTIMES_RIGHT_BLOCK,
    );
    let out_len = otimes_kappa(&left, &right, MATERIAL_KAPPA).len_blocks();
    let pattern = make_blocks(out_len);
    b.iter(|| {
        let blocks = clone_blocks(black_box(pattern.as_slice()));
        black_box(blocks);
    });
}

#[bench]
fn bench_make_zeroed_blocks_materialize_len_large_middle(b: &mut Bencher) {
    let left = make_uniform_trace(OTIMES_MATERIAL_LEFT_LAYERS, FAST_BLOCK);
    let right = make_uniform_trace(
        OTIMES_MATERIAL_RIGHT_LAYERS_LARGE,
        FAST_OTIMES_RIGHT_BLOCK,
    );
    let out_len = otimes_kappa(&left, &right, MATERIAL_KAPPA).len_blocks();
    b.iter(|| {
        let blocks = make_zeroed_blocks(out_len);
        black_box(blocks);
    });
}

#[bench]
fn bench_trace_new_pack_uniform_nonzero_materialize_len_large_middle(b: &mut Bencher) {
    let left = make_uniform_trace(OTIMES_MATERIAL_LEFT_LAYERS, FAST_BLOCK);
    let right = make_uniform_trace(
        OTIMES_MATERIAL_RIGHT_LAYERS_LARGE,
        FAST_OTIMES_RIGHT_BLOCK,
    );
    let out_len = otimes_kappa(&left, &right, MATERIAL_KAPPA).len_blocks();
    let uniform = vec![FAST_BLOCK; out_len];
    b.iter(|| {
        let blocks = clone_blocks(black_box(uniform.as_slice()));
        let trace = Trace::new(blocks);
        black_box(trace);
    });
}

#[bench]
fn bench_clone_uniform_nonzero_blocks_materialize_len_large_middle(b: &mut Bencher) {
    let left = make_uniform_trace(OTIMES_MATERIAL_LEFT_LAYERS, FAST_BLOCK);
    let right = make_uniform_trace(
        OTIMES_MATERIAL_RIGHT_LAYERS_LARGE,
        FAST_OTIMES_RIGHT_BLOCK,
    );
    let out_len = otimes_kappa(&left, &right, MATERIAL_KAPPA).len_blocks();
    let uniform = vec![FAST_BLOCK; out_len];
    b.iter(|| {
        let blocks = clone_blocks(black_box(uniform.as_slice()));
        black_box(blocks);
    });
}

#[bench]
fn bench_trace_new_pack_almost_uniform_materialize_len_large_middle(b: &mut Bencher) {
    let left = make_uniform_trace(OTIMES_MATERIAL_LEFT_LAYERS, FAST_BLOCK);
    let right = make_uniform_trace(
        OTIMES_MATERIAL_RIGHT_LAYERS_LARGE,
        FAST_OTIMES_RIGHT_BLOCK,
    );
    let out_len = otimes_kappa(&left, &right, MATERIAL_KAPPA).len_blocks();
    let almost = make_almost_uniform_blocks(out_len, FAST_BLOCK);
    b.iter(|| {
        let blocks = clone_blocks(black_box(almost.as_slice()));
        let trace = Trace::new(blocks);
        black_box(trace);
    });
}

#[bench]
fn bench_clone_almost_uniform_blocks_materialize_len_large_middle(b: &mut Bencher) {
    let left = make_uniform_trace(OTIMES_MATERIAL_LEFT_LAYERS, FAST_BLOCK);
    let right = make_uniform_trace(
        OTIMES_MATERIAL_RIGHT_LAYERS_LARGE,
        FAST_OTIMES_RIGHT_BLOCK,
    );
    let out_len = otimes_kappa(&left, &right, MATERIAL_KAPPA).len_blocks();
    let almost = make_almost_uniform_blocks(out_len, FAST_BLOCK);
    b.iter(|| {
        let blocks = clone_blocks(black_box(almost.as_slice()));
        black_box(blocks);
    });
}

#[bench]
fn bench_trace_compose(b: &mut Bencher) {
    let left = make_trace(BASIC_TRACE_LAYERS / 2);
    let right = make_trace(BASIC_TRACE_LAYERS / 2 + 1);
    b.iter(|| {
        let out = black_box(&left).compose(black_box(&right));
        black_box(out);
    });
}

#[bench]
fn bench_left_divide_trace(b: &mut Bencher) {
    let left = Trace::new(vec![Block::new(1, 0, 0), Block::zero()]);
    let right = Trace::new(vec![Block::new(1, 0, 0), Block::new(0, 1, 0)]);
    b.iter(|| {
        let out = left_divide_trace(black_box(&left), black_box(&right));
        black_box(out);
    });
}

#[bench]
fn bench_right_divide_trace(b: &mut Bencher) {
    let left = Trace::new(vec![Block::new(0, 1, 0)]);
    let right = Trace::new(vec![Block::new(1, 0, 0), Block::new(0, 1, 0)]);
    b.iter(|| {
        let out = right_divide_trace(black_box(&left), black_box(&right));
        black_box(out);
    });
}

#[bench]
fn bench_ts4_compose(b: &mut Bencher) {
    let (left, right) = make_ts4_pair();
    b.iter(|| {
        let out = black_box(&left).compose(black_box(&right));
        black_box(out);
    });
}
