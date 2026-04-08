# TS4 (4D Trace Semiring) — Zen 4-only Rust library

## Policy
This crate targets AMD Zen 4 / Ryzen 9 7950X class CPUs only. It is maintained as a Rust nightly, edition 2024, unsafe-oriented codebase. Fallback and scalar-only modes are intentionally unsupported.
The repository checkout is expected to run under the pinned nightly recorded in `rust-toolchain.toml` and the `znver4` codegen contract.
The supported downstream surface is the root reexports and `ts4::prelude::*`; module-path imports such as `ts4::trace::Trace` are intentionally out of contract, and implementation modules remain private.
Representation fields of `TS4`/`TS4Mod` (`terms`, `modulus`) are not part of the downstream contract. Use accessor methods such as `term_count`, `coeff_sum`, `get_coeff`, `iter`, and `modulus()` instead of direct field access.
`Trace` is also accessor-based: use methods such as `tau`, `as_blocks`, `into_blocks`, `mass_l1`, `blocks_l1_gt_mask`, and `pi` instead of depending on storage layout or slice-only helper entrypoints. Free functions such as `sum_l1_blocks`, `blocks_l1_gt`, and `blocks_l1_gt_mask` remain as compatibility helpers for raw `&[Block]` inputs, but the checked-in downstream contract prefers the `Trace` methods. For the first-class physical class layer from sections `20–22`, use `SyncTrace` plus `is_kappa_admissible` / `is_tight_core` and its `boxplus` / `boxplus_tight` / `sequential` / `time_refine` / successor methods. Sections `31–37` are now also exposed through the first-class theory layer: `normalize_trace`, `normalize_ts4`, `traces_equal_by_normal_form`, `ts4_equal_by_normal_form`, `ts4_semiring_laws_hold`, `ts4_noncommutative_example`, `trace_left_cancellation_holds`, `is_trace_atom`, `monomial_left_divides`, `left_divide_trace_is_exact`, `pi_trace_compose_morphism_holds`, and `proj_r4_matches_scaled_pi`.

The release-grade package contract is intentionally narrower than the implementation surface. Current consumer-facing verification is anchored by `consumer-fixture/`, `cargo test --quiet`, `cargo test --release --quiet`, `cargo test --doc --quiet`, `cargo doc --no-deps`, and `cargo package --list` on the checked-in tree.

## Specification
- `docs/4d_sections_clean.txt` — synchronized 4D specification (sections 0-37).

## Requirements
- Rust nightly (pinned via `rust-toolchain.toml`), edition 2024, with `rust-version = "1.85"` metadata floor
- Windows (MSVC toolchain)
- AMD Zen 4 / `znver4`

## Build (Windows)
Run these commands from the repository root.
```powershell
cargo build --release
```
The build contract is supplied by `.cargo/config.toml`, which pins `x86_64-pc-windows-msvc` and `-C target-cpu=znver4`.

## Zen 4 build policy
Use the repository's pinned nightly and the `znver4` target contract for release, check, and CI runs. The currently code-proven hot path is Zen 4-oriented through `AVX-512VL` mask semantics and a 256-bit datapath; the broader Zen 4 ISA envelope (`AVX-512F`, `FMA3`, `BMI2`, `GFNI`, `LZCNT`, `POPCNT`) is reserved by contract and should only be claimed in hot kernels when the implementation and benchmarks explicitly prove it.
On Ryzen 9 7950X, benchmark policy should account for the two CCDs, separate L3 domains, and locality-sensitive placement when interpreting results.

## Docs and tooling
- `tools\gen_rustdoc.ps1` generates `docs/RS_DOC.md` from the checked-in spec snapshot and accepts explicit `-SpecPath` / `-OutPath` overrides.
- `tools\sync_docs.ps1` refreshes `docs/4d_sections_clean.txt` from an explicit upstream source path; pass `-SourcePath` instead of relying on a local drive layout.
- `tools\run_topology_waves.ps1` is the operator-triggered helper for same-CCD and cross-CCD pinned waves; it keeps `baseline-smt-off` authoritative, resolves affinity from the host NUMA/CCD layout, and writes run-scoped artifacts under `target/bench-policy/runs/<wave>/<affinity-class>/<run-id>/`.
- Topology-wave manifests such as `target/bench-policy/topology-real-manifest.json` include `run.json`, `fingerprint.json`, `topology.json`, `machine.json`, bench logs/results, and optional Windows OS perf-counter snapshots for each run.
- `cargo package --list` should only expose the curated release surface declared in `Cargo.toml`.
  Cargo still emits `Cargo.lock` and `Cargo.toml.orig`, so the package list is not a pure manifest mirror.

Verified manifest facts currently exposed by `Cargo.toml` are the crate name, version, edition, `rust-version`, description, README, docs.rs link, license, keywords, categories, curated `include` set, and release-profile settings. No repository/homepage field is claimed in this checkout.

## Runnable consumer fixture
- `consumer-fixture/` is a real downstream crate in this checkout. It depends on `ts4` by path and keeps `publish = false`.
- Run the fixture from the repository root with `cargo run --manifest-path consumer-fixture/Cargo.toml`.
- Verify the exact downstream contract with `cargo test --manifest-path consumer-fixture/Cargo.toml --quiet`.
- The runnable entrypoint is `consumer-fixture/src/main.rs`; the exact-output assertions live in `consumer-fixture/tests/smoke.rs`.

## Usage
```rust
use ts4::prelude::*;

let a = Trace::new(vec![Block::new(1,0,0), Block::zero()]); // x τ
let b = Trace::new(vec![Block::new(0,1,0)]);                // y
let c = a.compose(&b);                                      // x τ y

let ta = TS4::from_trace(a, 3);
let tb = TS4::from_trace(b, 2);
let tc = ta.compose(&tb); // 6 * (x τ y)
```

## Checks
```powershell
.\tools\check.ps1
```

## Release build
```powershell
.\tools\build_release.ps1
```

## rustdoc
```powershell
.\tools\gen_rustdoc.ps1
```

## Local CI
```powershell
.\tools\ci.ps1
```

## Criterion bench
```powershell
cargo bench --bench criterion
```
