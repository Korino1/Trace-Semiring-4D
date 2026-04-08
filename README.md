# TS4

`ts4` — это Rust-библиотека для работы с 4D trace semiring.

Главная идея библиотеки такая:

**TS4 позволяет представить сложные объекты и процессы как 4D-трассы, чтобы выявлять структурные закономерности между ними, если такие закономерности действительно существуют в 4D-модели.**

Практический смысл в том, что библиотека помогает вынести тяжёлые вычисления в более строгий уровень абстракции. Вместо работы только с сырыми низкоуровневыми вычислениями можно работать через:
- композицию трасс,
- нормализацию,
- делимость,
- проекции,
- физическую допустимость,
- алгебраические законы и инварианты.

TS4 не "угадывает" закономерности сам по себе. Он даёт формальную систему, в которой скрытые связи, повторяемые формы и инварианты можно выразить, проверить и вычислить.

## Что это такое

На базовом уровне библиотека даёт:
- `Block` — один пространственный шаг `(x, y, z)`
- `Trace` — последовательность блоков, разделённых временными шагами `τ`
- `TS4` — формальную сумму трасс с целыми коэффициентами
- `κ`-операции: `phi_kappa`, `parallel_kappa`, `odot_kappa`, `otimes_kappa`
- physical layer и theory layer для нормализации, законов, делимости, морфизмов и связанных проверок

Эта библиотека нужна не для "общей алгебры вообще", а именно для работы с моделью TS4 как с самостоятельным формальным аппаратом.

## Для чего библиотека полезна

Если в твоей задаче есть 4D-структура, библиотека позволяет:
- представлять объекты как трассы и суммы трасс;
- композиционно собирать сложные процессы;
- искать инварианты и повторяемые формы;
- сравнивать объекты не только по числам, но и по структуре;
- выносить тяжёлые вычисления в более компактную алгебраическую форму;
- проверять, существуют ли между объектами закономерности, морфизмы или отношения делимости.

Именно это и есть основной смысл `TS4`: не заменить предметную модель, а дать строгий язык, в котором её структура становится вычислимой.

## Быстрый пример

```rust
use ts4::prelude::*;

let left = Trace::new(vec![Block::new(1, 0, 0), Block::zero()]); // x τ
let right = Trace::new(vec![Block::new(0, 1, 0)]);               // y

let composed = left.compose(&right); // x τ y
let mass = composed.mass_l1();
let mask = composed.blocks_l1_gt_mask(0);
let pi = composed.pi();

let a = TS4::from_trace(left, 2);
let b = TS4::from_trace(right, 3);
let c = a.compose(&b); // 6 * (x τ y)

assert_eq!(mass, 2);
assert_eq!(mask.get(0), true);
assert_eq!(pi, (2, 1, 1, 0));
assert_eq!(c.term_count(), 1);
assert_eq!(c.coeff_sum(), 6);
```

## Основные типы

### `Block`

`Block` — один 3D-шаг.

Основные методы:
- `Block::new(x, y, z)`
- `Block::zero()`
- `l1()`
- `add()`
- `sub()`
- аксессоры `x()`, `y()`, `z()`

### `Trace`

`Trace` — базовый time-aware объект библиотеки. Он представляет последовательность блоков, где нулевые блоки играют роль временных шагов `τ`.

Основные методы:
- `Trace::new(...)`
- `Trace::empty()`
- `Trace::tau(n)`
- `Trace::from_word(...)`
- `compose(...)`
- `mass_l1()`
- `blocks_l1_gt_mask(kappa)`
- `pi()`
- `as_blocks()`
- `into_blocks()`

### `TS4`

`TS4` — формальная сумма трасс с целыми коэффициентами.

Основные методы:
- `TS4::zero()`
- `TS4::one()`
- `TS4::from_trace(...)`
- `add(...)`
- `compose(...)`
- `normalize()`
- `term_count()`
- `coeff_sum()`
- `get_coeff(...)`
- `iter()`

