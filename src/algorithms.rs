//! Алгоритмы физического слоя (κ-нормализация и операции).
//!
//! Быстрое использование:
//! ```
//! use ts4::{Block, Trace, odot_kappa, parallel_kappa, phi_kappa};
//! let u = Trace::new(vec![Block::new(2,0,0)]);
//! let v = Trace::new(vec![Block::new(0,1,0)]);
//! let r = odot_kappa(&u, &v, 3);
//! let p = parallel_kappa(&u, &v, 3);
//! let n = phi_kappa(&r, 3);
//! ```

use crate::simd::{
    Chunk8, LANES_256, add_block_to_packed_chunk_and_l1_mask, add_packed_chunks_and_l1_mask,
    packed_chunk_l1_gt_mask, push_chunk_block_range, push_chunk_blocks,
};
use crate::trace::{PackedTraceBuilder, Trace};
use crate::types::Block;

#[inline(always)]
fn checked_l1_components(x: u32, y: u32, z: u32) -> u32 {
    let xy = match x.checked_add(y) {
        Some(value) => value,
        None => panic!("Block::l1 overflow"),
    };
    match xy.checked_add(z) {
        Some(value) => value,
        None => panic!("Block::l1 overflow"),
    }
}

#[inline(always)]
fn count_split_parts_for_mass(mass: u32, kappa: u32) -> usize {
    if mass == 0 {
        return 0;
    }
    (1 + (mass - 1) / kappa) as usize
}

#[inline(always)]
fn count_split_parts_kappa(block: Block, kappa: u32) -> usize {
    count_split_parts_for_mass(block.l1(), kappa)
}

#[inline(always)]
fn extend_split_xyz_kappa(
    out: &mut Vec<Block>,
    mut x: u32,
    mut y: u32,
    mut z: u32,
    mut remain_mass: u32,
    kappa: u32,
) {
    debug_assert!(kappa >= 1);
    while remain_mass > kappa {
        let take_x = x.min(kappa);
        x -= take_x;

        let rem_after_x = kappa - take_x;
        let take_y = y.min(rem_after_x);
        y -= take_y;

        let rem_after_y = rem_after_x - take_y;
        let take_z = z.min(rem_after_y);
        z -= take_z;

        out.push(Block::new(take_x, take_y, take_z));
        remain_mass -= kappa;
    }

    out.push(Block::new(x, y, z));
}

#[inline(always)]
fn extend_split_block_kappa_with_mass(out: &mut Vec<Block>, block: Block, mass: u32, kappa: u32) {
    extend_split_xyz_kappa(out, block.x, block.y, block.z, mass, kappa);
}

#[inline(always)]
fn extend_split_block_kappa(out: &mut Vec<Block>, block: Block, kappa: u32) {
    let mass = block.l1();
    extend_split_block_kappa_with_mass(out, block, mass, kappa);
}

#[inline(always)]
fn push_block_kappa(out: &mut Vec<Block>, block: Block, kappa: u32) {
    let mass = block.l1();
    if mass > kappa {
        out.reserve(count_split_parts_for_mass(mass, kappa).saturating_sub(1));
        extend_split_block_kappa_with_mass(out, block, mass, kappa);
    } else {
        out.push(block);
    }
}

fn extend_split_masked_chunk8_with_valid_lanes(
    out: &mut Vec<Block>,
    chunk: &Chunk8,
    split_mask: u8,
    valid_lanes: usize,
    kappa: u32,
) {
    let mut lane = 0usize;
    let mut mask = split_mask;
    while mask != 0 {
        let split_lane = mask.trailing_zeros() as usize;
        if lane < split_lane {
            push_chunk_block_range(out, chunk, lane, split_lane);
        }
        let x = chunk.x[split_lane];
        let y = chunk.y[split_lane];
        let z = chunk.z[split_lane];
        let mass = checked_l1_components(x, y, z);
        out.reserve(count_split_parts_for_mass(mass, kappa).saturating_sub(1));
        extend_split_xyz_kappa(out, x, y, z, mass, kappa);
        lane = split_lane + 1;
        mask &= mask - 1;
    }
    if lane < valid_lanes {
        push_chunk_block_range(out, chunk, lane, valid_lanes);
    }
}

