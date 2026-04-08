//! Fixed CPU policy for the Zen 4-only build.
//!
//! This module owns the compile-time target contract. The crate is only
//! expected to build on Windows MSVC x86_64 with Zen 4-class ISA enabled and a
//! pinned nightly toolchain. There is no runtime capability negotiation and no
//! compatibility surface for alternate CPUs.

#[cfg(not(target_arch = "x86_64"))]
compile_error!("ts4 targets AMD Zen 4 x86_64 only");

#[cfg(all(
    not(doc),
    target_arch = "x86_64",
    not(all(target_os = "windows", target_env = "msvc"))
))]
compile_error!(
    "ts4 requires the Windows MSVC target. Build on x86_64-pc-windows-msvc with the pinned nightly."
);

#[cfg(all(
    not(doc),
    target_arch = "x86_64",
    target_os = "windows",
    target_env = "msvc",
    not(all(
        target_feature = "avx2",
        target_feature = "avx512f",
        target_feature = "avx512vl",
        target_feature = "avx512bw",
        target_feature = "avx512cd",
        target_feature = "avx512dq",
        target_feature = "avx512vbmi",
        target_feature = "avx512vnni",
        target_feature = "avx512bitalg",
        target_feature = "avx512vpopcntdq",
        target_feature = "bmi2",
        target_feature = "fma",
        target_feature = "gfni",
        target_feature = "lzcnt",
        target_feature = "popcnt"
    ))
))]
compile_error!(
    "ts4 requires Zen 4-class ISA with AVX-512VL-first execution, BMI2, FMA3, GFNI, LZCNT, and POPCNT. Build with Rust nightly and `-C target-cpu=znver4`."
);
