use crate::algorithms::{odot_kappa, otimes_kappa, parallel_kappa, phi_kappa, split_block_kappa};
use crate::divisibility::{
    SolveMode, left_divide_monomial, left_divide_trace, left_divide_ts4, left_divide_ts4_monomial,
    left_divide_ts4_solve, left_divide_ts4_unique, right_divide_monomial, right_divide_trace,
    right_divide_ts4_monomial,
};
use crate::invariants::{layers, mass_l1, min_layers_for_mass, min_tau_for_mass, tau_count};
use crate::modular::{TS4Mod, gcd_monomial, gcd_u32};
use crate::physical::{SyncTrace, is_kappa_admissible, is_tight_core};
use crate::projections::{pi_trace, pi_ts4, proj_r4};
use crate::simd::{BlockMask, blocks_l1_gt, blocks_l1_gt_mask, sum_l1_blocks};
use crate::theory::{
    is_trace_atom, left_divide_trace_is_exact, monomial_left_divides, normalize_trace,
    normalize_ts4, pi_trace_compose_morphism_holds, proj_r4_matches_scaled_pi,
    trace_left_cancellation_holds, traces_equal_by_normal_form, ts4_equal_by_normal_form,
    ts4_noncommutative_example, ts4_semiring_laws_hold,
};
use crate::trace::{Trace, append_blocks_range};
use crate::ts4::TS4;
use crate::types::Block;
use std::collections::BTreeMap;

fn scalar_sum_l1(blocks: &[Block]) -> u64 {
    blocks.iter().map(|b| b.l1() as u64).sum()
}

fn scalar_blocks_l1_gt(blocks: &[Block], kappa: u32) -> Vec<bool> {
    blocks.iter().map(|b| b.l1() > kappa).collect()
}

fn trace_as_blocks(trace: &Trace) -> &[Block] {
    trace.as_blocks()
}

fn trace_into_blocks(trace: Trace) -> Vec<Block> {
    trace.into_blocks()
}

fn trace_from_raw_blocks_unchecked(blocks: Vec<Block>) -> Trace {
    Trace::from_raw_blocks_unchecked(blocks)
}

fn chunk_to_blocks(chunk: &crate::simd::Chunk8, valid_lanes: usize) -> Vec<Block> {
    (0..valid_lanes)
        .map(|lane| Block::new(chunk.x[lane], chunk.y[lane], chunk.z[lane]))
        .collect()
}

fn ts4_from_pairs(pairs: &[(Trace, u32)]) -> TS4 {
    let mut terms = BTreeMap::new();
    for (trace, coeff) in pairs.iter() {
        if *coeff != 0 {
            terms.insert(trace.clone(), *coeff);
        }
    }
    TS4::from_raw_terms_unchecked(terms)
}

fn ts4mod_from_pairs(modulus: u32, pairs: &[(Trace, u32)]) -> TS4Mod {
    let mut terms = BTreeMap::new();
    for (trace, coeff) in pairs.iter() {
        let reduced = coeff % modulus;
        if reduced != 0 {
            terms.insert(trace.clone(), reduced);
        }
    }
    TS4Mod::from_raw_terms_unchecked(modulus, terms)
}

fn assert_simd_oracle(blocks: &[Block], kappa: u32) {
    assert_eq!(sum_l1_blocks(blocks), scalar_sum_l1(blocks));
    assert_eq!(
        blocks_l1_gt(blocks, kappa),
        scalar_blocks_l1_gt(blocks, kappa)
    );
}

#[test]
fn compose_blocks_boundary() {
    let a = Trace::new(vec![Block::new(1, 0, 0), Block::zero()]);
    let b = Trace::new(vec![Block::new(0, 1, 0)]);
    let c = a.compose(&b);
    assert_eq!(
        trace_into_blocks(c),
        vec![Block::new(1, 0, 0), Block::new(0, 1, 0)]
    );
}

#[test]
fn split_block_respects_kappa() {
    let b = Block::new(5, 0, 0);
    let parts = split_block_kappa(b, 4);
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].l1(), 4);
    assert_eq!(parts[1].l1(), 1);
}

#[test]
fn split_block_exact_output_for_mixed_coordinates() {
    let parts = split_block_kappa(Block::new(3, 2, 1), 4);
    assert_eq!(parts, vec![Block::new(3, 1, 0), Block::new(0, 1, 1)]);
}

#[test]
fn split_block_exact_limit_stays_single_block() {
    let parts = split_block_kappa(Block::new(2, 1, 0), 3);
    assert_eq!(parts, vec![Block::new(2, 1, 0)]);
}

#[test]
fn block_coordinate_accessors_match_constructor_values() {
    let block = Block::new(7, 11, 13);
    assert_eq!(block.x(), 7);
    assert_eq!(block.y(), 11);
    assert_eq!(block.z(), 13);
}

#[test]
#[should_panic(expected = "split_block_kappa requires kappa >= 1")]
fn split_block_rejects_zero_kappa() {
    let _ = split_block_kappa(Block::new(1, 0, 0), 0);
}

#[test]
fn phi_kappa_preserves_mass() {
    let t = Trace::new(vec![Block::new(3, 2, 0)]);
    let p = phi_kappa(&t, 3);
    let sum_in = trace_as_blocks(&t).iter().map(|b| b.l1()).sum::<u32>();
    let sum_out = trace_as_blocks(&p).iter().map(|b| b.l1()).sum::<u32>();
    assert_eq!(sum_in, sum_out);
}

#[test]
fn phi_kappa_exact_output_for_overflow_block() {
    let t = Trace::new(vec![Block::new(3, 2, 1)]);
    let p = phi_kappa(&t, 4);
    assert_eq!(
        trace_into_blocks(p),
        vec![Block::new(3, 1, 0), Block::new(0, 1, 1)]
    );
}

#[test]
fn phi_kappa_exact_output_keeps_boundary_block_at_limit() {
    let t = Trace::new(vec![Block::new(2, 1, 0), Block::new(4, 0, 0)]);
    let p = phi_kappa(&t, 3);
    assert_eq!(
        trace_into_blocks(p),
        vec![
            Block::new(2, 1, 0),
            Block::new(3, 0, 0),
            Block::new(1, 0, 0),
        ]
    );
}

#[test]
fn phi_kappa_is_idempotent() {
    let trace = Trace::new(vec![
        Block::zero(),
        Block::new(5, 0, 0),
        Block::new(0, 4, 1),
        Block::new(2, 1, 2),
        Block::zero(),
        Block::new(1, 3, 1),
        Block::new(4, 0, 2),
        Block::zero(),
        Block::new(0, 5, 0),
        Block::new(7, 0, 0),
    ]);
    let once = phi_kappa(&trace, 4);
    let twice = phi_kappa(&once, 4);
    assert_eq!(twice, once);
}

#[test]
fn parallel_kappa_padding() {
    let u = Trace::new(vec![Block::new(1, 0, 0)]);
    let v = Trace::new(vec![Block::new(0, 1, 0), Block::new(1, 0, 0)]);
    let p = parallel_kappa(&u, &v, 4);
    assert_eq!(
        trace_into_blocks(p),
        vec![Block::new(1, 1, 0), Block::new(1, 0, 0)]
    );
}

#[test]
fn parallel_kappa_is_commutative_on_mismatched_lengths() {
    let u = Trace::new(vec![
        Block::new(3, 2, 0),
        Block::new(1, 0, 0),
        Block::zero(),
        Block::new(4, 0, 0),
        Block::new(2, 2, 1),
        Block::zero(),
        Block::new(5, 0, 0),
        Block::new(0, 1, 0),
        Block::new(0, 0, 5),
        Block::new(1, 1, 1),
    ]);
    let v = Trace::new(vec![
        Block::new(1, 1, 1),
        Block::new(0, 4, 1),
        Block::new(2, 1, 2),
        Block::zero(),
        Block::new(7, 0, 0),
        Block::new(0, 0, 1),
    ]);

    let uv = parallel_kappa(&u, &v, 4);
    let vu = parallel_kappa(&v, &u, 4);
    assert_eq!(uv, vu);
}

#[test]
fn parallel_kappa_exact_output_for_overflow_layer() {
    let u = Trace::new(vec![Block::new(3, 1, 0)]);
    let v = Trace::new(vec![Block::new(2, 2, 1)]);
    let p = parallel_kappa(&u, &v, 4);
    assert_eq!(
        trace_into_blocks(p),
        vec![
            Block::new(4, 0, 0),
            Block::new(1, 3, 0),
            Block::new(0, 0, 1),
        ]
    );
}

#[test]
fn parallel_kappa_exact_output_keeps_limit_layer_unsplit() {
    let u = Trace::new(vec![Block::new(1, 1, 0), Block::new(2, 0, 0)]);
    let v = Trace::new(vec![Block::new(1, 0, 1), Block::new(0, 0, 1)]);
    let p = parallel_kappa(&u, &v, 4);
    assert_eq!(
        trace_into_blocks(p),
        vec![Block::new(2, 1, 1), Block::new(2, 0, 1)]
    );
}

#[test]
fn parallel_kappa_fast_path() {
    let u = Trace::new(vec![Block::new(1, 0, 0), Block::zero()]);
    let v = Trace::new(vec![Block::new(0, 1, 0), Block::zero()]);
    let p = parallel_kappa(&u, &v, 4);
    assert_eq!(
        trace_into_blocks(p),
        vec![Block::new(1, 1, 0), Block::zero()]
    );
}

#[test]
fn odot_kappa_exact_output_for_overflow_boundary() {
    let u = Trace::new(vec![Block::new(3, 1, 0)]);
    let v = Trace::new(vec![Block::new(2, 2, 1)]);
    let r = odot_kappa(&u, &v, 4);
    assert_eq!(
        trace_into_blocks(r),
        vec![
            Block::new(4, 0, 0),
            Block::new(1, 3, 0),
            Block::new(0, 0, 1),
        ]
    );
}

#[test]
fn odot_kappa_exact_output_keeps_limit_boundary_unsplit() {
    let u = Trace::new(vec![Block::new(1, 0, 0), Block::new(1, 1, 0)]);
    let v = Trace::new(vec![Block::new(1, 0, 1), Block::zero()]);
    let r = odot_kappa(&u, &v, 4);
    assert_eq!(
        trace_into_blocks(r),
        vec![Block::new(1, 0, 0), Block::new(2, 1, 1), Block::zero()]
    );
}

#[test]
fn otimes_preserves_tau_free_trace() {
    let u = Trace::new(vec![Block::new(2, 0, 1)]);
    let v = Trace::new(vec![Block::new(1, 0, 0), Block::new(0, 1, 0)]);
    let r = otimes_kappa(&u, &v, 8);
    assert_eq!(r, u);
}

#[test]
fn otimes_identity_on_single_tau() {
    let u = Trace::new(vec![Block::new(1, 0, 0), Block::zero()]);
    let v = Trace::new(vec![Block::zero(), Block::zero()]);
    let r = otimes_kappa(&u, &v, 4);
    assert_eq!(r, u);
}

#[test]
fn otimes_pure_temporal_multiplies_time() {
    let u = Trace::new(vec![
        Block::zero(),
        Block::zero(),
        Block::zero(),
        Block::zero(),
    ]);
    let v = Trace::new(vec![
        Block::zero(),
        Block::zero(),
        Block::zero(),
        Block::zero(),
        Block::zero(),
    ]);
    let r = otimes_kappa(&u, &v, 4);
    assert_eq!(r.tau_count(), 12);
    assert!(trace_as_blocks(&r).iter().all(|b| *b == Block::zero()));
}

#[test]
fn otimes_substitutes_tau_with_trace_structure() {
    let u = Trace::new(vec![Block::zero(), Block::zero()]);
    let v = Trace::new(vec![Block::new(1, 0, 0), Block::new(0, 1, 0)]);
    let r = otimes_kappa(&u, &v, 4);
    assert_eq!(r, v);
}

#[test]
fn otimes_repeats_substitution_across_multiple_taus() {
    let u = Trace::new(vec![
        Block::new(1, 0, 0),
        Block::new(0, 0, 1),
        Block::new(0, 1, 0),
    ]);
    let v = Trace::new(vec![Block::new(1, 0, 0), Block::new(0, 1, 0)]);
    let r = otimes_kappa(&u, &v, 8);
    assert_eq!(
        trace_into_blocks(r),
        vec![
            Block::new(2, 0, 0),
            Block::new(1, 1, 1),
            Block::new(0, 2, 0),
        ]
    );
}

#[test]
fn ts4_compose_monomials() {
    let t1 = Trace::new(vec![Block::new(1, 0, 0)]);
    let t2 = Trace::new(vec![Block::new(0, 1, 0)]);
    let a = TS4::from_trace(t1.clone(), 3);
    let b = TS4::from_trace(t2.clone(), 2);
    let c = a.compose(&b);
    let t = t1.compose(&t2);
    assert_eq!(c.get_coeff(&t), Some(6));
}

#[test]
fn ts4_add_exact_output_merges_shared_terms() {
    let x = Trace::new(vec![Block::new(1, 0, 0)]);
    let y = Trace::new(vec![Block::new(0, 1, 0)]);
    let z = Trace::new(vec![Block::new(0, 0, 1)]);
    let left = TS4::from_trace(x.clone(), 2).add(&TS4::from_trace(y.clone(), 3));
    let right = TS4::from_trace(x.clone(), 5).add(&TS4::from_trace(z.clone(), 7));
    let sum = left.add(&right);

    assert_eq!(sum, ts4_from_pairs(&[(x, 7), (y, 3), (z, 7)]));
}

#[test]
fn ts4_compose_exact_output_accumulates_overlapping_products() {
    let x = Trace::new(vec![Block::new(1, 0, 0)]);
    let y = Trace::new(vec![Block::new(0, 1, 0)]);
    let left = TS4::from_trace(x.clone(), 2).add(&TS4::from_trace(y.clone(), 3));
    let right = TS4::from_trace(x.clone(), 5).add(&TS4::from_trace(y.clone(), 7));
    let product = left.compose(&right);

    assert_eq!(
        product,
        ts4_from_pairs(&[
            (Trace::new(vec![Block::new(2, 0, 0)]), 10),
            (Trace::new(vec![Block::new(1, 1, 0)]), 29),
            (Trace::new(vec![Block::new(0, 2, 0)]), 21),
        ])
    );
}

#[test]
fn ts4_compose_exact_output_uses_btree_path_for_large_pair_capacity() {
    let traces: Vec<_> = (1u32..=6)
        .map(|n| Trace::new(vec![Block::new(n, 0, 0)]))
        .collect();
    let left = TS4::from_trace(traces[0].clone(), 1)
        .add(&TS4::from_trace(traces[1].clone(), 1))
        .add(&TS4::from_trace(traces[2].clone(), 1))
        .add(&TS4::from_trace(traces[3].clone(), 1))
        .add(&TS4::from_trace(traces[4].clone(), 1))
        .add(&TS4::from_trace(traces[5].clone(), 1));
    let right = TS4::from_trace(traces[0].clone(), 1)
        .add(&TS4::from_trace(traces[1].clone(), 1))
        .add(&TS4::from_trace(traces[2].clone(), 1))
        .add(&TS4::from_trace(traces[3].clone(), 1))
        .add(&TS4::from_trace(traces[4].clone(), 1))
        .add(&TS4::from_trace(traces[5].clone(), 1));

    let product = left.compose(&right);
    assert_eq!(
        product,
        ts4_from_pairs(&[
            (Trace::new(vec![Block::new(2, 0, 0)]), 1),
            (Trace::new(vec![Block::new(3, 0, 0)]), 2),
            (Trace::new(vec![Block::new(4, 0, 0)]), 3),
            (Trace::new(vec![Block::new(5, 0, 0)]), 4),
            (Trace::new(vec![Block::new(6, 0, 0)]), 5),
            (Trace::new(vec![Block::new(7, 0, 0)]), 6),
            (Trace::new(vec![Block::new(8, 0, 0)]), 5),
            (Trace::new(vec![Block::new(9, 0, 0)]), 4),
            (Trace::new(vec![Block::new(10, 0, 0)]), 3),
            (Trace::new(vec![Block::new(11, 0, 0)]), 2),
            (Trace::new(vec![Block::new(12, 0, 0)]), 1),
        ])
    );
}