#[inline(always)]
fn append_trace_range_kappa(
    out: &mut Vec<Block>,
    trace: &Trace,
    start: usize,
    end: usize,
    kappa: u32,
) {
    if start >= end {
        return;
    }

    let mut cursor = start;
    let first_aligned = ((start + LANES_256 - 1) / LANES_256) * LANES_256;
    let scalar_prefix_end = first_aligned.min(end);
    while cursor < scalar_prefix_end {
        push_block_kappa(out, trace.block_at(cursor), kappa);
        cursor += 1;
    }

    let packed_end = end - ((end - cursor) % LANES_256);
    while cursor < packed_end {
        let chunk_index = cursor / LANES_256;
        let chunk = trace.packed_chunk(chunk_index);
        let split_mask = packed_chunk_l1_gt_mask(chunk, LANES_256, kappa);
        if split_mask == 0 {
            push_chunk_blocks(out, chunk, LANES_256);
        } else {
            extend_split_masked_chunk8_with_valid_lanes(out, chunk, split_mask, LANES_256, kappa);
        }
        cursor += LANES_256;
    }

    while cursor < end {
        push_block_kappa(out, trace.block_at(cursor), kappa);
        cursor += 1;
    }
}

#[inline(always)]
fn append_trace_range_added_kappa(
    out: &mut Vec<Block>,
    trace: &Trace,
    start: usize,
    end: usize,
    addend: Block,
    kappa: u32,
) {
    if start >= end {
        return;
    }

    let mut cursor = start;
    let first_aligned = start.div_ceil(LANES_256) * LANES_256;
    let scalar_prefix_end = first_aligned.min(end);
    while cursor < scalar_prefix_end {
        push_block_kappa(out, trace.block_at(cursor).add(addend), kappa);
        cursor += 1;
    }

    let packed_end = end - ((end - cursor) % LANES_256);
    while cursor < packed_end {
        let chunk_index = cursor / LANES_256;
        let chunk = trace.packed_chunk(chunk_index);
        let (summed, split_mask) =
            add_block_to_packed_chunk_and_l1_mask(chunk, addend, LANES_256, kappa);
        if split_mask == 0 {
            push_chunk_blocks(out, &summed, LANES_256);
        } else {
            extend_split_masked_chunk8_with_valid_lanes(out, &summed, split_mask, LANES_256, kappa);
        }
        cursor += LANES_256;
    }

    while cursor < end {
        push_block_kappa(out, trace.block_at(cursor).add(addend), kappa);
        cursor += 1;
    }
}

