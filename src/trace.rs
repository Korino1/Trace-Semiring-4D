//! Трассы: каноническая структура блоков между τ.
//!
//! Быстрое использование:
//! ```
//! use ts4::{Block, Trace};
//! let t = Trace::new(vec![Block::new(1,0,0), Block::zero()]); // x τ
//! let u = Trace::from_word("txty"); // t x t y
//! let v = t.compose(&u);
//! ```

use crate::simd::{
    BlockMask, Chunk8, LANES_256, packed_chunk_l1_gt_mask, push_chunk_block_range,
    push_chunk_blocks, sum_packed_chunk_l1, sum_packed_chunk_xyz,
};
use crate::types::Block;
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::sync::OnceLock;

#[derive(Debug)]
struct TraceStorage {
    packed_chunks: Box<[Chunk8]>,
    len_blocks: usize,
    blocks: OnceLock<Box<[Block]>>,
}

impl TraceStorage {
    #[inline]
    fn new(blocks: Vec<Block>) -> Self {
        let len_blocks = blocks.len();
        Self {
            packed_chunks: build_packed_chunks(&blocks),
            len_blocks,
            blocks: OnceLock::new(),
        }
    }

    #[inline]
    fn from_packed_chunks(packed_chunks: Box<[Chunk8]>, len_blocks: usize) -> Self {
        Self {
            packed_chunks,
            len_blocks,
            blocks: OnceLock::new(),
        }
    }
}

impl Clone for TraceStorage {
    #[inline]
    fn clone(&self) -> Self {
        Self::from_packed_chunks(self.packed_chunks.clone(), self.len_blocks)
    }
}

impl PartialEq for TraceStorage {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.len_blocks == other.len_blocks && self.packed_chunks == other.packed_chunks
    }
}

impl Eq for TraceStorage {}

impl Hash for TraceStorage {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.len_blocks.hash(state);
        self.packed_chunks.hash(state);
    }
}

impl PartialOrd for TraceStorage {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TraceStorage {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        match self.len_blocks.cmp(&other.len_blocks) {
            Ordering::Equal => self.packed_chunks.cmp(&other.packed_chunks),
            ordering => ordering,
        }
    }
}

#[inline]
fn build_packed_chunks(blocks: &[Block]) -> Box<[Chunk8]> {
    let chunk_count = blocks.len().div_ceil(LANES_256);
    if chunk_count == 0 {
        return Box::new([]);
    }

    // Fast path for long, uniform materializations (common in output-length probes):
    // avoid per-lane transposition when all input blocks are identical.
    if let Some(uniform) = try_uniform_block_fast(blocks) {
        return build_uniform_packed_chunks(uniform, chunk_count, blocks.len() % LANES_256);
    }

    let mut packed = Vec::with_capacity(chunk_count);
    let full_chunks = blocks.len() / LANES_256;
    let tail_lanes = blocks.len() % LANES_256;
    let src = blocks.as_ptr();
    let dst: *mut Chunk8 = packed.as_mut_ptr();

    unsafe {
        packed.set_len(chunk_count);
    }

    for chunk_index in 0..full_chunks {
        let chunk_src = unsafe { src.add(chunk_index * LANES_256) };
        let chunk_dst = unsafe { dst.add(chunk_index) };
        unsafe {
            pack_full_chunk_unchecked(chunk_dst, chunk_src);
        }
    }

    if tail_lanes != 0 {
        let tail_chunk_dst = unsafe { dst.add(full_chunks) };
        unsafe {
            tail_chunk_dst.write(Chunk8::ZERO);
        }
        let tail_src = unsafe { src.add(full_chunks * LANES_256) };
        unsafe {
            fill_tail_chunk_unchecked(tail_chunk_dst, tail_src, tail_lanes);
        }
    }

    packed.into_boxed_slice()
}

#[inline(always)]
unsafe fn pack_full_chunk_unchecked(chunk_dst: *mut Chunk8, chunk_src: *const Block) {
    let chunk = unsafe { &mut *chunk_dst };
    let blocks = unsafe { std::slice::from_raw_parts(chunk_src, LANES_256) };
    chunk.x = [
        blocks[0].x,
        blocks[1].x,
        blocks[2].x,
        blocks[3].x,
        blocks[4].x,
        blocks[5].x,
        blocks[6].x,
        blocks[7].x,
    ];
    chunk.y = [
        blocks[0].y,
        blocks[1].y,
        blocks[2].y,
        blocks[3].y,
        blocks[4].y,
        blocks[5].y,
        blocks[6].y,
        blocks[7].y,
    ];
    chunk.z = [
        blocks[0].z,
        blocks[1].z,
        blocks[2].z,
        blocks[3].z,
        blocks[4].z,
        blocks[5].z,
        blocks[6].z,
        blocks[7].z,
    ];
}