#[test]
fn odot_kappa_expands() {
    let u = Trace::new(vec![Block::new(3, 0, 0)]);
    let v = Trace::new(vec![Block::new(2, 0, 0)]);
    let r = odot_kappa(&u, &v, 4);
    assert_eq!(
        trace_into_blocks(r),
        vec![Block::new(4, 0, 0), Block::new(1, 0, 0)]
    );
}

#[test]
fn left_divide_trace_ok() {
    let a = Trace::new(vec![Block::new(1, 0, 0), Block::zero()]);
    let b = Trace::new(vec![Block::new(1, 0, 0), Block::new(0, 1, 0)]);
    let c = left_divide_trace(&a, &b).unwrap();
    assert_eq!(trace_into_blocks(c), vec![Block::new(0, 1, 0)]);
}

#[test]
fn right_divide_trace_ok() {
    let a = Trace::new(vec![Block::new(0, 1, 0)]);
    let b = Trace::new(vec![Block::new(1, 0, 0), Block::new(0, 1, 0)]);
    let c = right_divide_trace(&a, &b).unwrap();
    assert_eq!(
        trace_into_blocks(c),
        vec![Block::new(1, 0, 0), Block::zero()]
    );
}

#[test]
fn left_divide_trace_single_block_non_divisible_returns_none() {
    let a = Trace::new(vec![Block::new(2, 0, 0)]);
    let b = Trace::new(vec![Block::new(1, 0, 0)]);
    assert_eq!(left_divide_trace(&a, &b), None);
}

#[test]
fn left_divide_trace_prefix_mismatch_returns_none() {
    let a = Trace::new(vec![Block::new(1, 0, 0), Block::zero()]);
    let b = Trace::new(vec![Block::new(0, 1, 0), Block::zero()]);
    assert_eq!(left_divide_trace(&a, &b), None);
}

#[test]
fn right_divide_trace_single_block_non_divisible_returns_none() {
    let a = Trace::new(vec![Block::new(0, 2, 0)]);
    let b = Trace::new(vec![Block::new(0, 1, 0)]);
    assert_eq!(right_divide_trace(&a, &b), None);
}

#[test]
fn right_divide_trace_suffix_mismatch_returns_none() {
    let a = Trace::new(vec![Block::new(0, 1, 0), Block::new(0, 0, 1)]);
    let b = Trace::new(vec![
        Block::new(1, 0, 0),
        Block::new(0, 1, 0),
        Block::new(0, 1, 0),
    ]);
    assert_eq!(right_divide_trace(&a, &b), None);
}

#[test]
fn left_divide_trace_prefix_mismatch_at_chunk_boundary_returns_none() {
    let divisor = Trace::new(vec![
        Block::new(1, 0, 0),
        Block::new(0, 1, 0),
        Block::new(0, 0, 1),
        Block::new(1, 1, 0),
        Block::new(1, 0, 1),
        Block::new(0, 1, 1),
        Block::new(1, 1, 1),
        Block::new(2, 0, 0),
        Block::new(1, 0, 0),
    ]);
    let dividend = Trace::new(vec![
        Block::new(1, 0, 0),
        Block::new(0, 1, 0),
        Block::new(0, 0, 1),
        Block::new(1, 1, 0),
        Block::new(1, 0, 1),
        Block::new(0, 1, 1),
        Block::new(1, 1, 1),
        Block::new(2, 0, 0),
        Block::zero(),
        Block::new(0, 0, 1),
    ]);

    let _ = divisor.packed_chunks();
    let _ = dividend.packed_chunks();
    assert_eq!(left_divide_trace(&divisor, &dividend), None);
}

#[test]
fn right_divide_trace_suffix_mismatch_at_chunk_boundary_returns_none() {
    let divisor = Trace::new(vec![
        Block::new(0, 1, 0),
        Block::new(0, 0, 1),
        Block::new(0, 0, 1),
        Block::new(0, 0, 1),
        Block::new(0, 0, 1),
        Block::new(0, 0, 1),
        Block::new(0, 0, 1),
        Block::new(0, 0, 1),
        Block::new(0, 0, 1),
    ]);
    let dividend = Trace::new(vec![
        Block::new(1, 0, 0),
        Block::new(0, 1, 0),
        Block::new(0, 0, 1),
        Block::new(1, 1, 0),
        Block::new(1, 0, 1),
        Block::new(0, 1, 1),
        Block::new(1, 1, 1),
        Block::new(2, 0, 0),
        Block::new(9, 0, 0),
        Block::new(0, 1, 0),
        Block::new(0, 0, 1),
        Block::new(0, 0, 1),
        Block::new(0, 0, 1),
        Block::new(0, 0, 1),
        Block::new(0, 0, 1),
        Block::new(0, 0, 1),
        Block::new(0, 0, 1),
        Block::new(0, 0, 2),
    ]);

    let _ = divisor.packed_chunks();
    let _ = dividend.packed_chunks();
    assert_eq!(right_divide_trace(&divisor, &dividend), None);
}

#[test]
fn monomial_divide_ok() {
    let t1 = Trace::new(vec![Block::new(1, 0, 0)]);
    let t2 = Trace::new(vec![Block::new(1, 0, 0), Block::new(0, 1, 0)]);
    let (c, t) = left_divide_monomial(3, &t1, 12, &t2).unwrap();
    assert_eq!(c, 4);
    assert_eq!(
        trace_into_blocks(t),
        vec![Block::zero(), Block::new(0, 1, 0)]
    );
    let (c2, t2r) =
        right_divide_monomial(3, &Trace::new(vec![Block::new(0, 1, 0)]), 12, &t2).unwrap();
    assert_eq!(c2, 4);
    assert_eq!(
        trace_into_blocks(t2r),
        vec![Block::new(1, 0, 0), Block::zero()]
    );
}

#[test]
fn monomial_divide_exact_output_across_multiple_terms() {
    let x = Trace::new(vec![Block::new(1, 0, 0)]);
    let y = Trace::new(vec![Block::new(0, 1, 0)]);
    let left_dividend = Trace::new(vec![Block::new(1, 0, 0), Block::new(0, 1, 0)]);
    let right_dividend = Trace::new(vec![Block::new(1, 0, 0), Block::new(0, 0, 1)]);
    let divisor = TS4::from_trace(x.clone(), 2);
    let dividend =
        TS4::from_trace(left_dividend.clone(), 8).add(&TS4::from_trace(right_dividend.clone(), 10));

    let quotient = left_divide_ts4_monomial(&divisor, &dividend).unwrap();
    assert_eq!(
        quotient,
        ts4_from_pairs(&[
            (Trace::new(vec![Block::zero(), Block::new(0, 1, 0)]), 4),
            (Trace::new(vec![Block::zero(), Block::new(0, 0, 1)]), 5),
        ])
    );

    let right_divisor = TS4::from_trace(y.clone(), 2);
    let right_dividend_left = Trace::new(vec![Block::new(1, 0, 0), Block::new(0, 1, 0)]);
    let right_dividend_right = Trace::new(vec![Block::new(0, 0, 1), Block::new(0, 1, 0)]);
    let right_dividend = TS4::from_trace(right_dividend_left.clone(), 8)
        .add(&TS4::from_trace(right_dividend_right.clone(), 10));
    let right_quotient = right_divide_ts4_monomial(&right_divisor, &right_dividend).unwrap();
    assert_eq!(
        right_quotient,
        ts4_from_pairs(&[
            (Trace::new(vec![Block::new(1, 0, 0), Block::zero()]), 4),
            (Trace::new(vec![Block::new(0, 0, 1), Block::zero()]), 5),
        ])
    );
}

#[test]
fn monomial_divide_rejects_non_divisible_coefficients() {
    let divisor = TS4::from_trace(Trace::new(vec![Block::new(1, 0, 0)]), 4);
    let dividend = TS4::from_trace(
        Trace::new(vec![Block::new(1, 0, 0), Block::new(0, 1, 0)]),
        10,
    );
    assert_eq!(left_divide_ts4_monomial(&divisor, &dividend), None);
}

#[test]
fn monomial_divide_rejects_zero_coefficients() {
    let trace = Trace::new(vec![Block::new(1, 0, 0)]);
    assert_eq!(left_divide_monomial(0, &trace, 4, &trace), None);
    assert_eq!(left_divide_monomial(2, &trace, 0, &trace), None);
    assert_eq!(right_divide_monomial(0, &trace, 4, &trace), None);
    assert_eq!(right_divide_monomial(2, &trace, 0, &trace), None);
}

#[test]
fn ts4_normalize_removes_zero() {
    let t = Trace::new(vec![Block::new(1, 0, 0)]);
    let mut ts = TS4::from_trace(t, 0);
    ts.normalize();
    assert_eq!(ts.term_count(), 0);
}

#[test]
fn ts4_add_and_compose_ignore_zero_coefficient_garbage_terms() {
    let epsilon = Trace::empty();
    let x = Trace::new(vec![Block::new(1, 0, 0)]);
    let y = Trace::new(vec![Block::new(0, 1, 0)]);

    // Crate-private construction paths can still carry explicit zero-coefficient
    // terms. The supported contract is that algebraic ops treat those as absent
    // without requiring a prior `normalize()`.
    let mut a_terms = BTreeMap::new();
    a_terms.insert(x.clone(), 0);
    a_terms.insert(y.clone(), 2);
    let a = TS4::from_raw_terms_unchecked(a_terms);

    let mut b_terms = BTreeMap::new();
    b_terms.insert(epsilon, 0);
    b_terms.insert(x.clone(), 3);
    b_terms.insert(y.clone(), 0);
    let b = TS4::from_raw_terms_unchecked(b_terms);

    let sum = a.add(&b);
    assert_eq!(sum, ts4_from_pairs(&[(x.clone(), 3), (y.clone(), 2)]));
    assert_eq!(sum.term_count(), 2);

    let product = a.compose(&b);
    assert_eq!(product, TS4::from_trace(y.compose(&x), 6));
    assert_eq!(product.term_count(), 1);

    let mut only_zero_terms = BTreeMap::new();
    only_zero_terms.insert(x, 0);
    let only_zero = TS4::from_raw_terms_unchecked(only_zero_terms);
    assert_eq!(only_zero.add(&a), TS4::from_trace(y.clone(), 2));
    assert_eq!(only_zero.compose(&b), TS4::zero());
    assert_eq!(a.compose(&only_zero), TS4::zero());
}

#[test]
fn ts4_accessor_api_hides_zero_garbage_terms() {
    let x = Trace::new(vec![Block::new(1, 0, 0)]);
    let y = Trace::new(vec![Block::new(0, 1, 0)]);

    let mut terms = BTreeMap::new();
    terms.insert(x.clone(), 0);
    terms.insert(y.clone(), 2);
    let poly = TS4::from_raw_terms_unchecked(terms);

    assert_eq!(poly.term_count(), 1);
    assert_eq!(poly.coeff_sum(), 2);
    assert_eq!(poly.get_coeff(&x), None);
    assert_eq!(poly.get_coeff(&y), Some(2));
    assert_eq!(poly.iter().collect::<Vec<_>>(), vec![(&y, 2)]);
}

#[test]
#[should_panic(expected = "TS4::add coefficient overflow")]
fn ts4_add_overflow_panics() {
    let t = Trace::new(vec![Block::new(1, 0, 0)]);
    let a = TS4::from_trace(t.clone(), u32::MAX);
    let b = TS4::from_trace(t, 1);
    let _ = a.add(&b);
}

#[test]
#[should_panic(expected = "TS4::compose coefficient overflow")]
fn ts4_compose_overflow_panics() {
    let t = Trace::new(vec![Block::new(1, 0, 0)]);
    let a = TS4::from_trace(t.clone(), u32::MAX);
    let b = TS4::from_trace(t, 2);
    let _ = a.compose(&b);
}

#[test]
fn trace_from_word() {
    let t = Trace::from_word("txty");
    assert_eq!(trace_as_blocks(&t).len(), 3);
    assert_eq!(trace_as_blocks(&t)[0], Block::zero());
    assert_eq!(trace_as_blocks(&t)[1], Block::new(1, 0, 0));
    assert_eq!(trace_as_blocks(&t)[2], Block::new(0, 1, 0));

    let repeated_t = Trace::from_word("ttxyz");
    assert_eq!(
        trace_into_blocks(repeated_t),
        vec![Block::zero(), Block::zero(), Block::new(1, 1, 1)]
    );
}

#[test]
fn trace_tau_builds_canonical_pure_temporal_trace() {
    let trace = Trace::tau(4);
    assert_eq!(
        trace_into_blocks(trace),
        vec![
            Block::zero(),
            Block::zero(),
            Block::zero(),
            Block::zero(),
            Block::zero(),
        ]
    );
}

#[test]
fn trace_tau_matches_temporal_word_embedding() {
    let tau_trace = Trace::tau(3);
    let word_trace = Trace::from_word("ttt");
    assert_eq!(tau_trace, word_trace);
    assert_eq!(tau_trace.tau_count(), 3);
}

#[test]
fn trace_tau_zero_matches_empty_trace() {
    assert_eq!(Trace::tau(0), Trace::empty());
}

#[test]
#[should_panic(expected = "min_layers_for_mass requires kappa >= 1")]
fn min_layers_for_mass_zero_kappa_panics() {
    let _ = min_layers_for_mass(10, 0);
}

#[test]
#[should_panic(expected = "min_tau_for_mass requires kappa >= 1")]
fn min_tau_for_mass_zero_kappa_panics() {
    let _ = min_tau_for_mass(10, 0);
}

#[test]
fn sync_trace_zero_builds_canonical_zero_object() {
    let zero = SyncTrace::zero(4, 3);
    assert_eq!(zero.layers(), 4);
    assert_eq!(zero.kappa(), 3);
    assert_eq!(trace_as_blocks(zero.trace()), &[Block::zero(), Block::zero(), Block::zero(), Block::zero()]);
}

#[test]
fn sync_trace_new_rejects_non_admissible_trace() {
    let trace = Trace::new(vec![Block::new(5, 0, 0)]);
    assert!(SyncTrace::new(trace.clone(), 4).is_none());
    assert!(!is_kappa_admissible(&trace, 4));
}

#[test]
fn sync_trace_boxplus_matches_parallel_kappa_on_same_grid() {
    let lhs = SyncTrace::new(
        Trace::new(vec![Block::new(3, 0, 0), Block::new(1, 0, 0)]),
        4,
    )
    .unwrap();
    let rhs = SyncTrace::new(
        Trace::new(vec![Block::new(2, 0, 0), Block::new(0, 1, 0)]),
        4,
    )
    .unwrap();

    let actual = lhs.boxplus(&rhs).into_trace();
    let expected = parallel_kappa(lhs.trace(), rhs.trace(), 4);
    assert_eq!(actual, expected);
}

#[test]
fn sync_trace_boxplus_tight_is_direct_layer_sum_without_split() {
    let lhs = SyncTrace::new(
        Trace::new(vec![Block::new(1, 1, 0), Block::new(0, 1, 0)]),
        4,
    )
    .unwrap();
    let rhs = SyncTrace::new(
        Trace::new(vec![Block::new(1, 0, 1), Block::new(1, 0, 0)]),
        4,
    )
    .unwrap();

    assert!(lhs.is_tight_core());
    assert!(rhs.is_tight_core());
    assert!(is_tight_core(lhs.trace(), 4));

    let actual = lhs.boxplus_tight(&rhs).into_trace();
    let expected = Trace::new(vec![Block::new(2, 1, 1), Block::new(1, 1, 0)]);
    assert_eq!(actual, expected);
}

