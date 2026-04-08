# CI checks (local, Zen 4-only)
[CmdletBinding()]
param(
    [switch]$RunTopologyWaves,
    [string[]]$TopologyWave = @('same-ccd-proxy', 'cross-ccd-proxy'),
    [string[]]$TopologyBenchName = @('basic', 'criterion'),
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
        throw 'ci.ps1 requires Win32_Processor metadata.'
    }

    $physicalCores = [int]$cpu.NumberOfCores
    $logicalProcessors = [int]$cpu.NumberOfLogicalProcessors
    if ($logicalProcessors -eq $physicalCores) {
        return 'baseline-smt-off'
    }
    if ($logicalProcessors -eq ($physicalCores * 2)) {
        return 'smt-on-comparative'
    }

    throw "unsupported topology for CI checks: logical=$logicalProcessors physical=$physicalCores"
}

$primaryTopologyWave = Get-PrimaryTopologyWave
$primaryAffinityClass = if ($primaryTopologyWave -eq 'baseline-smt-off') { 'baseline-unpinned' } else { 'smt-on-unpinned' }

cargo test --quiet
cargo test --release --quiet
cargo test --doc --quiet
cargo package --list
cargo doc --no-deps
& "$PSScriptRoot\verify_codegen.ps1"
& "$PSScriptRoot\bench_policy.ps1" -TopologyWave $primaryTopologyWave -AffinityClass $primaryAffinityClass -BenchName basic,criterion -CapturePerfCounters:$CapturePerfCounters
if ($RunSmtOnComparativeWave -and $primaryTopologyWave -ne 'smt-on-comparative') {
    Write-Host "[ci] SMT-on comparative wave enabled"
    & "$PSScriptRoot\bench_policy.ps1" -TopologyWave smt-on-comparative -AffinityClass smt-on-unpinned -BenchName basic,criterion -CapturePerfCounters:$CapturePerfCounters
}
if ($RunTopologyWaves) {
    Write-Host "[ci] operator-triggered topology waves enabled"
    & "$PSScriptRoot\run_topology_waves.ps1" -TopologyWave $TopologyWave -BenchName $TopologyBenchName -CapturePerfCounters:$CapturePerfCounters
}
