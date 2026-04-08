//! First-class theory layer for sections 31-37 of the concept.

use crate::divisibility::{left_divide_monomial, left_divide_trace};
use crate::projections::{pi_trace, proj_r4};
use crate::trace::Trace;
use crate::ts4::TS4;
use crate::types::Block;
use std::collections::BTreeMap;

/// Canonical trace normal form.
#[inline]
pub fn normalize_trace(trace: &Trace) -> Trace {
    Trace::new(trace.as_blocks().to_vec())
}

/// Canonical TS4 normal form with zero coefficients removed.
#[inline]
pub fn normalize_ts4(value: &TS4) -> TS4 {
    let mut terms = BTreeMap::new();
    for (trace, coeff) in value.iter() {
        terms.insert(normalize_trace(trace), coeff);
    }
    TS4::from_raw_terms_unchecked_for_theory(terms)
}

/// Equality of traces via their canonical normal forms.
#[inline]
pub fn traces_equal_by_normal_form(lhs: &Trace, rhs: &Trace) -> bool {
    normalize_trace(lhs) == normalize_trace(rhs)
}

/// Equality of TS4 values via their canonical normal forms.
#[inline]
pub fn ts4_equal_by_normal_form(lhs: &TS4, rhs: &TS4) -> bool {
    normalize_ts4(lhs) == normalize_ts4(rhs)
}

/// Semiring-law check on a concrete triple `(a, b, c)`.
#[inline]
pub fn ts4_semiring_laws_hold(a: &TS4, b: &TS4, c: &TS4) -> bool {
    let add_assoc = a.add(&b.add(c)) == a.add(b).add(c);
    let add_comm = a.add(b) == b.add(a);
    let compose_assoc = a.compose(&b.compose(c)) == a.compose(b).compose(c);
    let left_dist = a.compose(&b.add(c)) == a.compose(b).add(&a.compose(c));
    let right_dist = a.add(b).compose(c) == a.compose(c).add(&b.compose(c));
    let additive_zero = a.add(&TS4::zero()) == *a;
    let multiplicative_one = a.compose(&TS4::one()) == *a && TS4::one().compose(a) == *a;
    add_assoc && add_comm && compose_assoc && left_dist && right_dist && additive_zero && multiplicative_one
}

/// Concrete non-commutativity witness from the concept: `[tau]∘[x] != [x]∘[tau]`.
#[inline]
pub fn ts4_noncommutative_example() -> bool {
    let tau = TS4::from_trace(Trace::tau(1), 1);
    let x = TS4::from_trace(Trace::new(vec![Block::new(1, 0, 0)]), 1);
    tau.compose(&x) != x.compose(&tau)
}

/// Left-cancellation witness on concrete traces.
#[inline]
pub fn trace_left_cancellation_holds(prefix: &Trace, lhs: &Trace, rhs: &Trace) -> bool {
    let left = prefix.compose(lhs);
    let right = prefix.compose(rhs);
    left != right || lhs == rhs
}

/// Atom/irreducibility check for traces.
#[inline]
pub fn is_trace_atom(trace: &Trace) -> bool {
    if trace.len_blocks() == 1 {
        return trace.as_blocks()[0].l1() == 1;
    }
    trace == &Trace::tau(1)
}

/// Monomial left-divisibility criterion from section 31.5 / 32.24.
#[inline]
pub fn monomial_left_divides(a_coeff: u32, a: &Trace, b_coeff: u32, b: &Trace) -> bool {
    left_divide_monomial(a_coeff, a, b_coeff, b).is_some()
}

/// Algorithmic left-divisibility exactness witness for traces.
#[inline]
pub fn left_divide_trace_is_exact(lhs: &Trace, rhs: &Trace) -> bool {
    match left_divide_trace(lhs, rhs) {
        Some(tail) => lhs.compose(&tail) == *rhs,
        None => true,
    }
}

/// Morphism law for `pi_trace`: composition maps to addition in `N^4`.
#[inline]
pub fn pi_trace_compose_morphism_holds(lhs: &Trace, rhs: &Trace) -> bool {
    let composed = lhs.compose(rhs);
    let (ct, cx, cy, cz) = pi_trace(&composed);
    let (lt, lx, ly, lz) = pi_trace(lhs);
    let (rt, rx, ry, rz) = pi_trace(rhs);
    (ct, cx, cy, cz) == (lt + rt, lx + rx, ly + ry, lz + rz)
}

/// Scale interpretation compatibility: `proj_r4` is just scaled `pi_trace`.
#[inline]
pub fn proj_r4_matches_scaled_pi(trace: &Trace, t0: f64, l0: f64) -> bool {
    let (tau, x, y, z) = pi_trace(trace);
    proj_r4(trace, t0, l0)
        == (
            tau as f64 * t0,
            x as f64 * l0,
            y as f64 * l0,
            z as f64 * l0,
        )
}
