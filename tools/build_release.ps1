# Release build (Windows, Zen 4-only)
$env:RUSTFLAGS="-C target-cpu=znver4"
cargo build --release