#[inline(always)]
fn append_trace_range_added_interleaved_middle_kappa(
    out: &mut Vec<Block>,
    trace: &Trace,
    start: usize,
    end: usize,
    addend: Block,
    middle: &[Block],
    kappa: u32,
) {
    if start >= end {
        return;
    }
    if middle.is_empty() {
        append_trace_range_added_kappa(out, trace, start, end, addend, kappa);
        return;
    }
    if middle.len() == 1 {
        let middle_block = middle[0];
        let mut cursor = start;
        let first_aligned = start.div_ceil(LANES_256) * LANES_256;
        let scalar_prefix_end = first_aligned.min(end);
        while cursor < scalar_prefix_end {
            push_block_kappa(out, trace.block_at(cursor).add(addend), kappa);
            out.push(middle_block);
            cursor += 1;
        }

        let packed_end = end - ((end - cursor) % LANES_256);
        while cursor < packed_end {
            let chunk_index = cursor / LANES_256;
            let chunk = trace.packed_chunk(chunk_index);
            let (summed, split_mask) =
                add_block_to_packed_chunk_and_l1_mask(chunk, addend, LANES_256, kappa);
            if split_mask == 0 {
                for lane in 0..LANES_256 {
                    out.push(Block::new(summed.x[lane], summed.y[lane], summed.z[lane]));
                    out.push(middle_block);
                }
            } else {
                for lane in 0..LANES_256 {
                    if (split_mask & (1 << lane)) == 0 {
                        out.push(Block::new(summed.x[lane], summed.y[lane], summed.z[lane]));
                    } else {
                        let x = summed.x[lane];
                        let y = summed.y[lane];
                        let z = summed.z[lane];
                        let mass = checked_l1_components(x, y, z);
                        out.reserve(count_split_parts_for_mass(mass, kappa).saturating_sub(1));
                        extend_split_xyz_kappa(out, x, y, z, mass, kappa);
                    }
                    out.push(middle_block);
                }
            }
            cursor += LANES_256;
        }

        while cursor < end {
            push_block_kappa(out, trace.block_at(cursor).add(addend), kappa);
            out.push(middle_block);
            cursor += 1;
        }
        return;
    }

    let mut cursor = start;
    let first_aligned = start.div_ceil(LANES_256) * LANES_256;
    let scalar_prefix_end = first_aligned.min(end);
    while cursor < scalar_prefix_end {
        push_block_kappa(out, trace.block_at(cursor).add(addend), kappa);
        out.extend_from_slice(middle);
        cursor += 1;
    }

    let packed_end = end - ((end - cursor) % LANES_256);
    while cursor < packed_end {
        let chunk_index = cursor / LANES_256;
        let chunk = trace.packed_chunk(chunk_index);
        let (summed, split_mask) =
            add_block_to_packed_chunk_and_l1_mask(chunk, addend, LANES_256, kappa);
        if split_mask == 0 {
            for lane in 0..LANES_256 {
                out.push(Block::new(summed.x[lane], summed.y[lane], summed.z[lane]));
                out.extend_from_slice(middle);
            }
        } else {
            for lane in 0..LANES_256 {
                if (split_mask & (1 << lane)) == 0 {
                    out.push(Block::new(summed.x[lane], summed.y[lane], summed.z[lane]));
                } else {
                    let x = summed.x[lane];
                    let y = summed.y[lane];
                    let z = summed.z[lane];
                    let mass = checked_l1_components(x, y, z);
                    out.reserve(count_split_parts_for_mass(mass, kappa).saturating_sub(1));
                    extend_split_xyz_kappa(out, x, y, z, mass, kappa);
                }
                out.extend_from_slice(middle);
            }
        }
        cursor += LANES_256;
    }

    while cursor < end {
        push_block_kappa(out, trace.block_at(cursor).add(addend), kappa);
        out.extend_from_slice(middle);
        cursor += 1;
    }
}

#[inline(always)]
fn trace_range_added_fits_kappa(
    trace: &Trace,
    start: usize,
    end: usize,
    addend: Block,
    kappa: u32,
) -> bool {
    if start >= end {
        return true;
    }

    let mut cursor = start;
    let first_aligned = start.div_ceil(LANES_256) * LANES_256;
    let scalar_prefix_end = first_aligned.min(end);
    while cursor < scalar_prefix_end {
        if trace.block_at(cursor).add(addend).l1() > kappa {
            return false;
        }
        cursor += 1;
    }

    let packed_end = end - ((end - cursor) % LANES_256);
    while cursor < packed_end {
        let chunk_index = cursor / LANES_256;
        let chunk = trace.packed_chunk(chunk_index);
        let (_, split_mask) = add_block_to_packed_chunk_and_l1_mask(chunk, addend, LANES_256, kappa);
        if split_mask != 0 {
            return false;
        }
        cursor += LANES_256;
    }

    while cursor < end {
        if trace.block_at(cursor).add(addend).l1() > kappa {
            return false;
        }
        cursor += 1;
    }

    true
}

#[inline(always)]
fn append_trace_range_added_interleaved_middle_tight(
    out: &mut PackedTraceBuilder,
    trace: &Trace,
    start: usize,
    end: usize,
    addend: Block,
    middle: &[Block],
) {
    if start >= end {
        return;
    }

    let mut cursor = start;
    let first_aligned = start.div_ceil(LANES_256) * LANES_256;
    let scalar_prefix_end = first_aligned.min(end);
    while cursor < scalar_prefix_end {
        out.push_block(trace.block_at(cursor).add(addend));
        out.extend_from_blocks(middle);
        cursor += 1;
    }

    let packed_end = end - ((end - cursor) % LANES_256);
    while cursor < packed_end {
        let chunk_index = cursor / LANES_256;
        let chunk = trace.packed_chunk(chunk_index);
        let (summed, split_mask) =
            add_block_to_packed_chunk_and_l1_mask(chunk, addend, LANES_256, u32::MAX);
        debug_assert_eq!(split_mask, 0);
        for lane in 0..LANES_256 {
            out.push_block(Block::new(summed.x[lane], summed.y[lane], summed.z[lane]));
            out.extend_from_blocks(middle);
        }
        cursor += LANES_256;
    }

    while cursor < end {
        out.push_block(trace.block_at(cursor).add(addend));
        out.extend_from_blocks(middle);
        cursor += 1;
    }
}

