//! Проекции и морфизмы (TS4 → N^4, TS4 → R^4).
//!
//! Быстрое использование:
//! ```
//! use ts4::{Block, Trace, pi_trace, proj_r4};
//! let t = Trace::new(vec![Block::new(1,2,3), Block::zero()]);
//! let p = pi_trace(&t);
//! let r = proj_r4(&t, 0.5, 2.0);
//! ```

use crate::trace::Trace;
use crate::ts4::TS4;

#[inline(always)]
fn checked_add_u64(lhs: u64, rhs: u64, context: &str) -> u64 {
    match lhs.checked_add(rhs) {
        Some(value) => value,
        None => panic!("{context} overflow"),
    }
}

#[inline(always)]
fn checked_mul_u64(lhs: u64, rhs: u64, context: &str) -> u64 {
    match lhs.checked_mul(rhs) {
        Some(value) => value,
        None => panic!("{context} overflow"),
    }
}

/// Проекция трассы в N^4: (#τ, #x, #y, #z)
pub fn pi_trace(t: &Trace) -> (u32, u32, u32, u32) {
    t.pi()
}

/// Проекция TS4 в N^4: сумма проекций трасс с коэффициентами.
pub fn pi_ts4(a: &TS4) -> (u64, u64, u64, u64) {
    let mut tau = 0u64;
    let mut x = 0u64;
    let mut y = 0u64;
    let mut z = 0u64;
    for (t, c) in a.iter() {
        let (tt, xx, yy, zz) = pi_trace(t);
        let cc = c as u64;
        tau = checked_add_u64(
            tau,
            checked_mul_u64(cc, tt as u64, "pi_ts4 tau"),
            "pi_ts4 tau",
        );
        x = checked_add_u64(x, checked_mul_u64(cc, xx as u64, "pi_ts4 x"), "pi_ts4 x");
        y = checked_add_u64(y, checked_mul_u64(cc, yy as u64, "pi_ts4 y"), "pi_ts4 y");
        z = checked_add_u64(z, checked_mul_u64(cc, zz as u64, "pi_ts4 z"), "pi_ts4 z");
    }
    (tau, x, y, z)
}

/// Масштабная проекция в R^4 по (t0, l0).
pub fn proj_r4(t: &Trace, t0: f64, l0: f64) -> (f64, f64, f64, f64) {
    let (tau, x, y, z) = pi_trace(t);
    (
        (tau as f64) * t0,
        (x as f64) * l0,
        (y as f64) * l0,
        (z as f64) * l0,
    )
}