#[inline(always)]
unsafe fn fill_tail_chunk_unchecked(chunk_dst: *mut Chunk8, tail_src: *const Block, tail_lanes: usize) {
    debug_assert!(tail_lanes > 0 && tail_lanes < LANES_256);

    let chunk = unsafe { &mut *chunk_dst };
    for lane in 0..tail_lanes {
        let block = unsafe { tail_src.add(lane).read() };
        chunk.x[lane] = block.x;
        chunk.y[lane] = block.y;
        chunk.z[lane] = block.z;
    }
}

#[inline]
fn try_uniform_block_fast(blocks: &[Block]) -> Option<Block> {
    if blocks.len() < (LANES_256 * 8) {
        return None;
    }

    let len = blocks.len();
    let first = blocks[0];
    // Cheap probe set to reject obvious long non-uniform inputs before the
    // full u128 scan. This keeps the uniform fast-path cheap on pattern-heavy
    // long traces while preserving exactness via the full-word verification below.
    let probe_indices = [1usize, len / 4, len / 2, len - (len / 4), len - 1];
    for &index in &probe_indices {
        if blocks[index] != first {
            return None;
        }
    }

    // Block is 16-byte aligned and laid out as 4 u32 values, so one u128 read per block
    // provides a cheap equality probe across the whole element, including private padding.
    let words = unsafe { std::slice::from_raw_parts(blocks.as_ptr() as *const u128, len) };
    let first_word = words[0];
    if all_words_equal_branchless(words, first_word) {
        Some(first)
    } else {
        None
    }
}

#[inline]
fn all_words_equal_branchless(words: &[u128], expected: u128) -> bool {
    // Branchless reduction keeps the long uniform probe predictable and avoids
    // per-element early-exit branches in the common all-equal case.
    let mut diff = 0u128;
    let mut chunks = words.chunks_exact(4);
    for chunk in chunks.by_ref() {
        diff |= chunk[0] ^ expected;
        diff |= chunk[1] ^ expected;
        diff |= chunk[2] ^ expected;
        diff |= chunk[3] ^ expected;
    }
    for word in chunks.remainder() {
        diff |= *word ^ expected;
    }
    diff == 0
}

#[inline]
fn build_uniform_packed_chunks(block: Block, chunk_count: usize, tail_lanes: usize) -> Box<[Chunk8]> {
    if block == Block::zero() {
        return vec![Chunk8::ZERO; chunk_count].into_boxed_slice();
    }

    let uniform = Chunk8 {
        x: [block.x; LANES_256],
        y: [block.y; LANES_256],
        z: [block.z; LANES_256],
    };
    let mut packed = vec![uniform; chunk_count];

    if tail_lanes != 0 {
        let last = &mut packed[chunk_count - 1];
        last.x[tail_lanes..].fill(0);
        last.y[tail_lanes..].fill(0);
        last.z[tail_lanes..].fill(0);
    }

    packed.into_boxed_slice()
}

#[inline]
fn build_block_view_from_packed(packed_chunks: &[Chunk8], len_blocks: usize) -> Box<[Block]> {
    let mut blocks = Vec::with_capacity(len_blocks);
    for (chunk_index, chunk) in packed_chunks.iter().enumerate() {
        let start = chunk_index * LANES_256;
        let valid_lanes = len_blocks.saturating_sub(start).min(LANES_256);
        if valid_lanes == 0 {
            break;
        }
        push_chunk_blocks(&mut blocks, chunk, valid_lanes);
    }
    blocks.into_boxed_slice()
}

#[derive(Debug)]
pub(crate) struct PackedTraceBuilder {
    packed_chunks: Vec<Chunk8>,
    len_blocks: usize,
}

impl PackedTraceBuilder {
    #[inline]
    pub(crate) fn with_capacity(capacity_blocks: usize) -> Self {
        Self {
            packed_chunks: Vec::with_capacity(capacity_blocks.div_ceil(LANES_256)),
            len_blocks: 0,
        }
    }

