#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../docs/USAGE.md")]

//! TS4: 4D арифметика трасс (Trace Semiring 4D).
//!
//! Каноническая публичная поверхность здесь ориентирована на Zen 4.
//! External consumers should use the root reexports or `ts4::prelude::*` only.
//! Implementation submodules are kept private to avoid downstream coupling
//! to internal layout and kernel details.
//! Compile-time target enforcement is internal to the crate, while the SIMD
//! layer keeps the Zen 4 fixed-path contract for the hot kernels.
//!
//! Базовые объекты:
//! - `Trace` — каноническая трасса в виде блоков между `τ`.
//! - `TS4` — конечная формальная сумма трасс с коэффициентами в `ℕ`.
//!
//! Основные операции:
//! - `add` (мультисет-сумма),
//! - `compose` (`∘`),
//! - `odot_kappa`, `parallel_kappa`, `otimes_kappa` (физический слой).

#[doc(hidden)]
mod algorithms;
#[doc(hidden)]
mod cpu;
mod divisibility;
#[allow(dead_code)]
mod docs;
mod invariants;
mod modular;
mod physical;
pub mod prelude;
mod projections;
#[doc(hidden)]
mod simd;
mod theory;
mod trace;
mod ts4;
mod types;

pub use algorithms::{odot_kappa, otimes_kappa, parallel_kappa, phi_kappa, split_block_kappa};
pub use divisibility::{
    SolveMode, left_divide_monomial, left_divide_trace, left_divide_ts4, left_divide_ts4_monomial,
    left_divide_ts4_solve, left_divide_ts4_unique, left_gcd_trace, right_divide_monomial,
    right_divide_trace, right_divide_ts4_monomial,
};
pub use invariants::{layers, mass_l1, min_layers_for_mass, min_tau_for_mass, tau_count};
pub use modular::{TS4Mod, gcd_monomial, gcd_u32};
pub use physical::{SyncTrace, is_kappa_admissible, is_tight_core};
pub use projections::{pi_trace, pi_ts4, proj_r4};
pub use simd::{BlockMask, blocks_l1_gt, blocks_l1_gt_mask, sum_l1_blocks};
pub use theory::{
    is_trace_atom, left_divide_trace_is_exact, monomial_left_divides, normalize_trace,
    normalize_ts4, pi_trace_compose_morphism_holds, proj_r4_matches_scaled_pi,
    trace_left_cancellation_holds, traces_equal_by_normal_form, ts4_equal_by_normal_form,
    ts4_noncommutative_example, ts4_semiring_laws_hold,
};
pub use trace::Trace;
pub use ts4::TS4;
pub use types::Block;

#[cfg(test)]
mod tests;
