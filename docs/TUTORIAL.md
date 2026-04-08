# Туториал (минимум)

Все примеры ниже предполагают checkout с pinned nightly и `znver4` target contract; fallback-пути отсутствуют.
Сборка и doc story опираются на `.cargo/config.toml`, а не на ручной локальный target override.

## 0) Реальный consumer path
```powershell
cargo run --manifest-path consumer-fixture/Cargo.toml
cargo test --manifest-path consumer-fixture/Cargo.toml --quiet
```
Этот downstream fixture живёт в `consumer-fixture/`: `src/main.rs` печатает smoke report, а `tests/smoke.rs` проверяет exact-output контракт.

## 1) Создание трассы
```rust
use ts4::{Block, Trace};
let t = Trace::new(vec![Block::new(1,0,0), Block::zero()]); // x τ
```

## 2) Композиция
```rust
let u = Trace::new(vec![Block::new(0,1,0)]); // y
let v = t.compose(&u); // x τ y
```

## 3) TS4 и коэффициенты
```rust
use ts4::TS4;
let a = TS4::from_trace(t, 3);
let b = TS4::from_trace(u, 2);
let c = a.compose(&b); // 6 * (x τ y)
```

## 4) κ‑режим
```rust
use ts4::odot_kappa;
let r = odot_kappa(&v, &u, 4);
```