    #[inline]
    pub(crate) fn push_block(&mut self, block: Block) {
        let lane = self.len_blocks % LANES_256;
        if lane == 0 {
            self.packed_chunks.push(Chunk8::ZERO);
        }
        let chunk = self
            .packed_chunks
            .last_mut()
            .expect("PackedTraceBuilder missing chunk");
        chunk.x[lane] = block.x;
        chunk.y[lane] = block.y;
        chunk.z[lane] = block.z;
        self.len_blocks += 1;
    }

    #[inline]
    pub(crate) fn extend_from_blocks(&mut self, blocks: &[Block]) {
        for &block in blocks {
            self.push_block(block);
        }
    }

    #[inline]
    pub(crate) fn append_chunk_range(&mut self, chunk: &Chunk8, start: usize, end: usize) {
        if start >= end {
            return;
        }
        if start == 0 && end == LANES_256 && (self.len_blocks % LANES_256) == 0 {
            self.packed_chunks.push(*chunk);
            self.len_blocks += LANES_256;
            return;
        }
        for lane in start..end {
            self.push_block(Block::new(chunk.x[lane], chunk.y[lane], chunk.z[lane]));
        }
    }

    #[inline]
    pub(crate) fn finish(self) -> Trace {
        if self.len_blocks == 0 {
            Trace::empty()
        } else {
            Trace::from_packed_chunks_unchecked(self.packed_chunks.into_boxed_slice(), self.len_blocks)
        }
    }
}

#[inline(always)]
pub(crate) fn append_blocks_range(out: &mut Vec<Block>, trace: &Trace, start: usize, end: usize) {
    if start >= end {
        return;
    }
    let range_len = end - start;
    out.reserve(range_len);

    if let Some(blocks) = trace.storage.blocks.get() {
        out.extend_from_slice(&blocks[start..end]);
        return;
    }

    let packed = trace.packed_chunks();
    let start_chunk = start / LANES_256;
    let end_chunk = (end - 1) / LANES_256;
    let start_lane = start % LANES_256;
    let mut end_lane = end % LANES_256;
    if end_lane == 0 {
        end_lane = LANES_256;
    }

    if start_chunk == end_chunk {
        let chunk = &packed[start_chunk];
        if start_lane == 0 && end_lane == LANES_256 {
            push_chunk_blocks(out, chunk, LANES_256);
        } else {
            push_chunk_block_range(out, chunk, start_lane, end_lane);
        }
        return;
    }

    let first_chunk = &packed[start_chunk];
    if start_lane == 0 {
        push_chunk_blocks(out, first_chunk, LANES_256);
    } else {
        push_chunk_block_range(out, first_chunk, start_lane, LANES_256);
    }

    for chunk in &packed[(start_chunk + 1)..end_chunk] {
        push_chunk_blocks(out, chunk, LANES_256);
    }

    let last_chunk = &packed[end_chunk];
    if end_lane == LANES_256 {
        push_chunk_blocks(out, last_chunk, LANES_256);
    } else {
        push_chunk_block_range(out, last_chunk, 0, end_lane);
    }
}

#[inline(always)]
fn append_blocks_range_packed(
    out: &mut PackedTraceBuilder,
    trace: &Trace,
    start: usize,
    end: usize,
) {
    if start >= end {
        return;
    }

    if let Some(blocks) = trace.storage.blocks.get() {
        out.extend_from_blocks(&blocks[start..end]);
        return;
    }

    let packed = trace.packed_chunks();
    let start_chunk = start / LANES_256;
    let end_chunk = (end - 1) / LANES_256;
    let start_lane = start % LANES_256;
    let mut end_lane = end % LANES_256;
    if end_lane == 0 {
        end_lane = LANES_256;
    }

    if start_chunk == end_chunk {
        let chunk = &packed[start_chunk];
        out.append_chunk_range(chunk, start_lane, end_lane);
        return;
    }

    let first_chunk = &packed[start_chunk];
    out.append_chunk_range(first_chunk, start_lane, LANES_256);

    for chunk in &packed[(start_chunk + 1)..end_chunk] {
        out.append_chunk_range(chunk, 0, LANES_256);
    }

    let last_chunk = &packed[end_chunk];
    out.append_chunk_range(last_chunk, 0, end_lane);
}

/// Каноническая трасса как список блоков между τ.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Trace {
    storage: TraceStorage,
}

