# Соответствие API разделам спецификации 4D

Источник: `docs/4d_sections_clean.txt`

Примечание: карта отражает текущую Zen 4-only реализацию. Векторные горячие участки ориентируются на `AVX-512VL` как базовый режим; `BMI2` и `FMA3` учитываются там, где они действительно полезны для ядра.
Итоговый статус по концепту: `docs/4d_sections_clean.txt` закрыт по разделам `0–37`. Незавершённые хвосты текущего checkout теперь технические, а не концептуальные.

## 0–8. Ядро TS4

- 1–2: `types::Block`, `trace::Trace`.
- 3: `Trace::new`, `Trace::compose`.
- 4–5: `ts4::TS4`, `TS4::add`, `TS4::compose`.
- 6–7: `TS4::from_trace`, `Trace::tau_count`, `projections::{pi_trace, pi_ts4}`.
- 8: `TS4::zero`, `TS4::one`.
- Статус: `done`.

## 9–15. Переход к физике

- 9: `Trace` как элемент `\mathbb N_4`.
- 10: `Trace::compose`.
- 11: каноническая временная цепочка через first-class `Trace::tau(n)`.
- 12: `projections::{pi_trace, pi_ts4, proj_r4}`.
- 13–15: `algorithms::{split_block_kappa, phi_kappa, odot_kappa, parallel_kappa, otimes_kappa}`.
- Статус: `done`.

## 16–22. Физический режим

- 16: `algorithms::{split_block_kappa, phi_kappa}`.
- 17: `invariants::{mass_l1, layers, tau_count, min_layers_for_mass, min_tau_for_mass}`.
- 18–19: `algorithms::{otimes_kappa, parallel_kappa}`.
- 20–22: `physical::{SyncTrace, is_kappa_admissible, is_tight_core}` плюс first-class successors `successor_t/x/y/z`.
- Статус: `done`.

## 23–30. Делимость, факторизация, модульность

- 23–27: `divisibility::{left_divide_trace, right_divide_trace, left_divide_monomial, right_divide_monomial, left_divide_ts4_monomial, right_divide_ts4_monomial, left_divide_ts4_unique, left_divide_ts4_solve, left_divide_ts4}`.
- 28–30: `modular::{TS4Mod, gcd_u32, gcd_monomial}`.
- Статус: `done`.

## 31–37. Теоремы, аксиоматика, морфизмы

- 31–32: `theory::{trace_left_cancellation_holds, is_trace_atom, monomial_left_divides, left_divide_trace_is_exact}`.
- 33–35: `theory::{normalize_trace, normalize_ts4, traces_equal_by_normal_form, ts4_equal_by_normal_form, ts4_semiring_laws_hold, ts4_noncommutative_example}`.
- 36–37: `theory::{pi_trace_compose_morphism_holds, proj_r4_matches_scaled_pi}`.
- Статус: `done`.

## Сопутствующие модули

- `cpu`: фиксированный Zen 4 capability descriptor для отчетности и архитектурного контекста.
- `simd`: Zen 4 fixed-path kernels для горячих операций.
- `prelude`: канонический публичный импорт.
- `docs`: rustdoc-склейка и документирующий слой.

## Что ещё не завершено по концепту

- Концепт `4d_sections_clean.txt` в текущем checkout закрыт по разделам `0–37`.
- Оставшиеся незавершённые пункты относятся уже к техническому Zen 4/data-layout/release closure, а не к отсутствующим концептуальным разделам.