/// Разбиение блока по ограничению κ (жадное, x→y→z).
/// Разбивает переполненный блок на список блоков, каждый с L1 <= κ.
#[inline]
pub fn split_block_kappa(block: Block, kappa: u32) -> Vec<Block> {
    assert!(kappa >= 1, "split_block_kappa requires kappa >= 1");
    let mut out = Vec::with_capacity(count_split_parts_kappa(block, kappa));
    extend_split_block_kappa(&mut out, block, kappa);
    out
}

/// Нормализация трассы по κ (разбиение переполненных блоков).
#[inline]
pub fn phi_kappa(trace: &Trace, kappa: u32) -> Trace {
    assert!(kappa >= 1, "phi_kappa requires kappa >= 1");
    trace.assert_canonical("phi_kappa");
    let trace_len = trace.len_blocks();

    let mut out: Option<Vec<Block>> = None;
    let mut processed = 0usize;

    for chunk_index in 0..trace.packed_chunk_count() {
        let valid_lanes = trace.packed_chunk_valid_lanes(chunk_index);
        let chunk = trace.packed_chunk(chunk_index);
        let mask = packed_chunk_l1_gt_mask(chunk, valid_lanes, kappa);
        if mask == 0 {
            if let Some(buf) = out.as_mut() {
                push_chunk_blocks(buf, chunk, valid_lanes);
            }
        } else {
            let buf = out.get_or_insert_with(|| {
                let mut prefilled = Vec::with_capacity(trace_len);
                let full_chunks = processed / LANES_256;
                for chunk_index in 0..full_chunks {
                    push_chunk_blocks(&mut prefilled, trace.packed_chunk(chunk_index), LANES_256);
                }
                for index in (full_chunks * LANES_256)..processed {
                    prefilled.push(trace.block_at(index));
                }
                prefilled
            });
            extend_split_masked_chunk8_with_valid_lanes(buf, chunk, mask, valid_lanes, kappa);
        }
        processed += valid_lanes;
    }

    match out {
        Some(buffer) => Trace::new(buffer),
        None => trace.clone(),
    }
}

/// Физическая последовательность: композиция + нормализация κ.
#[inline]
pub fn odot_kappa(u: &Trace, v: &Trace, kappa: u32) -> Trace {
    assert!(kappa >= 1, "odot_kappa requires kappa >= 1");
    u.assert_canonical("odot_kappa");
    v.assert_canonical("odot_kappa");
    let out_capacity = u.compose_capacity_with(v);
    let mut out = Vec::with_capacity(out_capacity);

    if u.len_blocks() > 1 {
        append_trace_range_kappa(&mut out, u, 0, u.len_blocks() - 1, kappa);
    }

    let merged = u.last_block().add(v.first_block());
    push_block_kappa(&mut out, merged, kappa);

    if v.len_blocks() > 1 {
        append_trace_range_kappa(&mut out, v, 1, v.len_blocks(), kappa);
    }

    Trace::new(out)
}

/// Физическая параллельность: padding, послойная сумма, нормализация κ.
#[inline]
pub fn parallel_kappa(u: &Trace, v: &Trace, kappa: u32) -> Trace {
    assert!(kappa >= 1, "parallel_kappa requires kappa >= 1");
    u.assert_canonical("parallel_kappa");
    v.assert_canonical("parallel_kappa");

    let overlap_len = u.len_blocks().min(v.len_blocks());
    let len = u.len_blocks().max(v.len_blocks());
    let mut out = Vec::with_capacity(len);
    let overlap_chunks = overlap_len.div_ceil(LANES_256);

    for chunk_index in 0..overlap_chunks {
        let offset = chunk_index * LANES_256;
        let valid_lanes = (overlap_len - offset).min(LANES_256);
        let left = u.packed_chunk(chunk_index);
        let right = v.packed_chunk(chunk_index);
        let (summed, split_mask) = add_packed_chunks_and_l1_mask(left, right, valid_lanes, kappa);

        if split_mask == 0 {
            push_chunk_blocks(&mut out, &summed, valid_lanes);
        } else {
            extend_split_masked_chunk8_with_valid_lanes(
                &mut out,
                &summed,
                split_mask,
                valid_lanes,
                kappa,
            );
        }
    }

    if u.len_blocks() > overlap_len {
        for index in overlap_len..u.len_blocks() {
            push_block_kappa(&mut out, u.block_at(index), kappa);
        }
    } else if v.len_blocks() > overlap_len {
        for index in overlap_len..v.len_blocks() {
            push_block_kappa(&mut out, v.block_at(index), kappa);
        }
    }

    Trace::new(out)
}