impl Trace {
    #[inline]
    pub(crate) fn assert_canonical(&self, context: &str) {
        assert!(
            self.storage.len_blocks != 0,
            "{context} requires at least one block"
        );
    }

    /// Создаёт трассу из списка блоков (если пусто — вставляет нулевой блок).
    #[inline]
    pub fn new(blocks: Vec<Block>) -> Self {
        let blocks = if blocks.is_empty() {
            vec![Block::zero()]
        } else {
            blocks
        };
        Self {
            storage: TraceStorage::new(blocks),
        }
    }

    /// Пустая трасса (ε) как один нулевой блок.
    #[inline]
    pub fn empty() -> Self {
        Self {
            storage: TraceStorage::new(vec![Block::zero()]),
        }
    }

    /// Каноническая чисто временная трасса `[\tau^n]`.
    ///
    /// Пример:
    /// ```
    /// use ts4::Trace;
    /// let t = Trace::tau(3);
    /// assert_eq!(t.len_blocks(), 4);
    /// assert_eq!(t.tau_count(), 3);
    /// ```
    #[inline]
    pub fn tau(n: usize) -> Self {
        let len_blocks = n.checked_add(1).expect("Trace::tau overflow");
        Self {
            storage: TraceStorage::new(vec![Block::zero(); len_blocks]),
        }
    }

    /// Доступ к каноническим блокам как к неизменяемому срезу.
    #[inline]
    pub fn as_blocks(&self) -> &[Block] {
        self.storage
            .blocks
            .get_or_init(|| build_block_view_from_packed(&self.storage.packed_chunks, self.storage.len_blocks))
    }

    /// Потребить трассу и вернуть внутренний список блоков.
    #[inline]
    pub fn into_blocks(self) -> Vec<Block> {
        match self.storage.blocks.into_inner() {
            Some(blocks) => blocks.into_vec(),
            None => build_block_view_from_packed(&self.storage.packed_chunks, self.storage.len_blocks).into_vec(),
        }
    }

    #[inline]
    pub(crate) fn packed_chunks(&self) -> &[Chunk8] {
        &self.storage.packed_chunks
    }

    #[inline]
    pub(crate) fn packed_chunk(&self, index: usize) -> &Chunk8 {
        &self.packed_chunks()[index]
    }

    #[inline]
    pub(crate) fn packed_chunk_count(&self) -> usize {
        self.storage.len_blocks.div_ceil(LANES_256)
    }

    #[inline]
    pub(crate) fn packed_chunk_valid_lanes(&self, index: usize) -> usize {
        let start = index
            .checked_mul(LANES_256)
            .expect("Trace::packed_chunk_valid_lanes index overflow");
        self.storage.len_blocks.saturating_sub(start).min(LANES_256)
    }

    #[inline]
    pub(crate) fn first_block(&self) -> Block {
        self.block_at(0)
    }

    #[inline]
    pub(crate) fn last_block(&self) -> Block {
        self.block_at(self.storage.len_blocks - 1)
    }

    #[inline]
    pub(crate) fn block_at(&self, index: usize) -> Block {
        if let Some(blocks) = self.storage.blocks.get() {
            return blocks[index];
        }
        let chunk_index = index / LANES_256;
        let lane = index % LANES_256;
        let chunk = &self.storage.packed_chunks[chunk_index];
        Block::new(chunk.x[lane], chunk.y[lane], chunk.z[lane])
    }

    #[inline]
    pub(crate) fn blocks_equal_range(
        &self,
        other: &Trace,
        mut self_start: usize,
        mut other_start: usize,
        mut len: usize,
    ) -> bool {
        if let (Some(self_blocks), Some(other_blocks)) =
            (self.storage.blocks.get(), other.storage.blocks.get())
        {
            let self_end = self_start + len;
            let other_end = other_start + len;
            return self_blocks[self_start..self_end] == other_blocks[other_start..other_end];
        }

        while len != 0
            && ((self_start % LANES_256) != 0 || (other_start % LANES_256) != 0)
        {
            if self.block_at(self_start) != other.block_at(other_start) {
                return false;
            }
            self_start += 1;
            other_start += 1;
            len -= 1;
        }

        while len >= LANES_256 {
            let self_chunk = self.packed_chunk(self_start / LANES_256);
            let other_chunk = other.packed_chunk(other_start / LANES_256);
            if self_chunk != other_chunk {
                return false;
            }
            self_start += LANES_256;
            other_start += LANES_256;
            len -= LANES_256;
        }

        while len != 0 {
            if self.block_at(self_start) != other.block_at(other_start) {
                return false;
            }
            self_start += 1;
            other_start += 1;
            len -= 1;
        }

        true
    }

