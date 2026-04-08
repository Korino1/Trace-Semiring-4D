# Release checklist

Этот чеклист считается валидным только после прохождения реальной проверки на Ryzen 9 7950X / Zen 4.
Текущее состояние: hardcoded local paths из docs/scripts убраны, build/doc contract опирается на `.cargo/config.toml`, `cargo doc --no-deps` на локальной проверке проходит без warnings, consumer-fixture docs linkage подтверждена, а текущая ветка зелёная на `cargo test --quiet` и `cargo test --release --quiet` (`171/171` unit tests + `109/109` doctests в последнем полном прогоне). Последние conceptual шаги закрыли `docs/4d_sections_clean.txt` по разделам `0–37`: first-class `Trace::tau(n)` покрывает раздел `11`, `physical::{SyncTrace, is_kappa_admissible, is_tight_core}` плюс `boxplus`, `boxplus_tight`, `sequential`, `time_refine`, `successor_t/x/y/z` закрывают разделы `20–22`, а `theory::{normalize_trace, normalize_ts4, traces_equal_by_normal_form, ts4_equal_by_normal_form, ts4_semiring_laws_hold, ts4_noncommutative_example, trace_left_cancellation_holds, is_trace_atom, monomial_left_divides, left_divide_trace_is_exact, pi_trace_compose_morphism_holds, proj_r4_matches_scaled_pi}` закрывает разделы `31–37`. Последний подтверждённый `.\tools\check.ps1` на текущем SMT-on host также зелёный и прогнал `30` measured benches, включая `blocks_l1_gt` ~`1499 ns`, `blocks_l1_gt_mask` ~`823 ns`, `odot_kappa` ~`254 ns`, `otimes_kappa` ~`834 ns`, `parallel_kappa` ~`210 ns`, `phi_kappa` ~`87 ns`, `trace_compose` ~`227 ns`, `trace_new_pack_materialize_len_large_middle` ~`8296 ns`, и `ts4_compose` ~`783 ns`. Proof surface теперь дополнительно фиксирует packed-aware divisibility helpers в `Trace` (`blocks_equal_range`, `common_prefix_len`, `append_blocks_range`, cached `block_at` after materialized view, materialized fast paths for range equality/prefix scans), first-class `Trace` helpers (`tau`, `mass_l1`, `blocks_l1_gt_mask`, `pi`), packed-aware physical class layer coverage (`Trace::all_blocks_l1_le`, `Trace::try_parallel_tight`, `SyncTrace`, `is_kappa_admissible`, `is_tight_core`, `boxplus`, `boxplus_tight`, `sequential`, `time_refine`, successors), first-class theory-layer coverage (`normalize_trace`, `normalize_ts4`, `traces_equal_by_normal_form`, `ts4_equal_by_normal_form`, `ts4_semiring_laws_hold`, `ts4_noncommutative_example`, `trace_left_cancellation_holds`, `is_trace_atom`, `monomial_left_divides`, `left_divide_trace_is_exact`, `pi_trace_compose_morphism_holds`, `proj_r4_matches_scaled_pi`), SIMD vector-path overflow panic tests для `sum_l1_blocks`/`blocks_l1_gt_mask`, symmetric right-divide garbage contract, cross-chunk `left_gcd_trace` / `left_divide_trace` / `right_divide_trace` cases, boundary-negative chunk-mismatch divisibility checks, chunk-boundary identity divisibility cases, single-block identity divisibility cases, append-to-nonempty helper contract, zero-length/empty-range helper contracts, materialized-view parity tests for helper semantics, direct packed-helper coverage for consuming `Trace::into_blocks`, cross-chunk/materialized parity coverage for packed physical predicates and `SyncTrace::boxplus_tight`, panic-contract coverage for `min_layers_for_mass` / `min_tau_for_mass` when `kappa == 0`, slice-facing compatibility SIMD helpers that now reuse transient packed chunks instead of AoS gather per chunk, direct packed-finalization parity/tail-zeroing coverage for split-free long-middle `otimes_kappa`, direct packed-finalization parity/tail-zeroing coverage for `Trace::compose`, a tighter canonical `Trace::new` full-chunk packing path via fixed-width chunk copy, and exhaustive boundary-length packing parity/zero-padding proof for `Trace::new` across lengths `0..=17`. Authoritative `baseline-smt-off` и topology reports остаются в `target/bench-policy/topology-real-report.json` и `target/bench-policy/topology-smt-on-report.json`.
Текущий perf baseline интерпретируется как `Ryzen 9 7950X / SMT-off / physical-cores-only`; SMT-on measurements фиксируются отдельно (`target/bench-policy/topology-smt-on-report.json`, `geomean_delta_pct = -0.575` vs `baseline-smt-off`), а `tools\check.ps1` / `tools\ci.ps1` now auto-detect the current host topology and choose the correct verification wave.
`tools\bench_policy.ps1` now emits baseline compatibility files plus run-scoped affinity artifacts under `target/bench-policy/runs/<wave>/<affinity-class>/<run-id>/`, including `run.json`, `fingerprint.json`, `topology.json`, `machine.json`, bench logs/results, and optional `perf.pre/post.json` snapshots.
`cargo package --list` уже ограничен release surface из `Cargo.toml`; Cargo по-прежнему добавляет служебные `Cargo.lock` и `Cargo.toml.orig` artifacts.
The supported external Rust surface is the root reexports plus `ts4::prelude::*`; internal implementation details remain out of contract, `Trace.blocks` is no longer a public downstream field, `Trace` hides its storage behind an accessor-based contract, internal callers already use boundary/range helpers instead of direct raw indexing, and `TraceStorage` is canonical packed-first with a private packed `Chunk8` cache plus lazy compatibility view without changing the downstream API. The checked-in downstream fixture now follows this contract directly through `ts4::prelude::*` and prefers `Trace` methods over slice-facing SIMD helpers.

