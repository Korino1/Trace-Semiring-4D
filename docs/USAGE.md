# Руководство по использованию TS4 (Zen 4-only)

Ниже — короткие примеры для основных публичных методов/функций, которые входят в Zen 4-facing поверхность библиотеки.

## Runnable consumer fixture
- Реальный downstream path находится в `consumer-fixture/`.
- `cargo run --manifest-path consumer-fixture/Cargo.toml` запускает `consumer-fixture/src/main.rs` и печатает поля smoke report.
- `cargo test --manifest-path consumer-fixture/Cargo.toml --quiet` выполняет `consumer-fixture/tests/smoke.rs` и проверяет exact contract.

## Contract
- This checkout expects the repository's pinned nightly, `.cargo/config.toml`, and the `znver4` codegen contract.
- `Cargo.toml` declares `rust-version = "1.85"` as the metadata floor; the operational toolchain contract remains the pinned nightly.
- The crate is Zen 4-only: there is no scalar fallback and no runtime capability dispatch surface.
- `AVX-512VL` is the code-proven baseline vector policy; `BMI2`, `FMA3`, `GFNI`, `LZCNT`, and `POPCNT` remain part of the Zen 4 target contract and should only be treated as implemented when a hot kernel and benchmark explicitly demonstrate them.
- Downstream code should use `use ts4::prelude::*;` or root reexports only. Module-path imports (for example `ts4::trace::Trace`) are intentionally out of contract.
- The checked-in release-grade verification surface is `consumer-fixture/`, `cargo test --quiet`, `cargo test --release --quiet`, `cargo test --doc --quiet`, `cargo doc --no-deps`, `cargo package --list`, and the benchmark policy scripts under `tools/`.
- `Trace.blocks` is not part of the downstream contract. Treat `Trace` as an accessor-based type and rely on the public API rather than internal storage details.
- `Trace` now exposes first-class helper methods on the type itself: `tau(n)`, `mass_l1()`, `blocks_l1_gt_mask(kappa)`, and `pi()`.
- Free functions `sum_l1_blocks`, `blocks_l1_gt`, and `blocks_l1_gt_mask` remain available for raw `&[Block]` compatibility, but checked-in downstream code should prefer the `Trace` methods when it already owns a `Trace`.
- Sections `20–22` of the concept now have a first-class surface through `SyncTrace`, `is_kappa_admissible`, `is_tight_core`, `boxplus`, `boxplus_tight`, `sequential`, `time_refine`, and `successor_t/x/y/z`.
- Sections `31–37` of the concept now have a first-class theory/morphism surface through `normalize_trace`, `normalize_ts4`, `traces_equal_by_normal_form`, `ts4_equal_by_normal_form`, `ts4_semiring_laws_hold`, `ts4_noncommutative_example`, `trace_left_cancellation_holds`, `is_trace_atom`, `monomial_left_divides`, `left_divide_trace_is_exact`, `pi_trace_compose_morphism_holds`, and `proj_r4_matches_scaled_pi`.
- `TS4.terms`, `TS4Mod.terms`, and direct `TS4Mod.modulus` field access are not part of the downstream contract. Use accessor methods (`term_count`, `coeff_sum`, `get_coeff`, `iter`, `modulus()`) instead.
- The current benchmark baseline is SMT-off on physical cores only; use `tools\run_topology_waves.ps1` for same-CCD/cross-CCD proxy waves and keep those results separate from the authoritative baseline.

## 1) Block

### Block::new
```rust
use ts4::Block;
let b = Block::new(1,2,3);
```

### Block::zero
```rust
use ts4::Block;
let z = Block::zero();
```

### Block::l1
```rust
use ts4::Block;
let b = Block::new(1,2,3);
let s = b.l1();
```

### Block::add
```rust
use ts4::Block;
let a = Block::new(1,0,0);
let b = Block::new(0,1,0);
let c = a.add(b);
```

### Block::sub
```rust
use ts4::Block;
let a = Block::new(3,2,1);
let b = Block::new(1,1,1);
let c = a.sub(b);
```

## 2) Trace

