# Basic local checks (Zen 4-only)
[CmdletBinding()]
param(
    [switch]$RunTopologyWaves,
    [string[]]$TopologyWave = @('same-ccd-proxy', 'cross-ccd-proxy'),
    [string[]]$TopologyBenchName = @('basic'),
    [switch]$RunSmtOnComparativeWave,
    [switch]$CapturePerfCounters
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path $PSScriptRoot -Parent
Set-Location $repoRoot
$env:RUSTFLAGS = "-C target-cpu=znver4"

function Get-PrimaryTopologyWave {
    $cpu = Get-CimInstance Win32_Processor | Select-Object -First 1
    if (-not $cpu) {
        throw 'check.ps1 requires Win32_Processor metadata.'
    }

    $physicalCores = [int]$cpu.NumberOfCores
    $logicalProcessors = [int]$cpu.NumberOfLogicalProcessors
    if ($logicalProcessors -eq $physicalCores) {
        return 'baseline-smt-off'
    }
    if ($logicalProcessors -eq ($physicalCores * 2)) {
        return 'smt-on-comparative'
    }

    throw "unsupported topology for local checks: logical=$logicalProcessors physical=$physicalCores"
}

$primaryTopologyWave = Get-PrimaryTopologyWave
$primaryAffinityClass = if ($primaryTopologyWave -eq 'baseline-smt-off') { 'baseline-unpinned' } else { 'smt-on-unpinned' }

Write-Host "[recovery] primary verification wave: $primaryTopologyWave"
Write-Host "[recovery] paired fast-path and split-heavy benches live in benches/basic.rs and benches/criterion.rs"
Write-Host "[recovery] affinity-class artifacts live under target/bench-policy/runs/<wave>/<affinity-class>"
cargo test --quiet
cargo test --release --quiet
cargo test --doc --quiet
cargo package --list
& "$PSScriptRoot\verify_codegen.ps1"
& "$PSScriptRoot\bench_policy.ps1" -TopologyWave $primaryTopologyWave -AffinityClass $primaryAffinityClass -BenchName basic -CapturePerfCounters:$CapturePerfCounters
if ($RunSmtOnComparativeWave -and $primaryTopologyWave -ne 'smt-on-comparative') {
    Write-Host "[recovery] SMT-on comparative wave enabled"
    & "$PSScriptRoot\bench_policy.ps1" -TopologyWave smt-on-comparative -AffinityClass smt-on-unpinned -BenchName basic -CapturePerfCounters:$CapturePerfCounters
}
if ($RunTopologyWaves) {
    Write-Host "[recovery] operator-triggered topology waves enabled"
    & "$PSScriptRoot\run_topology_waves.ps1" -TopologyWave $TopologyWave -BenchName $TopologyBenchName -CapturePerfCounters:$CapturePerfCounters
}