#[test]
fn sync_trace_boxplus_tight_matches_expected_across_chunk_boundary() {
    let lhs = SyncTrace::new(
        Trace::new(vec![
            Block::new(1, 0, 0),
            Block::new(0, 1, 0),
            Block::new(0, 0, 1),
            Block::new(1, 1, 0),
            Block::new(1, 0, 1),
            Block::new(0, 1, 1),
            Block::new(1, 1, 1),
            Block::new(2, 0, 0),
            Block::new(0, 2, 0),
            Block::new(0, 0, 2),
        ]),
        6,
    )
    .unwrap();
    let rhs = SyncTrace::new(
        Trace::new(vec![
            Block::new(0, 1, 0),
            Block::new(1, 0, 0),
            Block::new(0, 0, 1),
            Block::new(0, 1, 1),
            Block::new(1, 0, 0),
            Block::new(0, 1, 0),
            Block::new(1, 0, 0),
            Block::new(0, 1, 0),
            Block::new(1, 0, 1),
            Block::new(1, 0, 0),
        ]),
        6,
    )
    .unwrap();

    let actual = lhs.boxplus_tight(&rhs).into_trace();
    let expected = Trace::new(vec![
        Block::new(1, 1, 0),
        Block::new(1, 1, 0),
        Block::new(0, 0, 2),
        Block::new(1, 2, 1),
        Block::new(2, 0, 1),
        Block::new(0, 2, 1),
        Block::new(2, 1, 1),
        Block::new(2, 1, 0),
        Block::new(1, 2, 1),
        Block::new(1, 0, 2),
    ]);
    assert_eq!(actual, expected);
}

#[test]
fn physical_predicates_match_before_and_after_materialization() {
    let trace = Trace::new(vec![
        Block::new(1, 0, 0),
        Block::new(0, 1, 0),
        Block::new(0, 0, 1),
        Block::new(1, 1, 0),
        Block::new(1, 0, 1),
        Block::new(0, 1, 1),
        Block::new(1, 1, 1),
        Block::new(2, 0, 0),
        Block::new(0, 2, 0),
        Block::new(0, 0, 2),
    ]);

    let admissible_before = is_kappa_admissible(&trace, 3);
    let tight_before = is_tight_core(&trace, 6);
    let _ = trace.as_blocks();
    let admissible_after = is_kappa_admissible(&trace, 3);
    let tight_after = is_tight_core(&trace, 6);

    assert_eq!(admissible_before, admissible_after);
    assert_eq!(tight_before, tight_after);
    assert!(admissible_after);
    assert!(tight_after);
}

#[test]
fn sync_trace_boxplus_tight_preserves_result_after_materialization() {
    let lhs = SyncTrace::new(
        Trace::new(vec![
            Block::new(1, 0, 0),
            Block::new(0, 1, 0),
            Block::new(0, 0, 1),
            Block::new(1, 1, 0),
            Block::new(1, 0, 1),
            Block::new(0, 1, 1),
            Block::new(1, 1, 1),
            Block::new(2, 0, 0),
            Block::new(0, 2, 0),
        ]),
        6,
    )
    .unwrap();
    let rhs = SyncTrace::new(
        Trace::new(vec![
            Block::new(0, 1, 0),
            Block::new(1, 0, 0),
            Block::new(0, 0, 1),
            Block::new(0, 1, 1),
            Block::new(1, 0, 0),
            Block::new(0, 1, 0),
            Block::new(1, 0, 0),
            Block::new(0, 1, 0),
            Block::new(1, 0, 1),
        ]),
        6,
    )
    .unwrap();

    let expected = lhs.boxplus_tight(&rhs).into_trace();
    let _ = lhs.trace().as_blocks();
    let _ = rhs.trace().as_blocks();
    let actual = lhs.boxplus_tight(&rhs).into_trace();
    assert_eq!(actual, expected);
}

#[test]
fn sync_trace_sequential_matches_odot_kappa() {
    let lhs = SyncTrace::new(
        Trace::new(vec![Block::new(1, 0, 0), Block::new(1, 0, 0)]),
        4,
    )
    .unwrap();
    let rhs = SyncTrace::new(
        Trace::new(vec![Block::new(0, 1, 0), Block::zero()]),
        4,
    )
    .unwrap();

    let actual = lhs.sequential(&rhs).into_trace();
    let expected = odot_kappa(lhs.trace(), rhs.trace(), 4);
    assert_eq!(actual, expected);
}

#[test]
fn sync_trace_time_refine_matches_otimes_kappa() {
    let lhs = SyncTrace::new(
        Trace::new(vec![Block::new(1, 0, 0), Block::zero()]),
        4,
    )
    .unwrap();
    let rhs = SyncTrace::new(
        Trace::new(vec![Block::new(0, 1, 0)]),
        4,
    )
    .unwrap();

    let actual = lhs.time_refine(&rhs).into_trace();
    let expected = otimes_kappa(lhs.trace(), rhs.trace(), 4);
    assert_eq!(actual, expected);
}

#[test]
fn sync_trace_successors_match_physical_single_step_semantics() {
    let base = SyncTrace::new(
        Trace::new(vec![Block::new(1, 0, 0), Block::new(0, 1, 0)]),
        4,
    )
    .unwrap();

    assert_eq!(
        base.successor_t().into_trace(),
        odot_kappa(base.trace(), &Trace::tau(1), 4)
    );
    assert_eq!(
        base.successor_x().into_trace(),
        odot_kappa(base.trace(), &Trace::new(vec![Block::new(1, 0, 0)]), 4)
    );
    assert_eq!(
        base.successor_y().into_trace(),
        odot_kappa(base.trace(), &Trace::new(vec![Block::new(0, 1, 0)]), 4)
    );
    assert_eq!(
        base.successor_z().into_trace(),
        odot_kappa(base.trace(), &Trace::new(vec![Block::new(0, 0, 1)]), 4)
    );
}

#[test]
fn theory_normalize_trace_preserves_canonical_form() {
    let trace = Trace::new(vec![Block::new(1, 2, 0), Block::zero(), Block::new(0, 0, 1)]);
    assert_eq!(normalize_trace(&trace), trace);
}

#[test]
fn theory_normalize_ts4_removes_zero_garbage_terms() {
    let trace = Trace::new(vec![Block::new(1, 0, 0)]);
    let mut terms = BTreeMap::new();
    terms.insert(trace.clone(), 0);
    terms.insert(Trace::empty(), 1);
    let value = TS4::from_raw_terms_unchecked(terms);
    let normalized = normalize_ts4(&value);
    assert_eq!(normalized.term_count(), 1);
    assert_eq!(normalized.get_coeff(&Trace::empty()), Some(1));
    assert_eq!(normalized.get_coeff(&trace), None);
}

#[test]
fn theory_equal_by_normal_form_matches_native_equality() {
    let lhs = Trace::new(vec![Block::new(1, 1, 0), Block::zero()]);
    let rhs = Trace::from_word("xyt");
    assert!(traces_equal_by_normal_form(&lhs, &rhs));

    let a = TS4::from_trace(lhs.clone(), 2);
    let b = TS4::from_trace(rhs, 2);
    assert!(ts4_equal_by_normal_form(&a, &b));
}

#[test]
fn theory_semiring_laws_hold_on_concrete_triple() {
    let a = TS4::from_trace(Trace::new(vec![Block::new(1, 0, 0)]), 2);
    let b = TS4::from_trace(Trace::new(vec![Block::new(0, 1, 0)]), 3);
    let c = TS4::from_trace(Trace::tau(1), 1);
    assert!(ts4_semiring_laws_hold(&a, &b, &c));
}

#[test]
fn theory_noncommutative_example_matches_concept() {
    assert!(ts4_noncommutative_example());
}

#[test]
fn theory_trace_left_cancellation_matches_concept() {
    let prefix = Trace::new(vec![Block::new(1, 0, 0), Block::zero()]);
    let lhs = Trace::new(vec![Block::new(0, 1, 0)]);
    let rhs = Trace::new(vec![Block::new(0, 1, 0)]);
    assert!(trace_left_cancellation_holds(&prefix, &lhs, &rhs));
}

#[test]
fn theory_trace_atom_classification_matches_generators() {
    assert!(is_trace_atom(&Trace::tau(1)));
    assert!(is_trace_atom(&Trace::new(vec![Block::new(1, 0, 0)])));
    assert!(!is_trace_atom(&Trace::new(vec![Block::new(2, 0, 0)])));
    assert!(!is_trace_atom(&Trace::new(vec![Block::new(1, 0, 0), Block::zero()])));
}

#[test]
fn theory_monomial_left_divides_matches_existing_divisibility() {
    let lhs = Trace::new(vec![Block::new(1, 0, 0)]);
    let rhs = Trace::new(vec![Block::new(1, 0, 0), Block::new(0, 1, 0)]);
    assert!(monomial_left_divides(2, &lhs, 6, &rhs));
    assert!(left_divide_trace_is_exact(&lhs, &rhs));
}

#[test]
fn theory_projection_morphism_holds_for_compose() {
    let lhs = Trace::new(vec![Block::new(1, 0, 0), Block::zero()]);
    let rhs = Trace::new(vec![Block::new(0, 2, 1)]);
    assert!(pi_trace_compose_morphism_holds(&lhs, &rhs));
}

#[test]
fn theory_scale_projection_matches_scaled_pi() {
    let trace = Trace::new(vec![Block::new(1, 2, 3), Block::zero()]);
    assert!(proj_r4_matches_scaled_pi(&trace, 0.5, 2.0));
}

#[test]
fn trace_construction_and_padding_are_canonical() {
    let empty_from_new = Trace::new(Vec::new());
    let epsilon = Trace::empty();
    assert_eq!(empty_from_new, epsilon);
    assert_eq!(trace_as_blocks(&empty_from_new), &[Block::zero()]);
    assert_eq!(empty_from_new.len_blocks(), 1);
    assert_eq!(layers(&empty_from_new), 1);
    assert_eq!(tau_count(&empty_from_new), 0);

    let trace = Trace::new(vec![Block::new(1, 2, 3)]);
    assert_eq!(trace.pad_to(0), trace);
    assert_eq!(trace.pad_to(1), trace);

    let padded = trace.pad_to(4);
    assert_eq!(
        trace_as_blocks(&padded),
        vec![
            Block::new(1, 2, 3),
            Block::zero(),
            Block::zero(),
            Block::zero(),
        ]
    );
    assert_eq!(padded.len_blocks(), 4);
    assert_eq!(layers(&padded), 4);
    assert_eq!(tau_count(&padded), 3);
}

#[test]
#[should_panic(expected = "Trace::tau_count requires at least one block")]
fn trace_tau_count_rejects_empty_internal_state() {
    let t = trace_from_raw_blocks_unchecked(vec![]);
    let _ = t.tau_count();
}

#[test]
#[should_panic(expected = "Trace::compose requires at least one block")]
fn trace_compose_rejects_empty_internal_state() {
    let invalid = trace_from_raw_blocks_unchecked(vec![]);
    let valid = Trace::new(vec![Block::new(1, 0, 0)]);
    let _ = invalid.compose(&valid);
}

#[test]
#[should_panic(expected = "left_divide_trace requires at least one block")]
fn left_divide_trace_rejects_empty_internal_state() {
    let invalid = trace_from_raw_blocks_unchecked(vec![]);
    let valid = Trace::new(vec![Block::new(1, 0, 0)]);
    let _ = left_divide_trace(&invalid, &valid);
}

#[test]
#[should_panic(expected = "right_divide_trace requires at least one block")]
fn right_divide_trace_rejects_empty_internal_state() {
    let invalid = trace_from_raw_blocks_unchecked(vec![]);
    let valid = Trace::new(vec![Block::new(1, 0, 0)]);
    let _ = right_divide_trace(&invalid, &valid);
}

#[test]
#[should_panic(expected = "left_gcd_trace requires at least one block")]
fn left_gcd_trace_rejects_empty_internal_state() {
    let invalid = trace_from_raw_blocks_unchecked(vec![]);
    let valid = Trace::new(vec![Block::new(1, 0, 0)]);
    let _ = crate::divisibility::left_gcd_trace(&invalid, &valid);
}

#[test]
fn ts4_monomial_divide() {
    let t1 = Trace::new(vec![Block::new(1, 0, 0)]);
    let t2 = Trace::new(vec![Block::new(1, 0, 0), Block::new(0, 1, 0)]);
    let a = TS4::from_trace(t1.clone(), 2);
    let b = TS4::from_trace(t2.clone(), 4);
    let c = left_divide_ts4_monomial(&a, &b).unwrap();
    assert_eq!(c.term_count(), 1);
    let t = left_divide_trace(&t1, &t2).unwrap();
    assert_eq!(c.get_coeff(&t), Some(2));

    let a2 = TS4::from_trace(Trace::new(vec![Block::new(0, 1, 0)]), 2);
    let c2 = right_divide_ts4_monomial(&a2, &b).unwrap();
    let t_right = right_divide_trace(&Trace::new(vec![Block::new(0, 1, 0)]), &t2).unwrap();
    assert_eq!(c2.get_coeff(&t_right), Some(2));
}

#[test]
fn ts4_unique_divide() {
    let a = TS4::from_trace(Trace::new(vec![Block::new(1, 0, 0)]), 2);
    let b = TS4::from_trace(
        Trace::new(vec![Block::new(1, 0, 0), Block::new(0, 1, 0)]),
        4,
    );
    let c = left_divide_ts4_unique(&a, &b).unwrap();
    assert_eq!(c.term_count(), 1);
}

#[test]
fn ts4_unique_divide_exact_output_and_ambiguity_rejection() {
    let x = Trace::new(vec![Block::new(1, 0, 0)]);
    let y = Trace::new(vec![Block::new(0, 1, 0)]);
    let xy = Trace::new(vec![Block::new(1, 0, 0), Block::new(0, 1, 0)]);
    let yz = Trace::new(vec![Block::new(0, 1, 0), Block::new(0, 0, 1)]);
    let divisor = TS4::from_trace(x.clone(), 2).add(&TS4::from_trace(y.clone(), 3));
    let dividend = TS4::from_trace(
        Trace::new(vec![Block::new(1, 0, 0), Block::new(0, 1, 0)]),
        8,
    )
    .add(&TS4::from_trace(yz.clone(), 9));
    let quotient = left_divide_ts4_unique(&divisor, &dividend).unwrap();

    assert_eq!(
        quotient,
        ts4_from_pairs(&[
            (Trace::new(vec![Block::zero(), Block::new(0, 1, 0)]), 4),
            (Trace::new(vec![Block::zero(), Block::new(0, 0, 1)]), 3),
        ])
    );

    let ambiguous_divisor = TS4::from_trace(x.clone(), 1).add(&TS4::from_trace(xy.clone(), 1));
    let ambiguous_dividend = TS4::from_trace(
        Trace::new(vec![
            Block::new(1, 0, 0),
            Block::new(0, 1, 0),
            Block::new(0, 0, 1),
        ]),
        1,
    );
    assert_eq!(
        left_divide_ts4_unique(&ambiguous_divisor, &ambiguous_dividend),
        None
    );
}

#[test]
fn ts4_solve_divide() {
    let ax = TS4::from_trace(Trace::new(vec![Block::new(1, 0, 0)]), 1);
    let ay = TS4::from_trace(Trace::new(vec![Block::new(0, 1, 0)]), 1);
    let a = ax.add(&ay);
    let b = TS4::from_trace(Trace::new(vec![Block::new(1, 1, 0)]), 2);
    let c = left_divide_ts4_solve(&a, &b, 8, 1).unwrap();
    assert_eq!(
        c,
        ts4_from_pairs(&[(Trace::new(vec![Block::new(0, 1, 0)]), 2)])
    );
}

