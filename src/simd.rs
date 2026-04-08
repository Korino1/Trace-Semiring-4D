//! Zen 4 SIMD utilities for the fixed 7950X execution policy.
//!
//! Hot integer kernels keep one fixed Zen 4 path with a 256-bit baseline and
//! AVX-512VL mask semantics. Kernels operate in AoSoA8/YMM chunks and avoid
//! full-trace repack builders in steady-state hot loops.
//!
//! Быстрое использование:
//! ```
//! use ts4::{Block, sum_l1_blocks};
//! let blocks = vec![Block::new(1,2,3), Block::new(4,0,0)];
//! let s = sum_l1_blocks(&blocks);
//! ```

#[cfg(not(target_arch = "x86_64"))]
compile_error!("ts4 targets AMD Zen 4 x86_64 only");

use crate::types::Block;
use std::arch::x86_64::*;

pub(crate) const LANES_256: usize = 8;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(C, align(32))]
pub(crate) struct Chunk8 {
    pub(crate) x: [u32; LANES_256],
    pub(crate) y: [u32; LANES_256],
    pub(crate) z: [u32; LANES_256],
}

impl Chunk8 {
    pub(crate) const ZERO: Self = Self {
        x: [0; LANES_256],
        y: [0; LANES_256],
        z: [0; LANES_256],
    };
}

/// First-class packed mask for block predicates.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BlockMask {
    bits: Vec<u64>,
    len: usize,
}