## Toolchain and target contract

- [x] `rust-toolchain.toml` pinned на nightly
- [x] Target contract `znver4 + required ISA` enforceable технически; `Cargo.toml` фиксирует `rust-version = 1.85`
- [x] Consumer build story задокументирован на реальном downstream fixture (`consumer-fixture/Cargo.toml`, `src/main.rs`, `tests/smoke.rs`)

## Correctness

- [x] `cargo test --quiet` passes cleanly (`171/171` unit tests + `109/109` doctests in the last full run)
- [x] `cargo test --release --quiet` passes cleanly (`171/171` unit tests + `109/109` doctests in the last full run)
- [x] Doctests проходят (`109/109`)
- [x] `otimes_kappa` и `kappa`-слой покрыты exact-output fixtures на текущем checked-in contract surface
- [x] Divisibility/solver отрицательные случаи проверены на текущем checked-in contract surface

## Codegen and performance

- [x] asm/LLVM IR checks подтверждают ожидаемый Zen 4 codegen hot kernels
- [x] Bench harness защищён от dead-code elimination (`black_box` и fixed paired recovery workloads)
- [x] `cargo bench --bench basic` прошёл в last green run and remains the documented paired recovery baseline
- [x] Benchmark baseline снят на реальном 7950X under `baseline-smt-off`; SMT-on comparative wave выполнен отдельно (см. `target/bench-policy/topology-smt-on-report.json`).
- [x] Topology-aware same-CCD / cross-CCD runs выполняются отдельно от baseline и пишут реальные pinned run artifacts
- [x] Optional perf-counter capture и machine snapshots теперь привязаны к topology-wave runs
- [x] `tools\compare_basic_results.ps1 -TopologyManifestPath <manifest> -OutReportPath <report.json>` строит единый baseline/same-CCD/cross-CCD comparative report (table + geomean + machine-readable JSON); current checkout already has `target\bench-policy\topology-real-report.json`
- [x] Release sign-off policy для SMT-off topology report зафиксирован: report обязан содержать полный bench set без missing rows; `same_avg_vs_base_geomean_pct`, `cross_vs_base_geomean_pct`, и `cross_vs_same_avg_geomean_pct` должны оставаться не хуже `+5%`; любой отдельный hot-path bench хуже `+10%` требует явного комментария в sign-off
- [x] Release sign-off already has a machine-readable comparative report: `target\bench-policy\topology-real-report.json` (`same_avg_vs_base_geomean_pct = -9.206`, `cross_vs_base_geomean_pct = -12.772`, `cross_vs_same_avg_geomean_pct = -3.928`)
- [x] Отдельный SMT-on comparative wave выполнен (artifacts: `target/bench-policy/topology-smt-on-manifest.json`, `target/bench-policy/topology-smt-on-report.json`; `topology_wave=smt-on-comparative`, `affinity_class=smt-on-unpinned`, `benches_compared=30`, `improved=16`, `worsened=14`, `geomean_delta_pct=-0.575` vs `baseline-smt-off`).

## Docs and package

- [x] `cargo doc --no-deps` проходит без warnings
- [x] Hardcoded local paths убраны из docs/scripts
- [x] README и usage docs синхронизированы с checkout-relative docs/tooling contract и runnable consumer path
- [x] consumer-fixture docs linkage подтверждена в release/docs narrative
- [x] consumer-fixture now exercises the documented downstream import contract through `ts4::prelude::*`
- [x] downstream and consumer-fixture no longer require direct access to `Trace.blocks`
- [x] `Trace` storage is internally wrapped and no longer tied to a direct public `Vec<Block>` field contract
- [x] `Trace` now exposes first-class packed-first reduction/projection helpers: `mass_l1()`, `blocks_l1_gt_mask(kappa)`, and `pi()`
- [x] `TraceStorage` is canonical packed-first with a private packed `Chunk8` cache; `phi_kappa` / `parallel_kappa` plus reduction layer (`mass_l1`, `pi_trace`) already use it for trace-owned full-chunk paths; `odot_kappa` range-segments and `otimes_kappa` state-machine now reuse the same packed-first seam both for `v_len == 2` and for interleaved long-middle `v_len > 2`, while the split-free long-middle `otimes_kappa` path and `Trace::compose` now also finalize directly into packed chunks without a `Trace::new` repack; `blocks_l1_gt_mask` now exposes a first-class packed `BlockMask` while `blocks_l1_gt` remains a compatibility wrapper
- [x] `cargo package --list` показывает только намеренные release files
- [ ] Manifest metadata полностью заполнен: repository, homepage

## Что ещё не закончено

- [x] Концепт `docs/4d_sections_clean.txt` закрыт по разделам `0–37`.
- [x] Финальный packed-first/data-layout closure доведён до checked-in proof/data-layout contract.
- [ ] Manifest metadata всё ещё не полон: нужны подтверждённые `repository` и `homepage`, если они реально существуют.

Residual blockers: verified manifest metadata only.