#[test]
fn ts4_solve_divide_exact_output_and_unbounded_wrapper() {
    let ax = TS4::from_trace(Trace::new(vec![Block::new(1, 0, 0)]), 1);
    let ay = TS4::from_trace(Trace::new(vec![Block::new(0, 1, 0)]), 1);
    let a = ax.add(&ay);
    let b = TS4::from_trace(
        Trace::new(vec![Block::new(1, 0, 0), Block::new(0, 1, 0)]),
        2,
    )
    .add(&TS4::from_trace(
        Trace::new(vec![Block::new(0, 1, 0), Block::new(0, 0, 1)]),
        3,
    ));
    let expected = ts4_from_pairs(&[
        (Trace::new(vec![Block::zero(), Block::new(0, 1, 0)]), 2),
        (Trace::new(vec![Block::zero(), Block::new(0, 0, 1)]), 3),
    ]);

    let c = left_divide_ts4_solve(&a, &b, 8, 4).unwrap();
    assert_eq!(c, expected);

    let wrapped = left_divide_ts4(&a, &b, SolveMode::Unbounded { max_solutions: 4 }).unwrap();
    assert_eq!(wrapped, expected);

    let bounded = left_divide_ts4(
        &a,
        &b,
        SolveMode::Bounded {
            max_vars: 8,
            max_solutions: 4,
        },
    )
    .unwrap();
    assert_eq!(bounded, expected);
}

#[test]
fn ts4_solve_divide_returns_none_on_unsatisfied_system() {
    let a = TS4::from_trace(Trace::new(vec![Block::new(1, 0, 0)]), 1)
        .add(&TS4::from_trace(Trace::new(vec![Block::new(0, 1, 0)]), 1));
    let b = TS4::from_trace(Trace::new(vec![Block::new(0, 0, 1)]), 1);
    assert_eq!(left_divide_ts4_solve(&a, &b, 8, 1), None);
}

#[test]
fn ts4_solve_divide_rejects_incompatible_coefficients() {
    let x = Trace::new(vec![Block::new(1, 0, 0)]);
    let y = Trace::new(vec![Block::new(0, 1, 0)]);
    let divisor = TS4::from_trace(x.clone(), 4).add(&TS4::from_trace(y.clone(), 6));
    let dividend = TS4::from_trace(Trace::new(vec![Block::new(1, 0, 0), Block::zero()]), 10).add(
        &TS4::from_trace(Trace::new(vec![Block::new(0, 1, 0), Block::zero()]), 14),
    );

    assert_eq!(left_divide_ts4_solve(&divisor, &dividend, 8, 1), None);
}

#[test]
fn ts4_solve_divide_exact_output_and_limits_are_enforced() {
    let ax = TS4::from_trace(Trace::new(vec![Block::new(1, 0, 0)]), 1);
    let ay = TS4::from_trace(Trace::new(vec![Block::new(0, 1, 0)]), 1);
    let a = ax.add(&ay);
    let b = TS4::from_trace(
        Trace::new(vec![Block::new(1, 0, 0), Block::new(0, 1, 0)]),
        2,
    )
    .add(&TS4::from_trace(
        Trace::new(vec![Block::new(0, 1, 0), Block::new(0, 0, 1)]),
        3,
    ));
    let expected = ts4_from_pairs(&[
        (Trace::new(vec![Block::zero(), Block::new(0, 1, 0)]), 2),
        (Trace::new(vec![Block::zero(), Block::new(0, 0, 1)]), 3),
    ]);

    assert_eq!(left_divide_ts4_solve(&a, &b, 8, 4), Some(expected.clone()));
    assert_eq!(left_divide_ts4_solve(&a, &b, 1, 4), None);
    assert_eq!(
        left_divide_ts4(
            &a,
            &b,
            SolveMode::Bounded {
                max_vars: 1,
                max_solutions: 4,
            },
        ),
        None
    );
    assert_eq!(left_divide_ts4_solve(&a, &b, 8, 0), None);
    assert_eq!(left_divide_ts4_solve(&TS4::zero(), &b, 2, 1), None);
}

#[test]
fn gcd_helpers_handle_zero_inputs_exactly() {
    let a = Trace::new(vec![Block::new(2, 0, 0)]);
    let b = Trace::new(vec![Block::new(1, 0, 0), Block::new(0, 1, 0)]);
    assert_eq!(gcd_u32(0, 18), 18);
    assert_eq!(gcd_u32(84, 0), 84);
    assert_eq!(
        gcd_monomial(0, &a, 18, &b),
        (18, crate::divisibility::left_gcd_trace(&a, &b))
    );
}

#[test]
fn invariants_mass_and_bounds() {
    let t = Trace::new(vec![Block::new(2, 1, 0), Block::new(1, 0, 0)]);
    let m = mass_l1(&t);
    assert_eq!(m, 4);
    assert_eq!(min_layers_for_mass(m, 3), 2);
    assert_eq!(min_tau_for_mass(m, 3), 1);
}

#[test]
fn physical_ops_preserve_mass() {
    let u = Trace::new(vec![Block::new(2, 0, 0)]);
    let v = Trace::new(vec![Block::new(0, 1, 0), Block::new(1, 0, 0)]);
    let m = mass_l1(&u) + mass_l1(&v);
    let od = odot_kappa(&u, &v, 3);
    let pa = parallel_kappa(&u, &v, 3);
    assert_eq!(mass_l1(&od), m);
    assert_eq!(mass_l1(&pa), m);
}

#[test]
fn ts4_mod_basic() {
    let t = Trace::new(vec![Block::new(1, 0, 0)]);
    let a = TS4Mod::from_trace(t.clone(), 3, 5);
    let b = TS4Mod::from_trace(t.clone(), 4, 5);
    let c = a.add(&b);
    assert_eq!(c.get_coeff(&t), Some(2)); // 3+4=7 mod 5
}

#[test]
fn ts4_mod_add_and_compose_ignore_zero_coefficient_garbage_terms() {
    let modulus = 7;
    let epsilon = Trace::empty();
    let x = Trace::new(vec![Block::new(1, 0, 0)]);
    let y = Trace::new(vec![Block::new(0, 1, 0)]);

    let mut a_terms = BTreeMap::new();
    a_terms.insert(x.clone(), 0);
    a_terms.insert(y.clone(), 2);
    let a = TS4Mod::from_raw_terms_unchecked(modulus, a_terms);

    let mut b_terms = BTreeMap::new();
    b_terms.insert(epsilon, 0);
    b_terms.insert(x.clone(), 3);
    b_terms.insert(y.clone(), 0);
    let b = TS4Mod::from_raw_terms_unchecked(modulus, b_terms);

    let sum = a.add(&b);
    assert_eq!(
        sum,
        ts4mod_from_pairs(modulus, &[(x.clone(), 3), (y.clone(), 2)])
    );
    assert_eq!(sum.modulus(), modulus);
    assert_eq!(sum.term_count(), 2);

    let product = a.compose(&b);
    assert_eq!(product, TS4Mod::from_trace(y.compose(&x), 6, modulus));
    assert_eq!(product.modulus(), modulus);
    assert_eq!(product.term_count(), 1);

    let mut only_zero_terms = BTreeMap::new();
    only_zero_terms.insert(x, 0);
    let only_zero = TS4Mod::from_raw_terms_unchecked(modulus, only_zero_terms);
    assert_eq!(only_zero.add(&a), TS4Mod::from_trace(y.clone(), 2, modulus));
    assert_eq!(only_zero.compose(&b), TS4Mod::new(modulus));
    assert_eq!(a.compose(&only_zero), TS4Mod::new(modulus));
}

#[test]
fn ts4mod_accessor_api_hides_zero_garbage_terms() {
    let modulus = 7;
    let x = Trace::new(vec![Block::new(1, 0, 0)]);
    let y = Trace::new(vec![Block::new(0, 1, 0)]);

    let mut terms = BTreeMap::new();
    terms.insert(x.clone(), 0);
    terms.insert(y.clone(), 2);
    let poly = TS4Mod::from_raw_terms_unchecked(modulus, terms);

    assert_eq!(poly.modulus(), modulus);
    assert_eq!(poly.term_count(), 1);
    assert_eq!(poly.coeff_sum(), 2);
    assert_eq!(poly.get_coeff(&x), None);
    assert_eq!(poly.get_coeff(&y), Some(2));
    assert_eq!(poly.iter().collect::<Vec<_>>(), vec![(&y, 2)]);
}

#[test]
fn ts4_mod_add_uses_exact_large_modular_sum() {
    let modulus = u32::MAX;
    let t = Trace::new(vec![Block::new(1, 0, 0)]);
    let a = TS4Mod::from_trace(t.clone(), modulus - 1, modulus);
    let b = TS4Mod::from_trace(t.clone(), modulus - 1, modulus);
    let c = a.add(&b);
    assert_eq!(c.modulus(), modulus);
    assert_eq!(c.get_coeff(&t), Some(modulus - 2));
}

#[test]
fn ts4_mod_compose_removes_zero_terms_after_mod_reduction() {
    let modulus = 5;
    let epsilon = Trace::empty();
    let x = Trace::new(vec![Block::new(1, 0, 0)]);
    let a = TS4Mod::from_trace(epsilon.clone(), 2, modulus).add(&TS4Mod::from_trace(
        x.clone(),
        3,
        modulus,
    ));
    let b = TS4Mod::from_trace(x.clone(), 1, modulus).add(&TS4Mod::from_trace(
        epsilon.clone(),
        1,
        modulus,
    ));
    let c = a.compose(&b);
    assert_eq!(c.get_coeff(&x), None);
    assert_eq!(c.get_coeff(&epsilon), Some(2));
    assert_eq!(c.get_coeff(&x.compose(&x)), Some(3));
}

#[test]
#[should_panic(expected = "TS4Mod::new requires modulus >= 1")]
fn ts4_mod_new_rejects_zero_modulus() {
    let _ = TS4Mod::new(0);
}

#[test]
#[should_panic(expected = "TS4Mod::from_trace requires modulus >= 1")]
fn ts4_mod_from_trace_rejects_zero_modulus() {
    let _ = TS4Mod::from_trace(Trace::empty(), 1, 0);
}

#[test]
#[should_panic(expected = "TS4Mod::add requires identical moduli")]
fn ts4_mod_add_rejects_mismatched_moduli() {
    let t = Trace::new(vec![Block::new(1, 0, 0)]);
    let a = TS4Mod::from_trace(t.clone(), 1, 5);
    let b = TS4Mod::from_trace(t, 1, 7);
    let _ = a.add(&b);
}

#[test]
#[should_panic(expected = "TS4Mod::compose requires identical moduli")]
fn ts4_mod_compose_rejects_mismatched_moduli() {
    let t = Trace::new(vec![Block::new(1, 0, 0)]);
    let a = TS4Mod::from_trace(t.clone(), 1, 5);
    let b = TS4Mod::from_trace(t, 1, 7);
    let _ = a.compose(&b);
}

#[test]
fn gcd_helpers() {
    assert_eq!(gcd_u32(12, 8), 4);
    let t1 = Trace::new(vec![Block::new(2, 0, 0)]);
    let t2 = Trace::new(vec![Block::new(1, 0, 0)]);
    let (g, t) = gcd_monomial(6, &t1, 10, &t2);
    assert_eq!(g, 2);
    assert_eq!(trace_into_blocks(t), vec![Block::new(1, 0, 0)]);
}

#[test]
fn projections_basic() {
    let t = Trace::new(vec![Block::new(1, 2, 3), Block::zero()]);
    let (tt, xx, yy, zz) = pi_trace(&t);
    assert_eq!(tt, 1);
    assert_eq!(xx, 1);
    assert_eq!(yy, 2);
    assert_eq!(zz, 3);
    let a = TS4::from_trace(t.clone(), 2);
    let (t2, x2, y2, z2) = pi_ts4(&a);
    assert_eq!((t2, x2, y2, z2), (2, 2, 4, 6));
    let r = proj_r4(&t, 0.5, 2.0);
    assert_eq!(r, (0.5, 2.0, 4.0, 6.0));
}

#[test]
fn projections_exact_output_for_weighted_terms() {
    let left = Trace::new(vec![
        Block::new(2, 1, 0),
        Block::zero(),
        Block::new(0, 0, 4),
    ]);
    let right = Trace::new(vec![Block::new(1, 0, 1)]);
    let poly = TS4::from_trace(left.clone(), 3).add(&TS4::from_trace(right.clone(), 5));

    assert_eq!(pi_trace(&left), (2, 2, 1, 4));
    assert_eq!(pi_trace(&right), (0, 1, 0, 1));
    assert_eq!(pi_ts4(&poly), (6, 11, 3, 17));
    assert_eq!(proj_r4(&left, 0.25, 10.0), (0.5, 20.0, 10.0, 40.0));
}

#[test]
fn packed_cache_reductions_match_exact_mass_and_projection() {
    let trace = Trace::new(vec![
        Block::new(5, 0, 0),
        Block::new(0, 4, 1),
        Block::new(2, 1, 2),
        Block::zero(),
        Block::new(1, 3, 1),
        Block::new(4, 0, 2),
        Block::new(0, 2, 0),
        Block::new(0, 0, 3),
        Block::new(6, 1, 0),
    ]);
    let _ = trace.packed_chunks();

    assert_eq!(mass_l1(&trace), scalar_sum_l1(trace.as_blocks()));
    assert_eq!(pi_trace(&trace), (8, 18, 11, 9));
    assert_eq!(proj_r4(&trace, 0.25, 2.0), (2.0, 36.0, 22.0, 18.0));
}

#[test]
#[should_panic(expected = "pi_trace x overflow")]
fn pi_trace_panics_on_projection_overflow_after_packed_cache_materialization() {
    let trace = Trace::new(vec![Block::new(u32::MAX, 0, 0), Block::new(1, 0, 0)]);
    let _ = trace.packed_chunks();
    let _ = pi_trace(&trace);
}

#[test]
fn simd_mask_basic() {
    let blocks = vec![Block::new(1, 1, 1), Block::new(5, 0, 0)];
    let mask = blocks_l1_gt_mask(&blocks, 3);
    assert_eq!(mask.into_bools(), vec![false, true]);
    assert_eq!(blocks_l1_gt(&blocks, 3), vec![false, true]);
}

#[test]
fn simd_mask_packs_bits_into_words() {
    let blocks = vec![Block::new(2, 0, 0); 70];
    let mask: BlockMask = blocks_l1_gt_mask(&blocks, 1);
    assert_eq!(mask.len(), 70);
    assert_eq!(mask.count_ones(), 70);
    assert_eq!(mask.as_words(), &[u64::MAX, 0x3f]);
}

#[test]
fn simd_exact_output_for_prefix_lengths_zero_through_nine() {
    let blocks = vec![
        Block::new(1, 0, 0),
        Block::new(2, 0, 0),
        Block::new(3, 0, 0),
        Block::new(4, 0, 0),
        Block::new(5, 0, 0),
        Block::new(6, 0, 0),
        Block::new(7, 0, 0),
        Block::new(8, 0, 0),
        Block::new(9, 0, 0),
        Block::new(10, 0, 0),
    ];

    let expected_sums = [0, 1, 3, 6, 10, 15, 21, 28, 36, 45];
    let expected_masks = [
        vec![],
        vec![false],
        vec![false, false],
        vec![false, false, false],
        vec![false, false, false, false],
        vec![false, false, false, false, true],
        vec![false, false, false, false, true, true],
        vec![false, false, false, false, true, true, true],
        vec![false, false, false, false, true, true, true, true],
        vec![false, false, false, false, true, true, true, true, true],
    ];

    for len in 0..=9 {
        let slice = &blocks[..len];
        assert_eq!(sum_l1_blocks(slice), expected_sums[len] as u64);
        assert_eq!(
            blocks_l1_gt_mask(slice, 4).into_bools(),
            expected_masks[len]
        );
    }
}