impl BlockMask {
    #[inline]
    pub fn new(len: usize) -> Self {
        let words = len.div_ceil(64);
        Self {
            bits: vec![0u64; words],
            len,
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub fn count_ones(&self) -> usize {
        self.bits
            .iter()
            .map(|word| word.count_ones() as usize)
            .sum()
    }

    #[inline]
    pub fn get(&self, index: usize) -> bool {
        assert!(index < self.len, "BlockMask::get index out of bounds");
        let word = self.bits[index / 64];
        ((word >> (index % 64)) & 1) != 0
    }

    #[inline]
    pub fn as_words(&self) -> &[u64] {
        &self.bits
    }

    #[inline]
    pub fn into_words(self) -> Vec<u64> {
        self.bits
    }

    #[inline]
    pub fn into_bools(self) -> Vec<bool> {
        let mut out = Vec::with_capacity(self.len);
        for index in 0..self.len {
            let word = self.bits[index / 64];
            out.push(((word >> (index % 64)) & 1) != 0);
        }
        out
    }

    #[inline(always)]
    pub(crate) fn write_chunk_mask(&mut self, chunk_index: usize, mask: u8) {
        if mask == 0 {
            return;
        }
        let word_index = chunk_index / 8;
        let bit_shift = (chunk_index % 8) * 8;
        self.bits[word_index] |= (mask as u64) << bit_shift;
    }
}

#[inline(always)]
const fn valid_lane_mask(valid_lanes: usize) -> u8 {
    if valid_lanes >= LANES_256 {
        u8::MAX
    } else if valid_lanes == 0 {
        0
    } else {
        ((1u16 << valid_lanes) - 1) as u8
    }
}

#[inline(always)]
pub(crate) fn push_chunk_blocks(out: &mut Vec<Block>, chunk: &Chunk8, valid_lanes: usize) {
    push_chunk_block_range(out, chunk, 0, valid_lanes);
}

#[inline(always)]
pub(crate) fn push_chunk_block_range(
    out: &mut Vec<Block>,
    chunk: &Chunk8,
    start_lane: usize,
    end_lane: usize,
) {
    let start = start_lane.min(LANES_256);
    let end = end_lane.min(LANES_256);
    if start >= end {
        return;
    }

    let count = end - start;
    let base_len = out.len();
    out.reserve(count);
    let dst = unsafe { out.as_mut_ptr().add(base_len) };

    for offset in 0..count {
        let lane = start + offset;
        unsafe {
            dst.add(offset)
                .write(Block::new(chunk.x[lane], chunk.y[lane], chunk.z[lane]));
        }
    }
    unsafe { out.set_len(base_len + count) };
}

#[inline(always)]
unsafe fn load_packed_chunk_vectors(chunk: &Chunk8) -> (__m256i, __m256i, __m256i) {
    let x = unsafe { _mm256_load_si256(chunk.x.as_ptr() as *const __m256i) };
    let y = unsafe { _mm256_load_si256(chunk.y.as_ptr() as *const __m256i) };
    let z = unsafe { _mm256_load_si256(chunk.z.as_ptr() as *const __m256i) };
    (x, y, z)
}

#[inline(always)]
unsafe fn zero_invalid_lanes_u32(values: __m256i, valid_lanes: usize) -> __m256i {
    unsafe { _mm256_maskz_mov_epi32(valid_lane_mask(valid_lanes), values) }
}

#[inline(always)]
unsafe fn add_u32_with_overflow_mask(lhs: __m256i, rhs: __m256i) -> (__m256i, u8) {
    let sum = unsafe { _mm256_add_epi32(lhs, rhs) };
    let overflow = unsafe { _mm256_cmpgt_epu32_mask(lhs, sum) } as u8;
    (sum, overflow)
}

#[inline(always)]
unsafe fn block_l1_sum_and_mask_from_vectors(
    x: __m256i,
    y: __m256i,
    z: __m256i,
    kappa: u32,
) -> (__m256i, u8) {
    let (xy, overflow_xy) = unsafe { add_u32_with_overflow_mask(x, y) };
    let (sum, overflow_sum) = unsafe { add_u32_with_overflow_mask(xy, z) };
    let overflow = overflow_xy | overflow_sum;
    if overflow != 0 {
        panic!("Block::l1 overflow");
    }
    let limit = unsafe { _mm256_set1_epi32(kappa as i32) };
    let mask = unsafe { _mm256_cmpgt_epu32_mask(sum, limit) } as u8;
    (sum, mask)
}

#[inline(always)]
unsafe fn sum_u32_lanes(sum: __m256i) -> u64 {
    let lo128 = unsafe { _mm256_castsi256_si128(sum) };
    let hi128 = unsafe { _mm256_extracti128_si256(sum, 1) };
    let lo64 = unsafe { _mm256_cvtepu32_epi64(lo128) };
    let hi64 = unsafe { _mm256_cvtepu32_epi64(hi128) };
    let mut lanes = [0u64; 4];
    unsafe { _mm256_storeu_si256(lanes.as_mut_ptr() as *mut __m256i, lo64) };
    let mut acc = lanes.iter().copied().sum::<u64>();
    unsafe { _mm256_storeu_si256(lanes.as_mut_ptr() as *mut __m256i, hi64) };
    acc += lanes.iter().copied().sum::<u64>();
    acc
}

#[inline(always)]
unsafe fn store_chunk(sum_x: __m256i, sum_y: __m256i, sum_z: __m256i) -> Chunk8 {
    let mut chunk = Chunk8::ZERO;
    unsafe { _mm256_store_si256(chunk.x.as_mut_ptr() as *mut __m256i, sum_x) };
    unsafe { _mm256_store_si256(chunk.y.as_mut_ptr() as *mut __m256i, sum_y) };
    unsafe { _mm256_store_si256(chunk.z.as_mut_ptr() as *mut __m256i, sum_z) };
    chunk
}

#[inline]
fn pack_block_slice_to_chunks(blocks: &[Block]) -> Box<[Chunk8]> {
    let chunk_count = blocks.len().div_ceil(LANES_256);
    if chunk_count == 0 {
        return Box::new([]);
    }

    let mut packed = vec![Chunk8::ZERO; chunk_count];
    for (chunk_index, chunk_blocks) in blocks.chunks(LANES_256).enumerate() {
        let chunk = &mut packed[chunk_index];
        for (lane, block) in chunk_blocks.iter().enumerate() {
            chunk.x[lane] = block.x;
            chunk.y[lane] = block.y;
            chunk.z[lane] = block.z;
        }
    }
    packed.into_boxed_slice()
}

#[inline]
pub(crate) fn packed_chunk_l1_gt_mask(chunk: &Chunk8, valid_lanes: usize, kappa: u32) -> u8 {
    assert!(
        valid_lanes <= LANES_256,
        "packed_chunk_l1_gt_mask valid_lanes must be <= LANES_256"
    );
    if valid_lanes == 0 {
        return 0;
    }
    unsafe { packed_chunk_l1_gt_mask_zen4(chunk, valid_lanes, kappa) }
}

#[target_feature(enable = "avx2,avx512f,avx512vl")]
unsafe fn packed_chunk_l1_gt_mask_zen4(chunk: &Chunk8, valid_lanes: usize, kappa: u32) -> u8 {
    let (x, y, z) = unsafe { load_packed_chunk_vectors(chunk) };
    let x = unsafe { zero_invalid_lanes_u32(x, valid_lanes) };
    let y = unsafe { zero_invalid_lanes_u32(y, valid_lanes) };
    let z = unsafe { zero_invalid_lanes_u32(z, valid_lanes) };
    let (_, mask) = unsafe { block_l1_sum_and_mask_from_vectors(x, y, z, kappa) };
    mask
}

#[inline]
pub(crate) fn add_packed_chunks_and_l1_mask(
    left: &Chunk8,
    right: &Chunk8,
    valid_lanes: usize,
    kappa: u32,
) -> (Chunk8, u8) {
    assert!(
        valid_lanes <= LANES_256,
        "add_packed_chunks_and_l1_mask valid_lanes must be <= LANES_256"
    );
    if valid_lanes == 0 {
        return (Chunk8::ZERO, 0);
    }
    unsafe { add_packed_chunks_and_l1_mask_zen4(left, right, valid_lanes, kappa) }
}

#[target_feature(enable = "avx2,avx512f,avx512vl")]
unsafe fn add_packed_chunks_and_l1_mask_zen4(
    left: &Chunk8,
    right: &Chunk8,
    valid_lanes: usize,
    kappa: u32,
) -> (Chunk8, u8) {
    let (left_x, left_y, left_z) = unsafe { load_packed_chunk_vectors(left) };
    let (right_x, right_y, right_z) = unsafe { load_packed_chunk_vectors(right) };

    let left_x = unsafe { zero_invalid_lanes_u32(left_x, valid_lanes) };
    let left_y = unsafe { zero_invalid_lanes_u32(left_y, valid_lanes) };
    let left_z = unsafe { zero_invalid_lanes_u32(left_z, valid_lanes) };
    let right_x = unsafe { zero_invalid_lanes_u32(right_x, valid_lanes) };
    let right_y = unsafe { zero_invalid_lanes_u32(right_y, valid_lanes) };
    let right_z = unsafe { zero_invalid_lanes_u32(right_z, valid_lanes) };

    let (sum_x, overflow_x) = unsafe { add_u32_with_overflow_mask(left_x, right_x) };
    let (sum_y, overflow_y) = unsafe { add_u32_with_overflow_mask(left_y, right_y) };
    let (sum_z, overflow_z) = unsafe { add_u32_with_overflow_mask(left_z, right_z) };
    let component_overflow = overflow_x | overflow_y | overflow_z;
    if component_overflow != 0 {
        panic!("Block::add overflow");
    }

    let (_, mask) = unsafe { block_l1_sum_and_mask_from_vectors(sum_x, sum_y, sum_z, kappa) };
    let chunk = unsafe { store_chunk(sum_x, sum_y, sum_z) };
    (chunk, mask)
}

#[inline]
pub(crate) fn add_block_to_packed_chunk_and_l1_mask(
    chunk: &Chunk8,
    addend: Block,
    valid_lanes: usize,
    kappa: u32,
) -> (Chunk8, u8) {
    assert!(
        valid_lanes <= LANES_256,
        "add_block_to_packed_chunk_and_l1_mask valid_lanes must be <= LANES_256"
    );
    if valid_lanes == 0 {
        return (Chunk8::ZERO, 0);
    }
    unsafe { add_block_to_packed_chunk_and_l1_mask_zen4(chunk, addend, valid_lanes, kappa) }
}

#[target_feature(enable = "avx2,avx512f,avx512vl")]
unsafe fn add_block_to_packed_chunk_and_l1_mask_zen4(
    chunk: &Chunk8,
    addend: Block,
    valid_lanes: usize,
    kappa: u32,
) -> (Chunk8, u8) {
    let (x, y, z) = unsafe { load_packed_chunk_vectors(chunk) };
    let x = unsafe { zero_invalid_lanes_u32(x, valid_lanes) };
    let y = unsafe { zero_invalid_lanes_u32(y, valid_lanes) };
    let z = unsafe { zero_invalid_lanes_u32(z, valid_lanes) };

    let add_x = _mm256_set1_epi32(addend.x as i32);
    let add_y = _mm256_set1_epi32(addend.y as i32);
    let add_z = _mm256_set1_epi32(addend.z as i32);

    let (sum_x, overflow_x) = unsafe { add_u32_with_overflow_mask(x, add_x) };
    let (sum_y, overflow_y) = unsafe { add_u32_with_overflow_mask(y, add_y) };
    let (sum_z, overflow_z) = unsafe { add_u32_with_overflow_mask(z, add_z) };
    let component_overflow = overflow_x | overflow_y | overflow_z;
    if component_overflow != 0 {
        panic!("Block::add overflow");
    }

    let (_, mask) = unsafe { block_l1_sum_and_mask_from_vectors(sum_x, sum_y, sum_z, kappa) };
    let summed = unsafe { store_chunk(sum_x, sum_y, sum_z) };
    (summed, mask)
}

#[inline]
pub(crate) fn sum_packed_chunk_l1(chunk: &Chunk8, valid_lanes: usize) -> u64 {
    assert!(
        valid_lanes <= LANES_256,
        "sum_packed_chunk_l1 valid_lanes must be <= LANES_256"
    );
    if valid_lanes == 0 {
        return 0;
    }
    unsafe { sum_packed_chunk_l1_zen4(chunk, valid_lanes) }
}

#[target_feature(enable = "avx2,avx512f,avx512vl")]
unsafe fn sum_packed_chunk_l1_zen4(chunk: &Chunk8, valid_lanes: usize) -> u64 {
    let (x, y, z) = unsafe { load_packed_chunk_vectors(chunk) };
    let x = unsafe { zero_invalid_lanes_u32(x, valid_lanes) };
    let y = unsafe { zero_invalid_lanes_u32(y, valid_lanes) };
    let z = unsafe { zero_invalid_lanes_u32(z, valid_lanes) };
    let (xy, overflow_xy) = unsafe { add_u32_with_overflow_mask(x, y) };
    let (sum, overflow_sum) = unsafe { add_u32_with_overflow_mask(xy, z) };
    if (overflow_xy | overflow_sum) != 0 {
        panic!("Block::l1 overflow");
    }
    unsafe { sum_u32_lanes(sum) }
}

#[inline]
pub(crate) fn sum_packed_chunk_xyz(chunk: &Chunk8, valid_lanes: usize) -> (u64, u64, u64) {
    assert!(
        valid_lanes <= LANES_256,
        "sum_packed_chunk_xyz valid_lanes must be <= LANES_256"
    );
    if valid_lanes == 0 {
        return (0, 0, 0);
    }
    unsafe { sum_packed_chunk_xyz_zen4(chunk, valid_lanes) }
}

#[target_feature(enable = "avx2,avx512f,avx512vl")]
unsafe fn sum_packed_chunk_xyz_zen4(chunk: &Chunk8, valid_lanes: usize) -> (u64, u64, u64) {
    let (x, y, z) = unsafe { load_packed_chunk_vectors(chunk) };
    let x = unsafe { zero_invalid_lanes_u32(x, valid_lanes) };
    let y = unsafe { zero_invalid_lanes_u32(y, valid_lanes) };
    let z = unsafe { zero_invalid_lanes_u32(z, valid_lanes) };
    (
        unsafe { sum_u32_lanes(x) },
        unsafe { sum_u32_lanes(y) },
        unsafe { sum_u32_lanes(z) },
    )
}

/// Zen 4 fixed-path сумма L1 по блокам.
#[inline]
pub fn sum_l1_blocks(blocks: &[Block]) -> u64 {
    if blocks.is_empty() {
        return 0;
    }
    unsafe { sum_l1_blocks_zen4(blocks) }
}

/// Zen 4 fixed-path mask probe for blocks whose L1 exceeds `kappa`.
#[inline]
pub fn blocks_l1_gt_mask(blocks: &[Block], kappa: u32) -> BlockMask {
    if blocks.is_empty() {
        return BlockMask::new(0);
    }
    unsafe { blocks_l1_gt_mask_zen4(blocks, kappa) }
}

#[inline]
pub fn blocks_l1_gt(blocks: &[Block], kappa: u32) -> Vec<bool> {
    if blocks.is_empty() {
        return Vec::new();
    }
    blocks_l1_gt_mask(blocks, kappa).into_bools()
}

#[target_feature(enable = "avx2,avx512f,avx512vl")]
unsafe fn sum_l1_blocks_zen4(blocks: &[Block]) -> u64 {
    let packed = pack_block_slice_to_chunks(blocks);
    let mut acc = 0u64;
    for (chunk_index, chunk) in packed.iter().enumerate() {
        let start = chunk_index * LANES_256;
        let valid_lanes = blocks.len().saturating_sub(start).min(LANES_256);
        acc += sum_packed_chunk_l1(chunk, valid_lanes);
    }
    acc
}

#[target_feature(enable = "avx2,avx512f,avx512vl")]
unsafe fn blocks_l1_gt_mask_zen4(blocks: &[Block], kappa: u32) -> BlockMask {
    let packed = pack_block_slice_to_chunks(blocks);
    let mut out = BlockMask::new(blocks.len());
    for (chunk_index, chunk) in packed.iter().enumerate() {
        let start = chunk_index * LANES_256;
        let valid_lanes = blocks.len().saturating_sub(start).min(LANES_256);
        let mask = packed_chunk_l1_gt_mask(chunk, valid_lanes, kappa);
        out.write_chunk_mask(chunk_index, mask);
    }
    out
}
