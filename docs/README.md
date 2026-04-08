# Документация TS4

Этот набор документов следует той же Zen 4-only политике, что и библиотека: fallback и scalar-only режимы не поддерживаются.
Поддерживаемая downstream-поверхность для внешнего кода — root reexports и `ts4::prelude::*`; внутренние implementation details не считаются частью public contract.
Текущий release-grade сценарий проверяется через `consumer-fixture/`, `cargo test --quiet`, `cargo test --release --quiet`, `cargo test --doc --quiet`, `cargo doc --no-deps` и `cargo package --list` в этом checkout.

Этот каталог содержит синхронизированный текст спецификации 4D:
- `4d_sections_clean.txt` — разделы спецификации 0–37, синхронизированный snapshot в этом checkout.

Правило синхронизации: при изменении upstream-spec использовать `tools\sync_docs.ps1 -SourcePath <path-to-4d_sections_clean.txt>`.
Topology/perf helper для операторских proxy-wave запусков: `tools\run_topology_waves.ps1`. Он предназначен для same-CCD и cross-CCD интерпретаций, а baseline `baseline-smt-off` остаётся авторитетным источником основных цифр.

Consumer story для внешнего разработчика привязан к реальному `consumer-fixture/` в этом checkout:
- `consumer-fixture/Cargo.toml` подключает `ts4` по path и не публикуется.
- `cargo run --manifest-path consumer-fixture/Cargo.toml` запускает runnable example.
- `cargo test --manifest-path consumer-fixture/Cargo.toml --quiet` прогоняет exact-output smoke test из `consumer-fixture/tests/smoke.rs`.

Скрипт генерации rustdoc: `tools\gen_rustdoc.ps1`
- по умолчанию читает `docs\4d_sections_clean.txt` и пишет `docs\RS_DOC.md`
- принимает `-SpecPath` и `-OutPath`, чтобы не зависеть от локальных абсолютных путей

- `FORMULAS.md` — краткий справочник формул
- `TUTORIAL.md` — минимальный туториал
- `USAGE.md` — как пользоваться Zen 4-facing поверхностью API

Эти документы входят в curated release surface и должны оставаться синхронизированы с `Cargo.toml`, `README.md`, и `src/docs.rs`.
Если в документацию попадает новый внешний contract, он должен быть подтверждён реальным downstream crate или реальной проверкой на текущем checkout.