#[test]
fn simd_exact_output_for_tail_and_alignment_sensitive_slices() {
    let blocks = vec![
        Block::new(1, 0, 0),
        Block::new(0, 2, 0),
        Block::new(0, 0, 3),
        Block::new(4, 0, 0),
        Block::new(0, 5, 0),
        Block::new(0, 0, 6),
        Block::new(7, 0, 0),
        Block::new(0, 8, 0),
        Block::new(0, 0, 9),
        Block::new(10, 0, 0),
        Block::new(0, 11, 0),
        Block::new(0, 0, 12),
        Block::new(13, 0, 0),
        Block::new(0, 14, 0),
        Block::new(0, 0, 15),
        Block::new(16, 0, 0),
        Block::new(0, 17, 0),
        Block::new(0, 0, 18),
    ];

    assert_eq!(sum_l1_blocks(&blocks[1..9]), 44);
    assert_eq!(
        blocks_l1_gt_mask(&blocks[1..9], 5).into_bools(),
        vec![false, false, false, false, true, true, true, true]
    );

    assert_eq!(sum_l1_blocks(&blocks[2..10]), 52);
    assert_eq!(
        blocks_l1_gt_mask(&blocks[2..10], 7).into_bools(),
        vec![false, false, false, false, false, true, true, true]
    );

    assert_eq!(sum_l1_blocks(&blocks[3..18]), 165);
    assert_eq!(
        blocks_l1_gt_mask(&blocks[3..18], 12).into_bools(),
        vec![
            false, false, false, false, false, false, false, false, false, true, true, true, true,
            true, true,
        ]
    );

    assert_eq!(sum_l1_blocks(&blocks), 171);
    assert_eq!(
        blocks_l1_gt_mask(&blocks, 12).into_bools(),
        vec![
            false, false, false, false, false, false, false, false, false, false, false, false,
            true, true, true, true, true, true,
        ]
    );
}

#[test]
fn simd_equivalence_for_aligned_and_misaligned_long_windows() {
    let window = vec![
        Block::new(1, 0, 1),
        Block::new(0, 2, 1),
        Block::new(3, 0, 0),
        Block::new(0, 1, 2),
        Block::new(2, 1, 1),
        Block::new(1, 3, 0),
        Block::new(0, 0, 4),
        Block::new(2, 2, 0),
        Block::new(1, 0, 1),
        Block::new(0, 2, 1),
        Block::new(3, 0, 0),
        Block::new(0, 1, 2),
        Block::new(2, 1, 1),
        Block::new(1, 3, 0),
        Block::new(0, 0, 4),
        Block::new(2, 2, 0),
    ];
    let mut padded = vec![Block::new(9, 9, 9)];
    padded.extend_from_slice(&window);

    let aligned = window.as_slice();
    let misaligned = &padded[1..17];

    assert_eq!(sum_l1_blocks(aligned), scalar_sum_l1(aligned));
    assert_eq!(sum_l1_blocks(misaligned), scalar_sum_l1(misaligned));
    assert_eq!(
        blocks_l1_gt_mask(aligned, 4).into_bools(),
        scalar_blocks_l1_gt(aligned, 4)
    );
    assert_eq!(
        blocks_l1_gt_mask(misaligned, 4).into_bools(),
        scalar_blocks_l1_gt(misaligned, 4)
    );
    assert_eq!(sum_l1_blocks(aligned), sum_l1_blocks(misaligned));
    assert_eq!(
        blocks_l1_gt_mask(aligned, 4),
        blocks_l1_gt_mask(misaligned, 4)
    );
}

#[test]
fn simd_mask_compatibility_wrapper_matches_packed_mask_across_chunk_boundary() {
    let blocks = vec![
        Block::new(1, 0, 0),
        Block::new(2, 1, 0),
        Block::new(0, 0, 4),
        Block::new(3, 0, 0),
        Block::new(0, 2, 1),
        Block::new(4, 0, 0),
        Block::new(0, 0, 5),
        Block::new(1, 1, 1),
        Block::new(5, 0, 0),
        Block::new(0, 1, 2),
        Block::new(3, 3, 0),
        Block::new(0, 0, 6),
        Block::new(2, 2, 2),
        Block::new(1, 0, 3),
        Block::new(0, 4, 0),
        Block::new(4, 1, 0),
        Block::new(0, 0, 1),
    ];
    let expected = scalar_blocks_l1_gt(&blocks, 4);
    let mask = blocks_l1_gt_mask(&blocks, 4);

    assert_eq!(blocks_l1_gt(&blocks, 4), expected);
    assert_eq!(mask.clone().into_bools(), expected);
    assert_eq!(mask.len(), blocks.len());
    assert_eq!(mask.count_ones(), expected.iter().filter(|&&bit| bit).count());
}

#[test]
fn simd_mask_get_and_tail_bits_are_correct_at_sixty_four_boundary() {
    let mut blocks = vec![Block::new(1, 0, 0); 65];
    blocks[0] = Block::new(0, 0, 0);
    blocks[63] = Block::new(2, 0, 0);
    blocks[64] = Block::new(3, 0, 0);

    let mask = blocks_l1_gt_mask(&blocks, 1);
    assert_eq!(mask.len(), 65);
    assert_eq!(mask.count_ones(), 2);
    assert!(!mask.get(0));
    assert!(mask.get(63));
    assert!(mask.get(64));
    assert_eq!(mask.as_words(), &[1u64 << 63, 1]);
    assert_eq!(mask.clone().into_bools()[63], true);
    assert_eq!(mask.clone().into_bools()[64], true);
}

#[test]
fn phi_kappa_fast_path() {
    let t = Trace::new(vec![Block::new(1, 1, 0), Block::zero()]);
    let p = phi_kappa(&t, 3);
    assert_eq!(p, t);
}

#[test]
#[should_panic(expected = "phi_kappa requires kappa >= 1")]
fn phi_kappa_rejects_zero_kappa() {
    let t = Trace::new(vec![Block::new(1, 0, 0)]);
    let _ = phi_kappa(&t, 0);
}

#[test]
#[should_panic(expected = "BlockMask::get index out of bounds")]
fn simd_mask_get_panics_on_out_of_bounds_index() {
    let blocks = vec![Block::new(1, 0, 0); 65];
    let mask = blocks_l1_gt_mask(&blocks, 0);
    let _ = mask.get(65);
}

#[test]
#[should_panic(expected = "Block::add overflow")]
fn block_add_overflow_panics() {
    let _ = Block::new(u32::MAX, 0, 0).add(Block::new(1, 0, 0));
}

#[test]
#[should_panic(expected = "Block::sub underflow")]
fn block_sub_underflow_panics() {
    let _ = Block::new(0, 0, 0).sub(Block::new(1, 0, 0));
}

#[test]
#[should_panic(expected = "Block::l1 overflow")]
fn simd_sum_l1_panics_on_l1_overflow_in_vector_path() {
    let blocks = vec![Block::new(u32::MAX, 1, 0); 8];
    let _ = sum_l1_blocks(&blocks);
}

#[test]
#[should_panic(expected = "Block::l1 overflow")]
fn simd_mask_panics_on_l1_overflow_in_vector_path() {
    let blocks = vec![Block::new(u32::MAX, 1, 0); 8];
    let _ = blocks_l1_gt_mask(&blocks, 1);
}

#[test]
fn simd_sum_matches_scalar_on_full_chunk_length() {
    let blocks = vec![
        Block::new(1, 0, 0),
        Block::new(0, 2, 0),
        Block::new(0, 0, 3),
        Block::new(4, 1, 0),
        Block::new(0, 5, 1),
        Block::new(2, 2, 2),
        Block::new(3, 0, 1),
        Block::new(1, 1, 1),
    ];
    assert_simd_oracle(&blocks, 4);
}

#[test]
fn ts4_zero_and_one_are_exact_units() {
    let trace = Trace::new(vec![Block::new(1, 0, 0), Block::new(0, 1, 0)]);
    let poly = TS4::from_trace(trace.clone(), 4);

    assert_eq!(TS4::zero().add(&poly), poly);
    assert_eq!(poly.add(&TS4::zero()), poly);
    assert_eq!(TS4::one().compose(&poly), poly);
    assert_eq!(poly.compose(&TS4::one()), poly);
    assert_eq!(TS4::zero().compose(&poly), TS4::zero());
}

#[test]
fn trace_compose_exact_output_merges_multiple_boundaries() {
    let left = Trace::new(vec![
        Block::new(1, 2, 0),
        Block::new(0, 1, 3),
        Block::new(4, 0, 1),
    ]);
    let right = Trace::new(vec![Block::new(2, 0, 1), Block::new(0, 2, 2)]);
    let composed = left.compose(&right);

    assert_eq!(
        trace_into_blocks(composed),
        vec![
            Block::new(1, 2, 0),
            Block::new(0, 1, 3),
            Block::new(6, 0, 2),
            Block::new(0, 2, 2),
        ]
    );
}

#[test]
fn trace_compose_exact_output_after_packed_cache_materialization_crosses_chunk_boundary() {
    let left = Trace::new(vec![
        Block::new(1, 0, 0),
        Block::new(0, 1, 0),
        Block::new(0, 0, 1),
        Block::new(1, 1, 0),
        Block::new(1, 0, 1),
        Block::new(0, 1, 1),
        Block::new(1, 1, 1),
        Block::new(2, 0, 0),
        Block::new(1, 0, 0),
    ]);
    let right = Trace::new(vec![
        Block::new(0, 1, 0),
        Block::new(0, 0, 1),
        Block::new(0, 0, 1),
        Block::new(0, 0, 1),
        Block::new(0, 0, 1),
        Block::new(0, 0, 1),
        Block::new(0, 0, 1),
        Block::new(0, 0, 1),
        Block::new(0, 0, 1),
        Block::new(0, 0, 1),
    ]);
    let _ = left.packed_chunks();
    let _ = right.packed_chunks();

    let composed = left.compose(&right);
    assert_eq!(
        trace_into_blocks(composed),
        vec![
            Block::new(1, 0, 0),
            Block::new(0, 1, 0),
            Block::new(0, 0, 1),
            Block::new(1, 1, 0),
            Block::new(1, 0, 1),
            Block::new(0, 1, 1),
            Block::new(1, 1, 1),
            Block::new(2, 0, 0),
            Block::new(1, 1, 0),
            Block::new(0, 0, 1),
            Block::new(0, 0, 1),
            Block::new(0, 0, 1),
            Block::new(0, 0, 1),
            Block::new(0, 0, 1),
            Block::new(0, 0, 1),
            Block::new(0, 0, 1),
            Block::new(0, 0, 1),
            Block::new(0, 0, 1),
        ]
    );
}

#[test]
fn trace_compose_direct_packed_finalization_preserves_result_after_materialization() {
    let left = Trace::new(vec![
        Block::new(1, 0, 0),
        Block::new(0, 1, 0),
        Block::new(0, 0, 1),
        Block::new(1, 1, 0),
        Block::new(1, 0, 1),
        Block::new(0, 1, 1),
        Block::new(1, 1, 1),
        Block::new(2, 0, 0),
        Block::new(1, 0, 0),
    ]);
    let right = Trace::new(vec![
        Block::new(0, 1, 0),
        Block::new(0, 0, 1),
        Block::new(0, 0, 1),
        Block::new(0, 0, 1),
        Block::new(0, 0, 1),
        Block::new(0, 0, 1),
        Block::new(0, 0, 1),
        Block::new(0, 0, 1),
        Block::new(0, 0, 1),
        Block::new(0, 0, 1),
    ]);

    let composed = left.compose(&right);
    let _ = composed.as_blocks();

    assert_eq!(
        trace_into_blocks(composed),
        vec![
            Block::new(1, 0, 0),
            Block::new(0, 1, 0),
            Block::new(0, 0, 1),
            Block::new(1, 1, 0),
            Block::new(1, 0, 1),
            Block::new(0, 1, 1),
            Block::new(1, 1, 1),
            Block::new(2, 0, 0),
            Block::new(1, 1, 0),
            Block::new(0, 0, 1),
            Block::new(0, 0, 1),
            Block::new(0, 0, 1),
            Block::new(0, 0, 1),
            Block::new(0, 0, 1),
            Block::new(0, 0, 1),
            Block::new(0, 0, 1),
            Block::new(0, 0, 1),
            Block::new(0, 0, 1),
        ]
    );
}

#[test]
fn trace_compose_direct_packed_finalization_zero_pads_tail_lanes() {
    let left = Trace::new(vec![Block::new(1, 0, 0); 9]);
    let right = Trace::new(vec![Block::new(0, 1, 0); 10]);
    let composed = left.compose(&right);
    let last_chunk_index = composed.packed_chunk_count() - 1;
    let valid_lanes = composed.packed_chunk_valid_lanes(last_chunk_index);
    let last_chunk = composed.packed_chunk(last_chunk_index);

    for lane in valid_lanes..crate::simd::LANES_256 {
        assert_eq!(last_chunk.x[lane], 0);
        assert_eq!(last_chunk.y[lane], 0);
        assert_eq!(last_chunk.z[lane], 0);
    }
}

#[test]
fn trace_compose_identity_with_empty_trace_preserves_both_sides() {
    let trace = Trace::new(vec![
        Block::new(1, 0, 0),
        Block::new(0, 2, 0),
        Block::new(0, 0, 3),
        Block::new(4, 0, 1),
        Block::new(0, 1, 4),
        Block::new(2, 2, 0),
        Block::new(0, 0, 5),
        Block::new(3, 1, 0),
        Block::new(1, 0, 0),
    ]);
    let epsilon = Trace::empty();

    assert_eq!(epsilon.compose(&trace), trace);
    assert_eq!(trace.compose(&epsilon), trace);
}

#[test]
fn trace_packed_chunks_preserve_order_and_zero_pad_tail() {
    let trace = Trace::new(vec![
        Block::new(1, 0, 0),
        Block::new(0, 2, 0),
        Block::new(0, 0, 3),
        Block::new(4, 0, 0),
        Block::new(0, 5, 0),
        Block::new(0, 0, 6),
        Block::new(7, 0, 0),
        Block::new(0, 8, 0),
        Block::new(9, 0, 0),
        Block::new(0, 10, 0),
    ]);

    assert_eq!(trace.packed_chunk_count(), 2);
    assert_eq!(trace.packed_chunk_valid_lanes(0), 8);
    assert_eq!(trace.packed_chunk_valid_lanes(1), 2);

    let first = trace.packed_chunk(0);
    let second = trace.packed_chunk(1);

    assert_eq!(chunk_to_blocks(first, 8), trace.as_blocks()[..8].to_vec());
    assert_eq!(chunk_to_blocks(second, 2), trace.as_blocks()[8..].to_vec());
    for lane in 2..crate::simd::LANES_256 {
        assert_eq!(second.x[lane], 0);
        assert_eq!(second.y[lane], 0);
        assert_eq!(second.z[lane], 0);
    }
}

#[test]
fn trace_new_packs_xyz_from_input_exactly_and_zero_pads_each_tail_chunk() {
    let blocks: Vec<Block> = (0u32..33)
        .map(|i| Block::new(i, i.wrapping_mul(3), i.wrapping_mul(7)))
        .collect();
    let trace = Trace::new(blocks.clone());

    // This is the external oracle: Trace::new must preserve the caller-provided blocks.
    assert_eq!(trace.as_blocks(), blocks.as_slice());

    let chunk_count = trace.packed_chunk_count();
    assert_eq!(chunk_count, blocks.len().div_ceil(crate::simd::LANES_256));

    for chunk_index in 0..chunk_count {
        let chunk = trace.packed_chunk(chunk_index);
        let start = chunk_index * crate::simd::LANES_256;
        let valid = trace.packed_chunk_valid_lanes(chunk_index);

        for lane in 0..valid {
            let expected = blocks[start + lane];
            assert_eq!(chunk.x[lane], expected.x);
            assert_eq!(chunk.y[lane], expected.y);
            assert_eq!(chunk.z[lane], expected.z);
        }

        // If we ever optimize build_packed_chunks with unsafe loads/stores, unused lanes must stay
        // zeroed. Trace equality/hash/ordering depends on packed chunk contents.
        for lane in valid..crate::simd::LANES_256 {
            assert_eq!(chunk.x[lane], 0);
            assert_eq!(chunk.y[lane], 0);
            assert_eq!(chunk.z[lane], 0);
        }
    }
}