### Physical Layer

Разделы концепта `20–22` представлены через:
- `SyncTrace`
- `is_kappa_admissible`
- `is_tight_core`
- `boxplus`
- `boxplus_tight`
- `sequential`
- `time_refine`
- `successor_t`, `successor_x`, `successor_y`, `successor_z`

### Theory Layer

Разделы концепта `31–37` представлены через:
- `normalize_trace`
- `normalize_ts4`
- `traces_equal_by_normal_form`
- `ts4_equal_by_normal_form`
- `ts4_semiring_laws_hold`
- `ts4_noncommutative_example`
- `trace_left_cancellation_holds`
- `is_trace_atom`
- `monomial_left_divides`
- `left_divide_trace_is_exact`
- `pi_trace_compose_morphism_holds`
- `proj_r4_matches_scaled_pi`

## Публичный контракт

Downstream-код должен использовать:
- root reexports, или
- `use ts4::prelude::*;`

Не стоит завязываться на private module paths вроде `ts4::trace::Trace`.

Также не стоит завязываться на внутреннее представление:
- `Trace` надо воспринимать как accessor-based тип, а не как конкретный storage layout
- `TS4.terms` не входит в поддерживаемый контракт
- `TS4Mod.terms` и прямой доступ к `TS4Mod.modulus` не входят в поддерживаемый контракт

Используй публичные методы:
- `Trace::tau`, `mass_l1`, `blocks_l1_gt_mask`, `pi`
- `TS4::term_count`, `coeff_sum`, `get_coeff`, `iter`
- `TS4Mod::modulus()`

## Требования к платформе и сборке

Это намеренно **не** переносимая "общая" Rust-библиотека.

Текущий контракт такой:
- Rust nightly, зафиксированный в [rust-toolchain.toml](rust-toolchain.toml)
- edition 2024
- Windows MSVC
- AMD Zen 4 / `znver4`
- Zen 4-only policy: без scalar fallback и без runtime dispatch surface

Checked-in hot path ориентирован на packed implementation под Zen 4 с `AVX-512VL` mask semantics и 256-bit datapath.

Сборка из корня репозитория:

```powershell
cargo build --release
```

Файл `.cargo/config.toml` задаёт checked-in `znver4` target contract.

## Текущее состояние

Этот checkout содержит реализованный концепт TS4 и его checked-in Zen 4-only библиотечную поверхность.

Текущий локально подтверждённый статус:
- `cargo test --quiet`: `171/171` unit tests, `109/109` doctests
- `cargo test --release --quiet`: passed
- `cargo doc --no-deps`: passed
- `cargo package --list`: passed

Единственный metadata-хвост в этом checkout — незаполненные `repository` / `homepage`, потому что здесь нет подтверждённого публичного URL.

## С чего начать

Если пользователь видит проект впервые, порядок такой:

1. Этот `README`
2. [docs/USAGE.md](docs/USAGE.md)
3. [consumer-fixture/](consumer-fixture/)
4. [docs/4d_sections_clean.txt](docs/4d_sections_clean.txt), если нужен полный текст концепта

Реальный downstream-пример лежит здесь:
- [consumer-fixture/src/main.rs](consumer-fixture/src/main.rs)
- [consumer-fixture/tests/smoke.rs](consumer-fixture/tests/smoke.rs)

Запуск:

```powershell
cargo run --manifest-path consumer-fixture/Cargo.toml
cargo test --manifest-path consumer-fixture/Cargo.toml --quiet
```

## Полезные команды

Проверки:

```powershell
.\tools\check.ps1
```

Локальный CI:

```powershell
.\tools\ci.ps1
```

Релизная сборка:

```powershell
.\tools\build_release.ps1
```

Синхронизация rustdoc:

```powershell
.\tools\gen_rustdoc.ps1
```

Criterion:

```powershell
cargo bench --bench criterion
```

Topology / CCD-aware прогоны:

```powershell
.\tools\run_topology_waves.ps1
```