/// Физическое уточнение времени: подстановка τ↦v и нормализация κ.
#[inline]
pub fn otimes_kappa(u: &Trace, v: &Trace, kappa: u32) -> Trace {
    assert!(kappa >= 1, "otimes_kappa requires kappa >= 1");
    u.assert_canonical("otimes_kappa");
    v.assert_canonical("otimes_kappa");
    let u_len = u.len_blocks();
    let v_len = v.len_blocks();
    if u_len == 1 {
        return u.clone();
    }
    let v_first = v.first_block();
    let v_last = v.last_block();
    let mut acc = u.first_block();

    // These blocks are inserted unchanged for each outer τ-substitution step.
    let mut normalized_middle = Vec::new();
    if u_len > 1 && v_len > 2 {
        append_trace_range_kappa(&mut normalized_middle, v, 1, v_len - 1, kappa);
    }

    let estimated_len = match v_len {
        1 | 2 => u_len,
        _ => {
            let middle_len = normalized_middle.len();
            let repeat_span = u_len
                .saturating_sub(1)
                .checked_mul(middle_len)
                .expect("otimes_kappa capacity overflow");
            u_len
                .checked_add(repeat_span)
                .expect("otimes_kappa capacity overflow")
        }
    };
    let last_acc = v_last.add(u.last_block());

    if v_len == 1 {
        let mut out = Vec::with_capacity(estimated_len);
        for index in 1..u_len {
            let next = u.block_at(index);
            let left = acc.add(v_first);
            acc = left.add(next);
        }
        push_block_kappa(&mut out, acc, kappa);
        return Trace::new(out);
    }

    if v_len == 2 {
        let mut out = Vec::with_capacity(estimated_len);
        push_block_kappa(&mut out, acc.add(v_first), kappa);
        if u_len > 2 {
            let interior_addend = v_last.add(v_first);
            append_trace_range_added_kappa(&mut out, u, 1, u_len - 1, interior_addend, kappa);
        }
        push_block_kappa(&mut out, last_acc, kappa);
        return Trace::new(out);
    }

    let first_left = acc.add(v_first);
    let direct_packed_ok = first_left.l1() <= kappa
        && last_acc.l1() <= kappa
        && (u_len <= 2
            || trace_range_added_fits_kappa(u, 1, u_len - 1, v_last.add(v_first), kappa));
    if direct_packed_ok {
        let mut out = PackedTraceBuilder::with_capacity(estimated_len);
        out.push_block(first_left);
        out.extend_from_blocks(&normalized_middle);
        if u_len > 2 {
            append_trace_range_added_interleaved_middle_tight(
                &mut out,
                u,
                1,
                u_len - 1,
                v_last.add(v_first),
                &normalized_middle,
            );
        }
        out.push_block(last_acc);
        return out.finish();
    }

    let mut out = Vec::with_capacity(estimated_len);
    push_block_kappa(&mut out, first_left, kappa);
    if normalized_middle.len() == 1 {
        out.push(normalized_middle[0]);
    } else {
        out.extend_from_slice(&normalized_middle);
    }

    if u_len > 2 {
        let interior_addend = v_last.add(v_first);
        append_trace_range_added_interleaved_middle_kappa(
            &mut out,
            u,
            1,
            u_len - 1,
            interior_addend,
            &normalized_middle,
            kappa,
        );
    }

    push_block_kappa(&mut out, last_acc, kappa);
    Trace::new(out)
}