### Trace::new
```rust
use ts4::{Block, Trace};
let t = Trace::new(vec![Block::new(1,0,0), Block::zero()]);
```

### Trace::empty
```rust
use ts4::Trace;
let e = Trace::empty();
```

### Trace::tau
```rust
use ts4::Trace;
let t = Trace::tau(3);
```

### Trace::len_blocks
```rust
use ts4::{Block, Trace};
let t = Trace::new(vec![Block::new(1,0,0), Block::zero()]);
let n = t.len_blocks();
```

### Trace::tau_count
```rust
use ts4::{Block, Trace};
let t = Trace::new(vec![Block::new(1,0,0), Block::zero()]);
let k = t.tau_count();
```

### Trace::compose
```rust
use ts4::{Block, Trace};
let a = Trace::new(vec![Block::new(1,0,0), Block::zero()]);
let b = Trace::new(vec![Block::new(0,1,0)]);
let c = a.compose(&b);
```

### Trace::mass_l1
```rust
use ts4::{Block, Trace};
let t = Trace::new(vec![Block::new(1,2,3), Block::zero()]);
let s = t.mass_l1();
```

### Trace::blocks_l1_gt_mask
```rust
use ts4::{Block, Trace};
let t = Trace::new(vec![Block::new(2,0,0), Block::new(0,0,0)]);
let m = t.blocks_l1_gt_mask(1);
```

### Trace::pi
```rust
use ts4::{Block, Trace};
let t = Trace::new(vec![Block::new(1,2,3), Block::zero()]);
let p = t.pi();
```

### Trace::pad_to
```rust
use ts4::{Block, Trace};
let t = Trace::new(vec![Block::new(1,0,0)]);
let p = t.pad_to(3);
```

### Trace::from_word
```rust
use ts4::Trace;
let t = Trace::from_word("txty");
```

## 3) TS4

### TS4::zero
```rust
use ts4::TS4;
let z = TS4::zero();
```

### TS4::one
```rust
use ts4::TS4;
let o = TS4::one();
```

### TS4::from_trace
```rust
use ts4::{Block, Trace, TS4};
let t = Trace::new(vec![Block::new(1,0,0)]);
let a = TS4::from_trace(t, 3);
```

### TS4::add
```rust
use ts4::{Block, Trace, TS4};
let t1 = Trace::new(vec![Block::new(1,0,0)]);
let t2 = Trace::new(vec![Block::new(0,1,0)]);
let a = TS4::from_trace(t1, 3);
let b = TS4::from_trace(t2, 2);
let c = a.add(&b);
```

### TS4::compose
```rust
use ts4::{Block, Trace, TS4};
let t1 = Trace::new(vec![Block::new(1,0,0)]);
let t2 = Trace::new(vec![Block::new(0,1,0)]);
let a = TS4::from_trace(t1, 3);
let b = TS4::from_trace(t2, 2);
let c = a.compose(&b);
```

### TS4::normalize
```rust
use ts4::{Block, Trace, TS4};
let t = Trace::new(vec![Block::new(1,0,0)]);
let mut a = TS4::from_trace(t, 0);
a.normalize();
```

## 4) κ‑mode helpers

### split_block_kappa
```rust
use ts4::{Block, split_block_kappa};
let parts = split_block_kappa(Block::new(5,0,0), 4);
```

### phi_kappa
```rust
use ts4::{Block, Trace, phi_kappa};
let t = Trace::new(vec![Block::new(5,0,0)]);
let p = phi_kappa(&t, 4);
```

### odot_kappa
```rust
use ts4::{Block, Trace, odot_kappa};
let u = Trace::new(vec![Block::new(3,0,0)]);
let v = Trace::new(vec![Block::new(2,0,0)]);
let r = odot_kappa(&u, &v, 4);
```

### parallel_kappa
```rust
use ts4::{Block, Trace, parallel_kappa};
let u = Trace::new(vec![Block::new(1,0,0)]);
let v = Trace::new(vec![Block::new(0,1,0)]);
let r = parallel_kappa(&u, &v, 4);
```