#[test]
fn trace_new_long_all_zero_materialization_keeps_packed_chunks_zero() {
    let len = crate::simd::LANES_256 * 8 + 3; // triggers the long all-zero fast path + tail.
    let blocks = vec![Block::zero(); len];
    let trace = Trace::new(blocks.clone());

    assert_eq!(trace.len_blocks(), len);
    assert_eq!(trace.as_blocks(), blocks.as_slice());
    assert_eq!(trace.packed_chunk_count(), len.div_ceil(crate::simd::LANES_256));

    for chunk_index in 0..trace.packed_chunk_count() {
        assert_eq!(*trace.packed_chunk(chunk_index), crate::simd::Chunk8::ZERO);
    }
}

#[test]
fn trace_new_long_uniform_nonzero_fast_path_encodes_tail_and_uniform_guard_works() {
    let len = crate::simd::LANES_256 * 8 + 3; // triggers long uniform detection + tail.
    let uniform = Block::new(7, 11, 13);

    let blocks = vec![uniform; len];
    let trace = Trace::new(blocks.clone());

    // External oracle: Trace::new must preserve caller-provided blocks.
    assert_eq!(trace.len_blocks(), len);
    assert_eq!(trace.as_blocks(), blocks.as_slice());

    // Packed encoding must match and must keep unused tail lanes zeroed.
    for chunk_index in 0..trace.packed_chunk_count() {
        let chunk = trace.packed_chunk(chunk_index);
        let start = chunk_index * crate::simd::LANES_256;
        let valid = trace.packed_chunk_valid_lanes(chunk_index);

        for lane in 0..valid {
            let expected = blocks[start + lane];
            assert_eq!(chunk.x[lane], expected.x);
            assert_eq!(chunk.y[lane], expected.y);
            assert_eq!(chunk.z[lane], expected.z);
        }
        for lane in valid..crate::simd::LANES_256 {
            assert_eq!(chunk.x[lane], 0);
            assert_eq!(chunk.y[lane], 0);
            assert_eq!(chunk.z[lane], 0);
        }
    }

    // Uniform detection must not mis-fire on a near-uniform input where the final block differs.
    let mut almost = blocks;
    let last = Block::new(99, 0, 0);
    almost[len - 1] = last;
    let trace = Trace::new(almost);
    assert_eq!(trace.block_at(len - 1), last);
}

#[test]
fn trace_new_long_uniform_detector_does_not_miss_inner_difference() {
    let len = crate::simd::LANES_256 * 8 + 3; // triggers long uniform detection + tail.
    let uniform = Block::new(7, 11, 13);
    let mut blocks = vec![uniform; len];

    // Ensure the differing element is neither the midpoint nor the last element, so the uniform
    // detector must rely on the full-word scan (not just probes).
    let diff_index = crate::simd::LANES_256 * 3 + 1;
    assert!(diff_index < len / 2);
    let diff = Block::new(7, 11, 14);
    blocks[diff_index] = diff;

    let trace = Trace::new(blocks.clone());

    // External oracle: Trace::new must preserve caller-provided blocks.
    assert_eq!(trace.len_blocks(), len);
    assert_eq!(trace.as_blocks(), blocks.as_slice());
    assert_eq!(trace.block_at(diff_index), diff);
    assert_eq!(trace.block_at(len - 1), uniform);

    // Packed encoding must preserve the differing lane.
    let chunk_index = diff_index / crate::simd::LANES_256;
    let lane = diff_index % crate::simd::LANES_256;
    let chunk = trace.packed_chunk(chunk_index);
    assert_eq!(chunk.x[lane], diff.x);
    assert_eq!(chunk.y[lane], diff.y);
    assert_eq!(chunk.z[lane], diff.z);
}

#[test]
fn trace_new_long_uniform_detector_does_not_miss_tail_difference() {
    let len = crate::simd::LANES_256 * 8 + 3; // triggers long uniform detection + tail.
    let uniform = Block::new(7, 11, 13);
    let mut blocks = vec![uniform; len];

    // Make the difference land inside the tail chunk but not at the last element, so the
    // uniform detector's midpoint/last probes still match and the full-word scan must catch it.
    let diff_index = len - 2;
    let diff = Block::new(7, 11, 14);
    blocks[diff_index] = diff;

    let trace = Trace::new(blocks.clone());

    assert_eq!(trace.len_blocks(), len);
    assert_eq!(trace.as_blocks(), blocks.as_slice());
    assert_eq!(trace.block_at(diff_index), diff);
    assert_eq!(trace.block_at(len - 1), uniform);

    let chunk_index = diff_index / crate::simd::LANES_256;
    let lane = diff_index % crate::simd::LANES_256;
    assert_eq!(chunk_index, trace.packed_chunk_count() - 1);
    assert!(lane < trace.packed_chunk_valid_lanes(chunk_index));

    let chunk = trace.packed_chunk(chunk_index);
    assert_eq!(chunk.x[lane], diff.x);
    assert_eq!(chunk.y[lane], diff.y);
    assert_eq!(chunk.z[lane], diff.z);

    for tail_lane in trace.packed_chunk_valid_lanes(chunk_index)..crate::simd::LANES_256 {
        assert_eq!(chunk.x[tail_lane], 0);
        assert_eq!(chunk.y[tail_lane], 0);
        assert_eq!(chunk.z[tail_lane], 0);
    }
}

#[test]
fn trace_new_boundary_lengths_zero_through_seventeen_preserve_blocks_and_tail_zeroing() {
    for len in 0..=(crate::simd::LANES_256 * 2 + 1) {
        let blocks: Vec<Block> = (0..len)
            .map(|i| {
                let i = i as u32;
                Block::new(i.wrapping_mul(3), i.wrapping_mul(5), i.wrapping_mul(7))
            })
            .collect();

        let trace = Trace::new(blocks.clone());
        let expected_len = blocks.len().max(1);
        let expected_blocks = if blocks.is_empty() {
            vec![Block::zero()]
        } else {
            blocks
        };

        assert_eq!(trace.len_blocks(), expected_len);
        assert_eq!(trace.as_blocks(), expected_blocks.as_slice());

        for chunk_index in 0..trace.packed_chunk_count() {
            let chunk = trace.packed_chunk(chunk_index);
            let start = chunk_index * crate::simd::LANES_256;
            let valid = trace.packed_chunk_valid_lanes(chunk_index);

            for lane in 0..valid {
                let expected = expected_blocks[start + lane];
                assert_eq!(chunk.x[lane], expected.x());
                assert_eq!(chunk.y[lane], expected.y());
                assert_eq!(chunk.z[lane], expected.z());
            }

            for lane in valid..crate::simd::LANES_256 {
                assert_eq!(chunk.x[lane], 0);
                assert_eq!(chunk.y[lane], 0);
                assert_eq!(chunk.z[lane], 0);
            }
        }

        assert_eq!(trace_into_blocks(trace), expected_blocks);
    }
}

#[test]
fn phi_kappa_matches_expected_after_packed_cache_materialization() {
    let trace = Trace::new(vec![
        Block::new(5, 0, 0),
        Block::new(0, 0, 0),
        Block::new(0, 5, 0),
        Block::new(0, 0, 5),
        Block::new(1, 1, 1),
        Block::new(2, 2, 2),
        Block::new(3, 3, 0),
        Block::new(0, 3, 3),
        Block::new(6, 0, 0),
    ]);
    let _ = trace.packed_chunks();

    let normalized = phi_kappa(&trace, 4);
    assert_eq!(
        trace_into_blocks(normalized),
        vec![
            Block::new(4, 0, 0),
            Block::new(1, 0, 0),
            Block::zero(),
            Block::new(0, 4, 0),
            Block::new(0, 1, 0),
            Block::new(0, 0, 4),
            Block::new(0, 0, 1),
            Block::new(1, 1, 1),
            Block::new(2, 2, 0),
            Block::new(0, 0, 2),
            Block::new(3, 1, 0),
            Block::new(0, 2, 0),
            Block::new(0, 3, 1),
            Block::new(0, 0, 2),
            Block::new(4, 0, 0),
            Block::new(2, 0, 0),
        ]
    );
}

#[test]
fn parallel_kappa_matches_expected_after_both_packed_caches_materialize() {
    let left = Trace::new(vec![
        Block::new(3, 1, 0),
        Block::new(0, 0, 0),
        Block::new(1, 1, 1),
        Block::new(4, 0, 0),
        Block::new(0, 2, 0),
        Block::new(0, 0, 3),
        Block::new(2, 0, 0),
        Block::new(0, 1, 0),
        Block::new(5, 0, 0),
    ]);
    let right = Trace::new(vec![
        Block::new(2, 2, 1),
        Block::new(1, 0, 0),
        Block::new(0, 1, 0),
        Block::new(1, 0, 1),
        Block::new(3, 0, 0),
        Block::new(0, 4, 0),
        Block::new(0, 0, 1),
        Block::new(2, 0, 0),
        Block::new(0, 5, 0),
        Block::new(0, 0, 2),
    ]);
    let _ = left.packed_chunks();
    let _ = right.packed_chunks();

    let parallel = parallel_kappa(&left, &right, 4);
    assert_eq!(
        trace_into_blocks(parallel),
        vec![
            Block::new(4, 0, 0),
            Block::new(1, 3, 0),
            Block::new(0, 0, 1),
            Block::new(1, 0, 0),
            Block::new(1, 2, 1),
            Block::new(4, 0, 0),
            Block::new(1, 0, 1),
            Block::new(3, 1, 0),
            Block::new(0, 1, 0),
            Block::new(0, 4, 0),
            Block::new(0, 0, 3),
            Block::new(2, 0, 1),
            Block::new(2, 1, 0),
            Block::new(4, 0, 0),
            Block::new(1, 3, 0),
            Block::new(0, 2, 0),
            Block::new(0, 0, 2),
        ]
    );
}

#[test]
fn parallel_kappa_matches_expected_across_chunk_boundary() {
    let u = Trace::new(vec![Block::new(1, 0, 0); 9]);
    let v = Trace::new(vec![Block::new(0, 1, 0); 5]);
    let r = parallel_kappa(&u, &v, 10);
    assert_eq!(
        trace_into_blocks(r),
        vec![
            Block::new(1, 1, 0),
            Block::new(1, 1, 0),
            Block::new(1, 1, 0),
            Block::new(1, 1, 0),
            Block::new(1, 1, 0),
            Block::new(1, 0, 0),
            Block::new(1, 0, 0),
            Block::new(1, 0, 0),
            Block::new(1, 0, 0),
        ]
    );
}

#[test]
fn odot_kappa_matches_expected_after_both_packed_caches_materialize_mixed_trace() {
    let left = Trace::new({
        let mut blocks = vec![Block::new(1, 0, 0); 9];
        blocks.push(Block::new(3, 0, 0));
        blocks
    });
    let right = Trace::new({
        let mut blocks = vec![Block::new(2, 0, 0)];
        blocks.extend(vec![Block::new(0, 1, 0); 9]);
        blocks
    });
    let _ = left.packed_chunks();
    let _ = right.packed_chunks();

    let odot = odot_kappa(&left, &right, 4);
    let mut expected = vec![Block::new(1, 0, 0); 9];
    expected.extend_from_slice(&[Block::new(4, 0, 0), Block::new(1, 0, 0)]);
    expected.extend(vec![Block::new(0, 1, 0); 9]);

    assert_eq!(trace_into_blocks(odot), expected);
}

#[test]
fn otimes_kappa_matches_expected_after_packed_cache_materialization_with_long_middle() {
    let u = Trace::new(vec![
        Block::new(1, 0, 0),
        Block::new(0, 1, 0),
        Block::new(0, 0, 1),
    ]);
    let v = Trace::new({
        let mut blocks = vec![Block::new(1, 0, 0)];
        blocks.extend(vec![Block::new(0, 0, 1); 8]);
        blocks.push(Block::new(0, 1, 0));
        blocks
    });
    let _ = u.packed_chunks();
    let _ = v.packed_chunks();

    let otimes = otimes_kappa(&u, &v, 8);
    let mut expected = vec![Block::new(2, 0, 0)];
    expected.extend(vec![Block::new(0, 0, 1); 8]);
    expected.push(Block::new(1, 2, 0));
    expected.extend(vec![Block::new(0, 0, 1); 8]);
    expected.push(Block::new(0, 1, 1));

    assert_eq!(trace_into_blocks(otimes), expected);
}

#[test]
fn otimes_kappa_matches_expected_after_packed_cache_materialization_with_two_block_v() {
    let u = Trace::new(vec![
        Block::new(1, 0, 0),
        Block::new(0, 1, 0),
        Block::new(1, 0, 0),
        Block::new(0, 1, 0),
        Block::new(1, 0, 0),
        Block::new(0, 1, 0),
        Block::new(1, 0, 0),
        Block::new(0, 1, 0),
        Block::new(1, 0, 0),
        Block::new(0, 1, 0),
    ]);
    let v = Trace::new(vec![Block::new(2, 0, 0), Block::new(0, 0, 1)]);
    let _ = u.packed_chunks();
    let _ = v.packed_chunks();

    let otimes = otimes_kappa(&u, &v, 16);
    assert_eq!(
        trace_into_blocks(otimes),
        vec![
            Block::new(3, 0, 0),
            Block::new(2, 1, 1),
            Block::new(3, 0, 1),
            Block::new(2, 1, 1),
            Block::new(3, 0, 1),
            Block::new(2, 1, 1),
            Block::new(3, 0, 1),
            Block::new(2, 1, 1),
            Block::new(3, 0, 1),
            Block::new(0, 1, 1),
        ]
    );
}

#[test]
fn otimes_kappa_matches_expected_after_packed_cache_materialization_with_interleaved_long_middle() {
    let u = Trace::new(vec![
        Block::new(1, 0, 0),
        Block::new(0, 1, 0),
        Block::new(1, 0, 0),
        Block::new(0, 1, 0),
        Block::new(1, 0, 0),
        Block::new(0, 1, 0),
        Block::new(1, 0, 0),
        Block::new(0, 1, 0),
        Block::new(1, 0, 0),
        Block::new(0, 1, 0),
    ]);
    let v = Trace::new({
        let mut blocks = vec![Block::new(2, 0, 0)];
        blocks.extend(vec![Block::new(0, 0, 1); 8]);
        blocks.push(Block::new(0, 2, 0));
        blocks
    });
    let _ = u.packed_chunks();
    let _ = v.packed_chunks();

    let otimes = otimes_kappa(&u, &v, 32);
    let mut expected = vec![Block::new(3, 0, 0)];
    let middle = vec![Block::new(0, 0, 1); 8];
    expected.extend_from_slice(&middle);
    for index in 1..u.len_blocks() - 1 {
        let boundary = if index % 2 == 0 {
            Block::new(3, 2, 0)
        } else {
            Block::new(2, 3, 0)
        };
        expected.push(boundary);
        expected.extend_from_slice(&middle);
    }
    expected.push(Block::new(0, 3, 0));

    assert_eq!(trace_into_blocks(otimes), expected);
}

#[test]
fn otimes_kappa_direct_packed_long_middle_zero_pads_tail_lanes() {
    let u = Trace::new(vec![
        Block::new(1, 0, 0),
        Block::new(0, 1, 0),
        Block::new(1, 0, 0),
    ]);
    let v = Trace::new({
        let mut blocks = vec![Block::new(2, 0, 0)];
        blocks.extend(vec![Block::new(0, 0, 1); 8]);
        blocks.push(Block::new(0, 2, 0));
        blocks
    });

    let otimes = otimes_kappa(&u, &v, 32);
    let last_chunk_index = otimes.packed_chunk_count() - 1;
    let valid_lanes = otimes.packed_chunk_valid_lanes(last_chunk_index);
    let last_chunk = otimes.packed_chunk(last_chunk_index);

    for lane in valid_lanes..crate::simd::LANES_256 {
        assert_eq!(last_chunk.x[lane], 0);
        assert_eq!(last_chunk.y[lane], 0);
        assert_eq!(last_chunk.z[lane], 0);
    }
}