    #[inline]
    pub(crate) fn common_prefix_len(&self, other: &Trace, limit: usize) -> usize {
        if let (Some(self_blocks), Some(other_blocks)) =
            (self.storage.blocks.get(), other.storage.blocks.get())
        {
            for (index, (lhs, rhs)) in self_blocks[..limit]
                .iter()
                .zip(other_blocks[..limit].iter())
                .enumerate()
            {
                if lhs != rhs {
                    return index;
                }
            }
            return limit;
        }

        let mut matched = 0usize;

        while matched < limit && (matched % LANES_256) != 0 {
            if self.block_at(matched) != other.block_at(matched) {
                return matched;
            }
            matched += 1;
        }

        while matched + LANES_256 <= limit {
            let self_chunk = self.packed_chunk(matched / LANES_256);
            let other_chunk = other.packed_chunk(matched / LANES_256);
            if self_chunk != other_chunk {
                break;
            }
            matched += LANES_256;
        }

        while matched < limit {
            if self.block_at(matched) != other.block_at(matched) {
                break;
            }
            matched += 1;
        }

        matched
    }


    #[inline]
    pub(crate) fn compose_capacity_with(&self, other: &Trace) -> usize {
        self.storage
            .len_blocks
            .checked_add(other.storage.len_blocks)
            .and_then(|sum| sum.checked_sub(1))
            .expect("Trace::compose capacity overflow")
    }

    #[cfg(test)]
    #[inline]
    pub(crate) fn from_raw_blocks_unchecked(blocks: Vec<Block>) -> Self {
        Self {
            storage: TraceStorage::new(blocks),
        }
    }

    #[inline]
    pub(crate) fn from_packed_chunks_unchecked(
        packed_chunks: Box<[Chunk8]>,
        len_blocks: usize,
    ) -> Self {
        Self {
            storage: TraceStorage::from_packed_chunks(packed_chunks, len_blocks),
        }
    }

    /// Число блоков (слоёв).
    #[inline]
    pub fn len_blocks(&self) -> usize {
        self.storage.len_blocks
    }

    /// Число τ как (len_blocks - 1).
    #[inline]
    pub fn tau_count(&self) -> usize {
        self.assert_canonical("Trace::tau_count");
        self.storage.len_blocks - 1
    }

    /// Суммарная пространственная масса (L1) по всем блокам.
    #[inline]
    pub fn mass_l1(&self) -> u64 {
        let mut total = 0u64;
        for chunk_index in 0..self.packed_chunk_count() {
            total += sum_packed_chunk_l1(
                self.packed_chunk(chunk_index),
                self.packed_chunk_valid_lanes(chunk_index),
            );
        }
        total
    }

    /// Returns `true` when every block satisfies `|B_i|_1 <= kappa`.
    #[inline]
    pub(crate) fn all_blocks_l1_le(&self, kappa: u32) -> bool {
        for chunk_index in 0..self.packed_chunk_count() {
            let valid_lanes = self.packed_chunk_valid_lanes(chunk_index);
            if packed_chunk_l1_gt_mask(self.packed_chunk(chunk_index), valid_lanes, kappa) != 0 {
                return false;
            }
        }
        true
    }

    /// Layer-wise addition on traces with identical grids, but only when every
    /// resulting layer already fits under `kappa` and no split is required.
    #[inline]
    pub(crate) fn try_parallel_tight(&self, other: &Trace, kappa: u32) -> Option<Trace> {
        if self.len_blocks() != other.len_blocks() {
            return None;
        }

        let chunk_count = self.packed_chunk_count();
        let mut packed = Vec::with_capacity(chunk_count);
        for chunk_index in 0..chunk_count {
            let valid_lanes = self.packed_chunk_valid_lanes(chunk_index);
            let (summed, split_mask) = crate::simd::add_packed_chunks_and_l1_mask(
                self.packed_chunk(chunk_index),
                other.packed_chunk(chunk_index),
                valid_lanes,
                kappa,
            );
            if split_mask != 0 {
                return None;
            }
            packed.push(summed);
        }

        Some(Trace::from_packed_chunks_unchecked(
            packed.into_boxed_slice(),
            self.len_blocks(),
        ))
    }

