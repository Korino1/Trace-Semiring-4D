use ts4::prelude::*;
use ts4_consumer_fixture::{build_smoke_report, SmokeReport};

#[test]
fn downstream_smoke_demo_matches_exact_contract() {
    let report = build_smoke_report();
    let expected = SmokeReport {
        left_trace: Trace::new(vec![Block::new(1, 0, 0), Block::new(0, 1, 0)]),
        right_trace: Trace::from_word("z"),
        composed_trace: Trace::new(vec![Block::new(1, 0, 0), Block::new(0, 1, 1)]),
        normalized_trace: Trace::new(vec![
            Block::new(1, 0, 0),
            Block::new(0, 1, 0),
            Block::new(0, 0, 1),
        ]),
        mass_l1: 3,
        simd_mass_l1: 3,
        over_kappa: Trace::new(vec![Block::new(1, 1, 0), Block::new(5, 0, 0)]).blocks_l1_gt_mask(3),
        projection: (1, 1, 1, 1),
        ts4_term_count: 1,
        ts4_coeff_sum: 6,
    };

    assert_eq!(report, expected);
}