#[test]
fn otimes_kappa_direct_packed_long_middle_preserves_result_after_materialization() {
    let u = Trace::new(vec![
        Block::new(1, 0, 0),
        Block::new(0, 1, 0),
        Block::new(1, 0, 0),
    ]);
    let v = Trace::new({
        let mut blocks = vec![Block::new(2, 0, 0)];
        blocks.extend(vec![Block::new(0, 0, 1); 8]);
        blocks.push(Block::new(0, 2, 0));
        blocks
    });

    let otimes = otimes_kappa(&u, &v, 32);
    let _ = otimes.as_blocks();

    assert_eq!(
        trace_into_blocks(otimes),
        vec![
            Block::new(3, 0, 0),
            Block::new(0, 0, 1),
            Block::new(0, 0, 1),
            Block::new(0, 0, 1),
            Block::new(0, 0, 1),
            Block::new(0, 0, 1),
            Block::new(0, 0, 1),
            Block::new(0, 0, 1),
            Block::new(0, 0, 1),
            Block::new(2, 3, 0),
            Block::new(0, 0, 1),
            Block::new(0, 0, 1),
            Block::new(0, 0, 1),
            Block::new(0, 0, 1),
            Block::new(0, 0, 1),
            Block::new(0, 0, 1),
            Block::new(0, 0, 1),
            Block::new(0, 0, 1),
            Block::new(1, 2, 0),
        ]
    );
}

#[test]
fn phi_kappa_exact_output_across_chunk_boundary() {
    let mut blocks = vec![Block::zero(); 7];
    blocks.push(Block::new(5, 0, 0));
    blocks.push(Block::new(0, 1, 0));
    let trace = Trace::new(blocks);
    let normalized = phi_kappa(&trace, 4);
    assert_eq!(
        trace_into_blocks(normalized),
        vec![
            Block::zero(),
            Block::zero(),
            Block::zero(),
            Block::zero(),
            Block::zero(),
            Block::zero(),
            Block::zero(),
            Block::new(4, 0, 0),
            Block::new(1, 0, 0),
            Block::new(0, 1, 0),
        ]
    );
}

#[test]
fn phi_kappa_exact_output_for_tail_split_mixture() {
    let trace = Trace::new(vec![
        Block::zero(),
        Block::zero(),
        Block::zero(),
        Block::new(5, 0, 0),
        Block::new(0, 4, 1),
        Block::new(2, 1, 2),
        Block::zero(),
        Block::new(1, 3, 1),
        Block::new(4, 0, 2),
    ]);
    let normalized = phi_kappa(&trace, 4);
    assert_eq!(
        trace_into_blocks(normalized),
        vec![
            Block::zero(),
            Block::zero(),
            Block::zero(),
            Block::new(4, 0, 0),
            Block::new(1, 0, 0),
            Block::new(0, 4, 0),
            Block::new(0, 0, 1),
            Block::new(2, 1, 1),
            Block::new(0, 0, 1),
            Block::zero(),
            Block::new(1, 3, 0),
            Block::new(0, 0, 1),
            Block::new(4, 0, 0),
            Block::new(0, 0, 2),
        ]
    );
}

#[test]
fn parallel_kappa_exact_output_for_tail_split_mixture() {
    let u = Trace::new({
        let mut blocks = vec![Block::new(1, 0, 0); 9];
        blocks.push(Block::new(2, 0, 0));
        blocks
    });
    let v = Trace::new({
        let mut blocks = vec![Block::new(0, 1, 0); 9];
        blocks.push(Block::new(0, 0, 1));
        blocks
    });
    let r = parallel_kappa(&u, &v, 2);
    assert_eq!(
        trace_into_blocks(r),
        vec![
            Block::new(1, 1, 0),
            Block::new(1, 1, 0),
            Block::new(1, 1, 0),
            Block::new(1, 1, 0),
            Block::new(1, 1, 0),
            Block::new(1, 1, 0),
            Block::new(1, 1, 0),
            Block::new(1, 1, 0),
            Block::new(1, 1, 0),
            Block::new(2, 0, 0),
            Block::new(0, 0, 1),
        ]
    );
}

#[test]
fn phi_kappa_exact_output_for_long_split_heavy_repeated_trace() {
    let trace = Trace::new(vec![Block::new(5, 0, 0); 9]);
    let normalized = phi_kappa(&trace, 4);
    let expected = vec![Block::new(4, 0, 0), Block::new(1, 0, 0)].repeat(9);

    assert_eq!(trace_into_blocks(normalized), expected);
}

#[test]
fn parallel_kappa_exact_output_for_long_split_heavy_repeated_trace() {
    let left = Trace::new(vec![Block::new(5, 0, 0); 9]);
    let right = Trace::new(vec![Block::new(5, 0, 0); 10]);
    let parallel = parallel_kappa(&left, &right, 4);

    let mut expected = vec![
        Block::new(4, 0, 0),
        Block::new(4, 0, 0),
        Block::new(2, 0, 0),
    ]
    .repeat(9);
    expected.extend_from_slice(&[Block::new(4, 0, 0), Block::new(1, 0, 0)]);

    assert_eq!(trace_into_blocks(parallel), expected);
}

#[test]
fn odot_kappa_exact_output_for_long_split_heavy_repeated_trace() {
    let left = Trace::new(vec![Block::new(5, 0, 0); 9]);
    let right = Trace::new(vec![Block::new(5, 0, 0); 9]);
    let product = odot_kappa(&left, &right, 4);

    let mut expected = vec![Block::new(4, 0, 0), Block::new(1, 0, 0)].repeat(8);
    expected.extend_from_slice(&[
        Block::new(4, 0, 0),
        Block::new(4, 0, 0),
        Block::new(2, 0, 0),
    ]);
    expected.extend_from_slice(&vec![Block::new(4, 0, 0), Block::new(1, 0, 0)].repeat(8));

    assert_eq!(trace_into_blocks(product), expected);
}

#[test]
fn divisibility_exact_output_for_multi_block_division_and_gcd() {
    let divisor = Trace::new(vec![Block::new(1, 0, 0), Block::new(0, 1, 0)]);
    let quotient = Trace::new(vec![Block::new(0, 0, 1), Block::new(1, 2, 0)]);
    let dividend = divisor.compose(&quotient);
    let secondary = Trace::new(vec![
        Block::new(1, 0, 0),
        Block::new(0, 2, 1),
        Block::new(3, 0, 0),
    ]);

    assert_eq!(
        trace_into_blocks(dividend.clone()),
        vec![
            Block::new(1, 0, 0),
            Block::new(0, 1, 1),
            Block::new(1, 2, 0),
        ]
    );
    assert_eq!(
        left_divide_trace(&divisor, &dividend),
        Some(quotient.clone())
    );
    assert_eq!(
        right_divide_trace(&quotient, &dividend),
        Some(divisor.clone())
    );
    assert_eq!(
        crate::divisibility::left_gcd_trace(&divisor, &secondary),
        divisor
    );

    let (g, t) = gcd_monomial(84, &divisor, 18, &secondary);
    assert_eq!(g, 6);
    assert_eq!(t, divisor);
}

#[test]
fn left_gcd_trace_matches_expected_across_chunk_boundary() {
    let left = Trace::new(vec![
        Block::new(1, 0, 0),
        Block::new(0, 1, 0),
        Block::new(0, 0, 1),
        Block::new(1, 1, 0),
        Block::new(1, 0, 1),
        Block::new(0, 1, 1),
        Block::new(1, 1, 1),
        Block::new(2, 0, 0),
        Block::new(3, 1, 0),
        Block::new(0, 2, 0),
    ]);
    let right = Trace::new(vec![
        Block::new(1, 0, 0),
        Block::new(0, 1, 0),
        Block::new(0, 0, 1),
        Block::new(1, 1, 0),
        Block::new(1, 0, 1),
        Block::new(0, 1, 1),
        Block::new(1, 1, 1),
        Block::new(2, 0, 0),
        Block::new(1, 4, 0),
        Block::new(9, 0, 0),
    ]);
    let _ = left.packed_chunks();
    let _ = right.packed_chunks();

    assert_eq!(
        crate::divisibility::left_gcd_trace(&left, &right),
        Trace::new(vec![
            Block::new(1, 0, 0),
            Block::new(0, 1, 0),
            Block::new(0, 0, 1),
            Block::new(1, 1, 0),
            Block::new(1, 0, 1),
            Block::new(0, 1, 1),
            Block::new(1, 1, 1),
            Block::new(2, 0, 0),
            Block::new(1, 1, 0),
        ])
    );
}

#[test]
fn left_divide_trace_matches_expected_across_chunk_boundary() {
    let divisor = Trace::new(vec![
        Block::new(1, 0, 0),
        Block::new(0, 1, 0),
        Block::new(0, 0, 1),
        Block::new(1, 1, 0),
        Block::new(1, 0, 1),
        Block::new(0, 1, 1),
        Block::new(1, 1, 1),
        Block::new(2, 0, 0),
        Block::new(1, 0, 0),
    ]);
    let quotient = Trace::new(vec![
        Block::new(0, 1, 0),
        Block::new(0, 0, 1),
        Block::new(0, 0, 1),
        Block::new(0, 0, 1),
        Block::new(0, 0, 1),
        Block::new(0, 0, 1),
        Block::new(0, 0, 1),
        Block::new(0, 0, 1),
        Block::new(0, 0, 1),
        Block::new(0, 0, 1),
    ]);
    let dividend = divisor.compose(&quotient);
    let _ = divisor.packed_chunks();
    let _ = dividend.packed_chunks();

    assert_eq!(left_divide_trace(&divisor, &dividend), Some(quotient));
}

#[test]
fn right_divide_trace_matches_expected_across_chunk_boundary() {
    let quotient = Trace::new(vec![
        Block::new(1, 0, 0),
        Block::new(0, 1, 0),
        Block::new(0, 0, 1),
        Block::new(1, 1, 0),
        Block::new(1, 0, 1),
        Block::new(0, 1, 1),
        Block::new(1, 1, 1),
        Block::new(2, 0, 0),
        Block::new(1, 0, 0),
    ]);
    let divisor = Trace::new(vec![
        Block::new(0, 1, 0),
        Block::new(0, 0, 1),
        Block::new(0, 0, 1),
        Block::new(0, 0, 1),
        Block::new(0, 0, 1),
        Block::new(0, 0, 1),
        Block::new(0, 0, 1),
        Block::new(0, 0, 1),
        Block::new(0, 0, 1),
        Block::new(0, 0, 1),
    ]);
    let dividend = quotient.compose(&divisor);
    let _ = divisor.packed_chunks();
    let _ = dividend.packed_chunks();

    assert_eq!(right_divide_trace(&divisor, &dividend), Some(quotient));
}

#[test]
fn modular_exact_output_for_compose_and_gcd_helpers() {
    let modulus = 7;
    let epsilon = Trace::empty();
    let x = Trace::new(vec![Block::new(1, 0, 0)]);
    let left = TS4Mod::from_trace(epsilon.clone(), 3, modulus).add(&TS4Mod::from_trace(
        x.clone(),
        4,
        modulus,
    ));
    let right = TS4Mod::from_trace(epsilon.clone(), 5, modulus).add(&TS4Mod::from_trace(
        x.clone(),
        6,
        modulus,
    ));

    let product = left.compose(&right);
    assert_eq!(
        product,
        ts4mod_from_pairs(
            modulus,
            &[(epsilon.clone(), 1), (x.clone(), 3), (x.compose(&x), 3),]
        )
    );

    assert_eq!(
        TS4Mod::from_trace(x.clone(), 14, modulus),
        TS4Mod::new(modulus)
    );
    assert_eq!(gcd_u32(84, 18), 6);
}

#[test]
fn simd_boundary_lengths_zero_through_nine_match_scalar_across_kappa_grid() {
    let blocks = vec![
        Block::new(0, 0, 0),
        Block::new(1, 0, 0),
        Block::new(0, 2, 0),
        Block::new(0, 0, 3),
        Block::new(2, 2, 0),
        Block::new(1, 1, 3),
        Block::new(4, 0, 0),
        Block::new(0, 5, 0),
        Block::new(0, 0, 6),
        Block::new(3, 3, 3),
    ];
    let kappas = [0u32, 1, 3, 6, 20];

    for len in 0..=9 {
        let slice = &blocks[..len];
        for &kappa in &kappas {
            let expected = scalar_blocks_l1_gt(slice, kappa);
            let mask = blocks_l1_gt_mask(slice, kappa);

            assert_eq!(sum_l1_blocks(slice), scalar_sum_l1(slice));
            assert_eq!(mask.len(), len);
            assert_eq!(mask.is_empty(), len == 0);
            assert_eq!(mask.as_words().len(), len.div_ceil(64));
            assert_eq!(mask.count_ones(), expected.iter().filter(|&&bit| bit).count());
            assert_eq!(blocks_l1_gt(slice, kappa), expected);
            assert_eq!(mask.clone().into_bools(), expected);

            for (index, expected_bit) in expected.iter().copied().enumerate() {
                assert_eq!(mask.get(index), expected_bit);
            }
        }
    }
}

#[test]
fn divisibility_ignores_zero_coefficient_garbage_terms_in_all_left_modes() {
    let x = Trace::new(vec![Block::new(1, 0, 0)]);
    let y = Trace::new(vec![Block::new(0, 1, 0)]);
    let z = Trace::new(vec![Block::new(0, 0, 1)]);

    let mut divisor_terms = BTreeMap::new();
    divisor_terms.insert(x.clone(), 0);
    divisor_terms.insert(y.clone(), 2);
    let divisor = TS4::from_raw_terms_unchecked(divisor_terms);

    let mut dividend_terms = BTreeMap::new();
    dividend_terms.insert(x, 0);
    dividend_terms.insert(y.compose(&z), 6);
    let dividend = TS4::from_raw_terms_unchecked(dividend_terms);

    let expected = TS4::from_trace(z, 3);

    assert_eq!(left_divide_ts4_monomial(&divisor, &dividend), Some(expected.clone()));
    assert_eq!(left_divide_ts4_unique(&divisor, &dividend), Some(expected.clone()));
    assert_eq!(left_divide_ts4_solve(&divisor, &dividend, 8, 2), Some(expected.clone()));
    assert_eq!(
        left_divide_ts4(
            &divisor,
            &dividend,
            SolveMode::Bounded {
                max_vars: 8,
                max_solutions: 2,
            },
        ),
        Some(expected.clone())
    );
    assert_eq!(
        left_divide_ts4(
            &divisor,
            &dividend,
            SolveMode::Unbounded { max_solutions: 2 },
        ),
        Some(expected)
    );
}

#[test]
fn right_divisibility_ignores_zero_coefficient_garbage_terms_in_monomial_mode() {
    let x = Trace::new(vec![Block::new(1, 0, 0)]);
    let y = Trace::new(vec![Block::new(0, 1, 0)]);
    let z = Trace::new(vec![Block::new(0, 0, 1)]);

    let mut divisor_terms = BTreeMap::new();
    divisor_terms.insert(x.clone(), 0);
    divisor_terms.insert(y.clone(), 2);
    let divisor = TS4::from_raw_terms_unchecked(divisor_terms);

    let mut dividend_terms = BTreeMap::new();
    dividend_terms.insert(x, 0);
    dividend_terms.insert(z.compose(&y), 6);
    let dividend = TS4::from_raw_terms_unchecked(dividend_terms);

    let expected = TS4::from_trace(z, 3);
    assert_eq!(right_divide_ts4_monomial(&divisor, &dividend), Some(expected));
}