    /// First-class packed mask for blocks whose L1 exceeds `kappa`.
    #[inline]
    pub fn blocks_l1_gt_mask(&self, kappa: u32) -> BlockMask {
        let mut out = BlockMask::new(self.len_blocks());
        for chunk_index in 0..self.packed_chunk_count() {
            let valid_lanes = self.packed_chunk_valid_lanes(chunk_index);
            let mask =
                packed_chunk_l1_gt_mask(self.packed_chunk(chunk_index), valid_lanes, kappa);
            out.write_chunk_mask(chunk_index, mask);
        }
        out
    }

    /// Проекция трассы в `N^4`: `(#τ, #x, #y, #z)`.
    #[inline]
    pub fn pi(&self) -> (u32, u32, u32, u32) {
        let mut x = 0u64;
        let mut y = 0u64;
        let mut z = 0u64;
        for chunk_index in 0..self.packed_chunk_count() {
            let valid_lanes = self.packed_chunk_valid_lanes(chunk_index);
            let (sx, sy, sz) = sum_packed_chunk_xyz(self.packed_chunk(chunk_index), valid_lanes);
            x = x.checked_add(sx).expect("pi_trace x overflow");
            y = y.checked_add(sy).expect("pi_trace y overflow");
            z = z.checked_add(sz).expect("pi_trace z overflow");
        }
        (
            u32::try_from(self.tau_count()).expect("pi_trace tau overflow"),
            u32::try_from(x).expect("pi_trace x overflow"),
            u32::try_from(y).expect("pi_trace y overflow"),
            u32::try_from(z).expect("pi_trace z overflow"),
        )
    }

    /// Каноническая композиция (склейка границы блоков).
    ///
    /// Пример:
    /// ```
    /// use ts4::{Block, Trace};
    /// let a = Trace::new(vec![Block::new(1,0,0), Block::zero()]); // x τ
    /// let b = Trace::new(vec![Block::new(0,1,0)]);                // y
    /// let c = a.compose(&b);                                      // x τ y
    /// assert_eq!(c.len_blocks(), 2);
    /// ```
    #[inline]
    pub fn compose(&self, other: &Trace) -> Trace {
        self.assert_canonical("Trace::compose");
        other.assert_canonical("Trace::compose");
        let out_capacity = self.compose_capacity_with(other);
        let mut out = PackedTraceBuilder::with_capacity(out_capacity);
        if self.len_blocks() > 1 {
            append_blocks_range_packed(&mut out, self, 0, self.len_blocks() - 1);
        }
        let merged = self.last_block().add(other.first_block());
        out.push_block(merged);
        if other.len_blocks() > 1 {
            append_blocks_range_packed(&mut out, other, 1, other.len_blocks());
        }
        out.finish()
    }

    /// Дополнение нулевыми блоками до заданной длины.
    #[inline]
    pub fn pad_to(&self, len: usize) -> Trace {
        self.assert_canonical("Trace::pad_to");
        if self.storage.len_blocks >= len {
            return self.clone();
        }
        let mut out = Vec::with_capacity(len);
        append_blocks_range(&mut out, self, 0, self.len_blocks());
        out.resize(len, Block::zero());
        Trace::new(out)
    }

    /// Построить трассу из слова над {t,x,y,z}, где `t` соответствует τ.
    ///
    /// Пример:
    /// ```
    /// use ts4::Trace;
    /// let t = Trace::from_word("txty");
    /// assert_eq!(t.len_blocks(), 3);
    /// ```
    pub fn from_word(word: &str) -> Trace {
        #[inline]
        fn checked_inc(value: u32) -> u32 {
            match value.checked_add(1) {
                Some(next) => next,
                None => panic!("Trace::from_word overflow"),
            }
        }

        let mut blocks = Vec::new();
        let mut cur = Block::zero();
        for ch in word.chars() {
            match ch {
                't' => {
                    blocks.push(cur);
                    cur = Block::zero();
                }
                'x' => cur.x = checked_inc(cur.x),
                'y' => cur.y = checked_inc(cur.y),
                'z' => cur.z = checked_inc(cur.z),
                _ => {}
            }
        }
        blocks.push(cur);
        Trace::new(blocks)
    }
}
