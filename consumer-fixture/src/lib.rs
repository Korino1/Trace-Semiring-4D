use ts4::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmokeReport {
    pub left_trace: Trace,
    pub right_trace: Trace,
    pub composed_trace: Trace,
    pub normalized_trace: Trace,
    pub mass_l1: u64,
    pub simd_mass_l1: u64,
    pub over_kappa: BlockMask,
    pub projection: (u32, u32, u32, u32),
    pub ts4_term_count: usize,
    pub ts4_coeff_sum: u32,
}

pub fn build_smoke_report() -> SmokeReport {
    let left_trace = Trace::new(vec![Block::new(1, 0, 0), Block::new(0, 1, 0)]);
    let right_trace = Trace::from_word("z");

    let composed_trace = left_trace.compose(&right_trace);
    let normalized_trace = phi_kappa(&composed_trace, 1);

    let left_poly = TS4::from_trace(left_trace.clone(), 2);
    let right_poly = TS4::from_trace(right_trace.clone(), 3);
    let product = left_poly.compose(&right_poly);
    let ts4_term_count = product.term_count();
    let ts4_coeff_sum = product.coeff_sum();

    SmokeReport {
        left_trace,
        right_trace,
        composed_trace: composed_trace.clone(),
        normalized_trace,
        mass_l1: composed_trace.mass_l1(),
        simd_mass_l1: composed_trace.mass_l1(),
        over_kappa: composed_trace.blocks_l1_gt_mask(1),
        projection: composed_trace.pi(),
        ts4_term_count,
        ts4_coeff_sum,
    }
}