### otimes_kappa
```rust
use ts4::{Block, Trace, otimes_kappa};
let u = Trace::new(vec![Block::new(1,0,0), Block::zero()]);
let v = Trace::new(vec![Block::new(0,1,0)]);
let r = otimes_kappa(&u, &v, 4);
```

## 4a) physical layer

### SyncTrace and boxplus
```rust
use ts4::{Block, Trace, SyncTrace};
let a = SyncTrace::new(Trace::new(vec![Block::new(1,0,0)]), 4).unwrap();
let b = SyncTrace::new(Trace::new(vec![Block::new(0,1,0)]), 4).unwrap();
let c = a.boxplus(&b);
```

### SyncTrace successors
```rust
use ts4::{SyncTrace, Trace};
let z = SyncTrace::new(Trace::tau(1), 2).unwrap();
let next_t = z.successor_t();
let next_x = z.successor_x();
```

## 4b) theory layer

### normalize_trace / equality by normal form
```rust
use ts4::{Block, Trace, normalize_trace, traces_equal_by_normal_form};
let a = Trace::new(vec![Block::new(1,0,0), Block::zero()]);
let b = normalize_trace(&a);
let same = traces_equal_by_normal_form(&a, &b);
```

### semiring law / noncommutativity witnesses
```rust
use ts4::{Block, Trace, TS4, ts4_noncommutative_example, ts4_semiring_laws_hold};
let a = TS4::from_trace(Trace::new(vec![Block::new(1,0,0)]), 1);
let b = TS4::from_trace(Trace::tau(1), 1);
let c = TS4::from_trace(Trace::new(vec![Block::new(0,1,0)]), 1);
let ok = ts4_semiring_laws_hold(&a, &b, &c);
let noncomm = ts4_noncommutative_example();
```

### morphism helpers
```rust
use ts4::{Block, Trace, pi_trace_compose_morphism_holds, proj_r4_matches_scaled_pi};
let a = Trace::new(vec![Block::new(1,0,0)]);
let b = Trace::tau(1);
let morphism_ok = pi_trace_compose_morphism_holds(&a, &b);
let scaled_ok = proj_r4_matches_scaled_pi(&a, 0.5, 2.0);
```

## 5) divisibility

### left_divide_trace / right_divide_trace
```rust
use ts4::{Block, Trace, left_divide_trace, right_divide_trace};
let a = Trace::new(vec![Block::new(1,0,0), Block::zero()]);
let b = Trace::new(vec![Block::new(1,0,0), Block::new(0,1,0)]);
let c = left_divide_trace(&a, &b).unwrap();
let d = right_divide_trace(&Trace::new(vec![Block::new(0,1,0)]), &b).unwrap();
```

### left_divide_monomial / right_divide_monomial
```rust
use ts4::{Block, Trace, left_divide_monomial, right_divide_monomial};
let t = Trace::new(vec![Block::new(1,0,0)]);
let u = Trace::new(vec![Block::new(1,0,0), Block::new(0,1,0)]);
let (c, tr) = left_divide_monomial(3, &t, 12, &u).unwrap();
let (c2, tr2) = right_divide_monomial(3, &Trace::new(vec![Block::new(0,1,0)]), 12, &u).unwrap();
```

### left_divide_ts4_monomial / right_divide_ts4_monomial
```rust
use ts4::{Block, Trace, TS4, left_divide_ts4_monomial};
let t = Trace::new(vec![Block::new(1,0,0)]);
let u = Trace::new(vec![Block::new(1,0,0), Block::new(0,1,0)]);
let a = TS4::from_trace(t, 2);
let b = TS4::from_trace(u, 4);
let c = left_divide_ts4_monomial(&a, &b).unwrap();
```

### left_divide_ts4_unique
```rust
use ts4::{Block, Trace, TS4, left_divide_ts4_unique};
let a = TS4::from_trace(Trace::new(vec![Block::new(1,0,0)]), 2);
let b = TS4::from_trace(Trace::new(vec![Block::new(1,0,0), Block::new(0,1,0)]), 4);
let c = left_divide_ts4_unique(&a, &b).unwrap();
```

