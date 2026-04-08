//! Prelude for the canonical Zen 4-facing API.
//!
//! Everything re-exported here assumes the pinned nightly and the fixed
//! x86_64/MSVC Zen 4 contract.

pub use crate::{
    Block, BlockMask, SolveMode, TS4, TS4Mod, Trace, blocks_l1_gt, blocks_l1_gt_mask,
    gcd_monomial, gcd_u32, layers, left_divide_monomial, left_divide_trace, left_divide_ts4,
    left_divide_ts4_monomial, left_divide_ts4_solve, left_divide_ts4_unique, left_gcd_trace, mass_l1,
    min_layers_for_mass, min_tau_for_mass, odot_kappa, otimes_kappa, parallel_kappa, phi_kappa,
    pi_trace, pi_ts4, proj_r4, right_divide_monomial, right_divide_trace,
    right_divide_ts4_monomial, split_block_kappa, sum_l1_blocks, tau_count, SyncTrace, is_kappa_admissible,
    is_tight_core, is_trace_atom, left_divide_trace_is_exact, monomial_left_divides, normalize_trace,
    normalize_ts4, pi_trace_compose_morphism_holds, proj_r4_matches_scaled_pi,
    trace_left_cancellation_holds, traces_equal_by_normal_form, ts4_equal_by_normal_form,
    ts4_noncommutative_example, ts4_semiring_laws_hold,
};
