fn main() {
    let report = ts4_consumer_fixture::build_smoke_report();

    println!("left_trace_blocks={:?}", report.left_trace.as_blocks());
    println!("right_trace_blocks={:?}", report.right_trace.as_blocks());
    println!(
        "composed_trace_blocks={:?}",
        report.composed_trace.as_blocks()
    );
    println!(
        "normalized_trace_blocks={:?}",
        report.normalized_trace.as_blocks()
    );
    println!("mass_l1={}", report.mass_l1);
    println!("simd_mass_l1={}", report.simd_mass_l1);
    println!("over_kappa={:?}", report.over_kappa);
    println!("projection={:?}", report.projection);
    println!("ts4_term_count={}", report.ts4_term_count);
    println!("ts4_coeff_sum={}", report.ts4_coeff_sum);
}