### left_divide_ts4_solve / left_divide_ts4 (hybrid)
```rust
use ts4::{Block, Trace, TS4, left_divide_ts4_solve, left_divide_ts4, SolveMode};
let ax = TS4::from_trace(Trace::new(vec![Block::new(1,0,0)]), 1);
let ay = TS4::from_trace(Trace::new(vec![Block::new(0,1,0)]), 1);
let a = ax.add(&ay);
let b = TS4::from_trace(Trace::new(vec![Block::new(1,1,0)]), 2);
let c = left_divide_ts4_solve(&a, &b, 8, 1).unwrap();
let d = left_divide_ts4(&a, &b, SolveMode::Unbounded { max_solutions: 1 }).unwrap();
```

### left_gcd_trace
```rust
use ts4::{Block, Trace, left_gcd_trace};
let a = Trace::new(vec![Block::new(2,0,0)]);
let b = Trace::new(vec![Block::new(1,0,0)]);
let g = left_gcd_trace(&a, &b);
```

## 6) TS4Mod and GCD helpers

### TS4Mod::new
```rust
use ts4::TS4Mod;
let m = TS4Mod::new(7);
```

### TS4Mod::from_trace
```rust
use ts4::{Block, Trace, TS4Mod};
let t = Trace::new(vec![Block::new(1,0,0)]);
let m = TS4Mod::from_trace(t, 3, 5);
```

### TS4Mod::add
```rust
use ts4::{Block, Trace, TS4Mod};
let t = Trace::new(vec![Block::new(1,0,0)]);
let a = TS4Mod::from_trace(t.clone(), 3, 5);
let b = TS4Mod::from_trace(t, 4, 5);
let c = a.add(&b);
```

### TS4Mod::compose
```rust
use ts4::{Block, Trace, TS4Mod};
let t = Trace::new(vec![Block::new(1,0,0)]);
let a = TS4Mod::from_trace(t.clone(), 3, 5);
let b = TS4Mod::from_trace(t, 4, 5);
let c = a.compose(&b);
```

### gcd_u32 / gcd_monomial
```rust
use ts4::{Block, Trace, gcd_u32, gcd_monomial};
let g = gcd_u32(12, 8);
let t1 = Trace::new(vec![Block::new(2,0,0)]);
let t2 = Trace::new(vec![Block::new(1,0,0)]);
let (c, t) = gcd_monomial(6, &t1, 10, &t2);
```

## 7) Invariants

### mass_l1 / layers / tau_count
```rust
use ts4::{Block, Trace, mass_l1, layers, tau_count};
let t = Trace::new(vec![Block::new(1,2,3), Block::zero()]);
let m = mass_l1(&t);
let l = layers(&t);
let tau = tau_count(&t);
```

### min_layers_for_mass / min_tau_for_mass
```rust
use ts4::{min_layers_for_mass, min_tau_for_mass};
let l = min_layers_for_mass(10, 4);
let t = min_tau_for_mass(10, 4);
```

## 8) Projections

### pi_trace / pi_ts4 / proj_r4
```rust
use ts4::{Block, Trace, TS4, pi_trace, pi_ts4, proj_r4};
let t = Trace::new(vec![Block::new(1,2,3), Block::zero()]);
let p = pi_trace(&t);
let a = TS4::from_trace(t.clone(), 2);
let p2 = pi_ts4(&a);
let r = proj_r4(&t, 0.5, 2.0);
```

## 9) SIMD helpers

### sum_l1_blocks
```rust
use ts4::{Block, sum_l1_blocks};
let blocks = vec![Block::new(1,2,3), Block::new(4,0,0)];
let s1 = sum_l1_blocks(&blocks);
```

### blocks_l1_gt
```rust
use ts4::{Block, blocks_l1_gt};
let blocks = vec![Block::new(1,1,1), Block::new(5,0,0)];
let mask = blocks_l1_gt(&blocks, 3);
```

### blocks_l1_gt_mask
```rust
use ts4::{Block, blocks_l1_gt_mask};
let blocks = vec![Block::new(1,1,1), Block::new(5,0,0)];
let mask = blocks_l1_gt_mask(&blocks, 3);
assert_eq!(mask.as_words(), &[0b10]);
```
