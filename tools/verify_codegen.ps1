# Zen 4-only codegen verification for hot kernels.
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path $PSScriptRoot -Parent
Set-Location $repoRoot
$env:RUSTFLAGS = "-C target-cpu=znver4"

cargo rustc --lib --release -- --emit=asm,llvm-ir

$asm = Get-ChildItem -Path 'target/release/deps' -Filter 'ts4-*.s' |
    Sort-Object LastWriteTime -Descending |
    Select-Object -First 1
$ll = Get-ChildItem -Path 'target/release/deps' -Filter 'ts4-*.ll' |
    Sort-Object LastWriteTime -Descending |
    Select-Object -First 1

if (-not $asm) {
    throw 'asm verification failed: no ts4 assembly artifact was produced.'
}

if (-not $ll) {
    throw 'llvm-ir verification failed: no ts4 LLVM IR artifact was produced.'
}

$asmText = Get-Content -Raw $asm.FullName
$llText = Get-Content -Raw $ll.FullName

$requiredAsm = @(
    'sum_l1_blocks_zen4',
    'blocks_l1_gt_zen4',
    'vpaddd',
    'vpinsrd',
    'vinserti128',
    'vpcmpgtd',
    'kmovd'
)
foreach ($needle in $requiredAsm) {
    if (-not $asmText.Contains($needle)) {
        throw "asm verification failed: missing '$needle' in $($asm.FullName)"
    }
}

$requiredLl = @(
    'sum_l1_blocks_zen4',
    'blocks_l1_gt_zen4',
    'target-features',
    '+avx2',
    '+avx512f',
    '+avx512vl'
)
foreach ($needle in $requiredLl) {
    if (-not $llText.Contains($needle)) {
        throw "llvm-ir verification failed: missing '$needle' in $($ll.FullName)"
    }
}

$forbidden = @('is_x86_feature_detected', '__cpuid', 'runtime dispatch')
foreach ($needle in $forbidden) {
    if ($asmText.Contains($needle) -or $llText.Contains($needle)) {
        throw "verification failed: forbidden runtime dispatch marker '$needle' found in codegen artifacts"
    }
}

Write-Host "Codegen verification passed."
Write-Host "ASM: $($asm.FullName)"
Write-Host "IR : $($ll.FullName)"