#[test]
fn ts4_solver_wrappers_reject_zero_or_unusable_search_limits() {
    let ax = TS4::from_trace(Trace::new(vec![Block::new(1, 0, 0)]), 1);
    let ay = TS4::from_trace(Trace::new(vec![Block::new(0, 1, 0)]), 1);
    let a = ax.add(&ay);
    let b = TS4::from_trace(Trace::new(vec![Block::new(1, 1, 0)]), 2);

    assert_eq!(left_divide_ts4_solve(&a, &b, 0, 1), None);
    assert_eq!(
        left_divide_ts4(
            &a,
            &b,
            SolveMode::Bounded {
                max_vars: 8,
                max_solutions: 0,
            },
        ),
        None
    );
    assert_eq!(
        left_divide_ts4(&a, &b, SolveMode::Unbounded { max_solutions: 0 }),
        None
    );
}

#[test]
fn trace_into_blocks_matches_canonical_blocks_before_and_after_view_materialization() {
    let expected = vec![
        Block::new(1, 0, 0),
        Block::new(0, 2, 0),
        Block::new(0, 0, 3),
        Block::new(4, 0, 0),
        Block::new(0, 5, 0),
        Block::new(0, 0, 6),
        Block::new(7, 0, 0),
        Block::new(0, 8, 0),
        Block::new(9, 0, 0),
    ];

    let trace = Trace::new(expected.clone());
    assert_eq!(trace.clone().into_blocks(), expected);

    let trace = Trace::new(expected.clone());
    let _ = trace.as_blocks();
    assert_eq!(trace.into_blocks(), expected);
}

#[test]
fn append_blocks_range_matches_expected_across_chunk_boundary_and_tail() {
    let trace = Trace::new(vec![
        Block::new(1, 0, 0),
        Block::new(0, 1, 0),
        Block::new(0, 0, 1),
        Block::new(1, 1, 0),
        Block::new(1, 0, 1),
        Block::new(0, 1, 1),
        Block::new(1, 1, 1),
        Block::new(2, 0, 0),
        Block::new(3, 0, 0),
        Block::new(0, 4, 0),
        Block::new(0, 0, 5),
    ]);

    let mut out = Vec::new();
    append_blocks_range(&mut out, &trace, 3, 10);

    assert_eq!(
        out,
        vec![
            Block::new(1, 1, 0),
            Block::new(1, 0, 1),
            Block::new(0, 1, 1),
            Block::new(1, 1, 1),
            Block::new(2, 0, 0),
            Block::new(3, 0, 0),
            Block::new(0, 4, 0),
        ]
    );
}

#[test]
fn trace_blocks_equal_range_matches_scalar_across_misaligned_chunk_windows() {
    let left = Trace::new(vec![
        Block::new(9, 0, 0),
        Block::new(1, 0, 0),
        Block::new(0, 1, 0),
        Block::new(0, 0, 1),
        Block::new(1, 1, 0),
        Block::new(1, 0, 1),
        Block::new(0, 1, 1),
        Block::new(1, 1, 1),
        Block::new(2, 0, 0),
        Block::new(3, 0, 0),
        Block::new(0, 4, 0),
        Block::new(7, 0, 0),
    ]);
    let right = Trace::new(vec![
        Block::new(5, 5, 5),
        Block::new(1, 0, 0),
        Block::new(0, 1, 0),
        Block::new(0, 0, 1),
        Block::new(1, 1, 0),
        Block::new(1, 0, 1),
        Block::new(0, 1, 1),
        Block::new(1, 1, 1),
        Block::new(2, 0, 0),
        Block::new(3, 0, 0),
        Block::new(0, 4, 0),
        Block::new(8, 0, 0),
    ]);

    let _ = left.packed_chunks();
    let _ = right.packed_chunks();

    assert!(left.blocks_equal_range(&right, 1, 1, 10));
    assert!(!left.blocks_equal_range(&right, 0, 0, 12));
    assert!(!left.blocks_equal_range(&right, 1, 1, 11));
}

#[test]
fn trace_common_prefix_len_matches_expected_across_chunk_boundary() {
    let left = Trace::new(vec![
        Block::new(1, 0, 0),
        Block::new(0, 1, 0),
        Block::new(0, 0, 1),
        Block::new(1, 1, 0),
        Block::new(1, 0, 1),
        Block::new(0, 1, 1),
        Block::new(1, 1, 1),
        Block::new(2, 0, 0),
        Block::new(3, 0, 0),
        Block::new(0, 4, 0),
        Block::new(0, 0, 5),
    ]);
    let right = Trace::new(vec![
        Block::new(1, 0, 0),
        Block::new(0, 1, 0),
        Block::new(0, 0, 1),
        Block::new(1, 1, 0),
        Block::new(1, 0, 1),
        Block::new(0, 1, 1),
        Block::new(1, 1, 1),
        Block::new(2, 0, 0),
        Block::new(9, 0, 0),
        Block::new(0, 4, 0),
        Block::new(0, 0, 5),
    ]);

    let _ = left.packed_chunks();
    let _ = right.packed_chunks();

    assert_eq!(left.common_prefix_len(&right, 11), 8);
    assert_eq!(left.common_prefix_len(&right, 8), 8);
    assert_eq!(left.common_prefix_len(&right, 5), 5);
}

#[test]
fn append_blocks_range_noop_for_empty_range_keeps_output_unchanged() {
    let trace = Trace::new(vec![
        Block::new(1, 0, 0),
        Block::new(0, 1, 0),
        Block::new(0, 0, 1),
        Block::new(1, 1, 0),
        Block::new(2, 0, 0),
    ]);

    let mut out = vec![Block::new(9, 9, 9)];
    append_blocks_range(&mut out, &trace, 3, 3);

    assert_eq!(out, vec![Block::new(9, 9, 9)]);
}

#[test]
fn trace_blocks_equal_range_zero_len_returns_true_without_touching_indices() {
    let left = Trace::new(vec![Block::new(1, 0, 0), Block::new(0, 1, 0)]);
    let right = Trace::new(vec![Block::new(9, 0, 0), Block::new(0, 9, 0)]);

    assert!(left.blocks_equal_range(&right, left.len_blocks(), right.len_blocks(), 0));
}

#[test]
fn left_divide_trace_identity_returns_empty_trace_across_chunk_boundary() {
    let trace = Trace::new(vec![
        Block::new(1, 0, 0),
        Block::new(0, 1, 0),
        Block::new(0, 0, 1),
        Block::new(1, 1, 0),
        Block::new(1, 0, 1),
        Block::new(0, 1, 1),
        Block::new(1, 1, 1),
        Block::new(2, 0, 0),
        Block::new(3, 0, 0),
        Block::new(0, 4, 0),
    ]);

    let quotient = left_divide_trace(&trace, &trace).expect("left self-divide should succeed");
    assert_eq!(quotient, Trace::empty());
}

#[test]
fn right_divide_trace_identity_returns_empty_trace_across_chunk_boundary() {
    let trace = Trace::new(vec![
        Block::new(1, 0, 0),
        Block::new(0, 1, 0),
        Block::new(0, 0, 1),
        Block::new(1, 1, 0),
        Block::new(1, 0, 1),
        Block::new(0, 1, 1),
        Block::new(1, 1, 1),
        Block::new(2, 0, 0),
        Block::new(3, 0, 0),
        Block::new(0, 4, 0),
    ]);

    let quotient = right_divide_trace(&trace, &trace).expect("right self-divide should succeed");
    assert_eq!(quotient, Trace::empty());
}

#[test]
fn trace_block_at_matches_materialized_view_for_all_indices() {
    let blocks = vec![
        Block::new(1, 0, 0),
        Block::new(0, 1, 0),
        Block::new(0, 0, 1),
        Block::new(1, 1, 0),
        Block::new(1, 0, 1),
        Block::new(0, 1, 1),
        Block::new(1, 1, 1),
        Block::new(2, 0, 0),
        Block::new(3, 0, 0),
        Block::new(0, 4, 0),
        Block::new(0, 0, 5),
        Block::new(6, 0, 0),
        Block::new(0, 7, 0),
        Block::new(0, 0, 8),
        Block::new(9, 0, 0),
        Block::new(0, 10, 0),
        Block::new(0, 0, 11),
    ];
    let trace = Trace::new(blocks.clone());

    for (index, expected) in blocks.iter().copied().enumerate() {
        assert_eq!(trace.block_at(index), expected);
    }

    let materialized = trace.as_blocks().to_vec();
    for (index, expected) in materialized.iter().copied().enumerate() {
        assert_eq!(trace.block_at(index), expected);
    }
}

#[test]
fn append_blocks_range_appends_to_non_empty_buffer_without_overwrite() {
    let trace = Trace::new(vec![
        Block::new(1, 0, 0),
        Block::new(0, 1, 0),
        Block::new(0, 0, 1),
        Block::new(1, 1, 0),
        Block::new(1, 0, 1),
        Block::new(0, 1, 1),
        Block::new(1, 1, 1),
        Block::new(2, 0, 0),
        Block::new(3, 0, 0),
        Block::new(0, 4, 0),
        Block::new(0, 0, 5),
    ]);
    let mut out = vec![Block::new(9, 9, 9), Block::new(8, 8, 8)];

    append_blocks_range(&mut out, &trace, 6, 11);

    assert_eq!(
        out,
        vec![
            Block::new(9, 9, 9),
            Block::new(8, 8, 8),
            Block::new(1, 1, 1),
            Block::new(2, 0, 0),
            Block::new(3, 0, 0),
            Block::new(0, 4, 0),
            Block::new(0, 0, 5),
        ]
    );
}

#[test]
fn left_divide_trace_single_block_identity_returns_empty_trace() {
    let trace = Trace::new(vec![Block::new(7, 3, 2)]);
    let quotient = left_divide_trace(&trace, &trace).expect("left self-divide should succeed");
    assert_eq!(quotient, Trace::empty());
}

#[test]
fn right_divide_trace_single_block_identity_returns_empty_trace() {
    let trace = Trace::new(vec![Block::new(7, 3, 2)]);
    let quotient = right_divide_trace(&trace, &trace).expect("right self-divide should succeed");
    assert_eq!(quotient, Trace::empty());
}

#[test]
fn trace_mass_l1_matches_free_function_before_and_after_materialization() {
    let trace = Trace::new(vec![
        Block::new(1, 0, 0),
        Block::new(0, 2, 0),
        Block::new(0, 0, 3),
        Block::new(4, 0, 0),
        Block::new(0, 5, 0),
        Block::new(0, 0, 6),
        Block::new(7, 0, 0),
        Block::new(0, 8, 0),
        Block::new(9, 0, 0),
    ]);

    assert_eq!(trace.mass_l1(), mass_l1(&trace));
    let _ = trace.as_blocks();
    assert_eq!(trace.mass_l1(), mass_l1(&trace));
}

#[test]
fn trace_blocks_l1_gt_mask_matches_slice_contract_before_and_after_materialization() {
    let trace = Trace::new(vec![
        Block::new(1, 0, 0),
        Block::new(0, 2, 0),
        Block::new(0, 0, 3),
        Block::new(1, 1, 0),
        Block::new(1, 0, 1),
        Block::new(0, 1, 1),
        Block::new(1, 1, 1),
        Block::new(2, 0, 0),
        Block::new(3, 0, 0),
        Block::new(0, 4, 0),
    ]);

    let expected = blocks_l1_gt_mask(trace.as_blocks(), 2);
    assert_eq!(trace.blocks_l1_gt_mask(2), expected);

    let _ = trace.as_blocks();
    assert_eq!(trace.blocks_l1_gt_mask(2), expected);
}

#[test]
fn trace_pi_matches_free_function_before_and_after_materialization() {
    let trace = Trace::new(vec![
        Block::new(1, 0, 0),
        Block::new(0, 2, 0),
        Block::new(0, 0, 3),
        Block::new(4, 0, 0),
        Block::new(0, 5, 0),
        Block::new(0, 0, 6),
        Block::new(7, 0, 0),
        Block::new(0, 8, 0),
        Block::new(9, 0, 0),
    ]);

    assert_eq!(trace.pi(), pi_trace(&trace));
    let _ = trace.as_blocks();
    assert_eq!(trace.pi(), pi_trace(&trace));
}

#[test]
fn trace_blocks_equal_range_preserves_results_after_materializing_both_views() {
    let left = Trace::new(vec![
        Block::new(9, 0, 0),
        Block::new(1, 0, 0),
        Block::new(0, 1, 0),
        Block::new(0, 0, 1),
        Block::new(1, 1, 0),
        Block::new(1, 0, 1),
        Block::new(0, 1, 1),
        Block::new(1, 1, 1),
        Block::new(2, 0, 0),
        Block::new(3, 0, 0),
        Block::new(0, 4, 0),
        Block::new(7, 0, 0),
    ]);
    let right = Trace::new(vec![
        Block::new(5, 5, 5),
        Block::new(1, 0, 0),
        Block::new(0, 1, 0),
        Block::new(0, 0, 1),
        Block::new(1, 1, 0),
        Block::new(1, 0, 1),
        Block::new(0, 1, 1),
        Block::new(1, 1, 1),
        Block::new(2, 0, 0),
        Block::new(3, 0, 0),
        Block::new(0, 4, 0),
        Block::new(8, 0, 0),
    ]);

    let expected_equal = left.blocks_equal_range(&right, 1, 1, 10);
    let expected_mismatch = left.blocks_equal_range(&right, 1, 1, 11);

    let _ = left.as_blocks();
    let _ = right.as_blocks();

    assert_eq!(left.blocks_equal_range(&right, 1, 1, 10), expected_equal);
    assert_eq!(left.blocks_equal_range(&right, 1, 1, 11), expected_mismatch);
    assert!(!left.blocks_equal_range(&right, 0, 0, 12));
}

#[test]
fn trace_common_prefix_len_preserves_results_after_materialized_views() {
    let left = Trace::new(vec![
        Block::new(1, 0, 0),
        Block::new(0, 1, 0),
        Block::new(0, 0, 1),
        Block::new(1, 1, 0),
        Block::new(1, 0, 1),
        Block::new(0, 1, 1),
        Block::new(1, 1, 1),
        Block::new(2, 0, 0),
        Block::new(3, 0, 0),
        Block::new(0, 4, 0),
        Block::new(0, 0, 5),
    ]);
    let right = Trace::new(vec![
        Block::new(1, 0, 0),
        Block::new(0, 1, 0),
        Block::new(0, 0, 1),
        Block::new(1, 1, 0),
        Block::new(1, 0, 1),
        Block::new(0, 1, 1),
        Block::new(1, 1, 1),
        Block::new(2, 0, 0),
        Block::new(9, 0, 0),
        Block::new(0, 4, 0),
        Block::new(0, 0, 5),
    ]);

    let prefix_full = left.common_prefix_len(&right, 11);
    let prefix_short = left.common_prefix_len(&right, 5);

    let _ = left.as_blocks();
    let _ = right.as_blocks();

    assert_eq!(left.common_prefix_len(&right, 11), prefix_full);
    assert_eq!(left.common_prefix_len(&right, 8), 8);
    assert_eq!(left.common_prefix_len(&right, 5), prefix_short);
}

#[test]
fn append_blocks_range_matches_materialized_slice_window() {
    let trace = Trace::new(vec![
        Block::new(1, 0, 0),
        Block::new(0, 1, 0),
        Block::new(0, 0, 1),
        Block::new(1, 1, 0),
        Block::new(1, 0, 1),
        Block::new(0, 1, 1),
        Block::new(1, 1, 1),
        Block::new(2, 0, 0),
        Block::new(3, 0, 0),
        Block::new(0, 4, 0),
        Block::new(0, 0, 5),
        Block::new(6, 0, 0),
    ]);
    let materialized = trace.as_blocks().to_vec();

    let mut out = vec![Block::new(9, 9, 9)];
    append_blocks_range(&mut out, &trace, 2, 11);

    let mut expected = vec![Block::new(9, 9, 9)];
    expected.extend_from_slice(&materialized[2..11]);
    assert_eq!(out, expected);
}
