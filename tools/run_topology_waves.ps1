# Operator entrypoint for Zen 4 topology/perf waves.
[CmdletBinding()]
param(
    [string[]]$BenchName = @('basic'),
    [string[]]$TopologyWave = @('same-ccd-proxy', 'cross-ccd-proxy'),
    [switch]$IncludeBaseline,
    [switch]$IncludeSmtOnComparative,
    [switch]$CapturePerfCounters,
    [string]$ManifestPath
)
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path $PSScriptRoot -Parent
Set-Location $repoRoot
$env:RUSTFLAGS = "-C target-cpu=znver4"

$policyRoot = Join-Path $repoRoot 'target/bench-policy'
New-Item -ItemType Directory -Force -Path $policyRoot | Out-Null

if (-not $ManifestPath) {
    $ManifestPath = Join-Path $policyRoot 'topology-wave-manifest.json'
}

function Normalize-Wave {
    param([string]$Wave)
    if ($Wave -eq 'pending-smt-on') {
        Write-Host "[run_topology_waves] TopologyWave 'pending-smt-on' is deprecated and now aliases to 'smt-on-comparative'."
        return 'smt-on-comparative'
    }

    return $Wave
}

function Assert-Wave {
    param([string]$Wave)
    if ($Wave -notin @('baseline-smt-off', 'same-ccd-proxy', 'cross-ccd-proxy', 'smt-on-comparative')) {
        throw "unsupported topology wave '$Wave'."
    }
}

function Get-AffinityClassesForWave {
    param([string]$Wave)

    switch ($Wave) {
        'baseline-smt-off' { return @('baseline-unpinned') }
        'same-ccd-proxy' { return @('same-ccd-primary-proxy', 'same-ccd-secondary-proxy') }
        'cross-ccd-proxy' { return @('cross-ccd-proxy') }
        'smt-on-comparative' { return @('smt-on-unpinned') }
        default { throw "unsupported topology wave '$Wave'." }
    }
}
$resolvedWaves = [System.Collections.Generic.List[string]]::new()
if ($IncludeBaseline) {
    [void]$resolvedWaves.Add('baseline-smt-off')
}
foreach ($wave in $TopologyWave) {
    if ([string]::IsNullOrWhiteSpace($wave)) {
        continue
    }
    [void]$resolvedWaves.Add((Normalize-Wave -Wave $wave))
}
if ($IncludeSmtOnComparative) {
    [void]$resolvedWaves.Add('smt-on-comparative')
}
$resolvedWaves = @($resolvedWaves | Sort-Object -Unique)

if ($resolvedWaves.Count -eq 0) {
    throw 'run_topology_waves requires at least one topology wave.'
}

foreach ($wave in $resolvedWaves) {
    Assert-Wave $wave
}

$manifestRuns = @()

foreach ($wave in $resolvedWaves) {
    foreach ($affinityClass in (Get-AffinityClassesForWave $wave)) {
        Write-Host "Running topology wave $wave / $affinityClass"
        $runInfo = & "$PSScriptRoot\bench_policy.ps1" `
            -TopologyWave $wave `
            -AffinityClass $affinityClass `
            -BenchName $BenchName `
            -CapturePerfCounters:$CapturePerfCounters `
            -EmitRunObject

        if (-not $runInfo) {
            throw "bench_policy.ps1 did not emit run metadata for $wave / $affinityClass."
        }

        $runPath = $runInfo.run_json
        $fingerprintPath = $runInfo.fingerprint_json
        $topologyPath = $runInfo.topology_json
        $machinePath = $runInfo.machine_json
        $artifactDir = $runInfo.run_artifact_dir

        if (-not (Test-Path -LiteralPath $runPath)) {
            throw "run metadata missing for $wave / $affinityClass at '$runPath'."
        }
        if (-not (Test-Path -LiteralPath $fingerprintPath)) {
            throw "fingerprint missing for $wave / $affinityClass at '$fingerprintPath'."
        }
        if (-not (Test-Path -LiteralPath $topologyPath)) {
            throw "topology plan missing for $wave / $affinityClass at '$topologyPath'."
        }
        if (-not (Test-Path -LiteralPath $machinePath)) {
            throw "machine snapshot missing for $wave / $affinityClass at '$machinePath'."
        }

        $runManifest = Get-Content -Raw -LiteralPath $runPath | ConvertFrom-Json
        $manifestRuns += [ordered]@{
            topology_wave = $wave
            affinity_class = $affinityClass
            run_id = $runInfo.run_id
            artifact_dir = $artifactDir
            run_json = $runPath
            fingerprint_json = $fingerprintPath
            topology_json = $topologyPath
            machine_json = $machinePath
            benches = @($BenchName)
            execution_policy = $runManifest.execution_policy
            bench_results = $runManifest.bench_results
            bench_logs = $runManifest.bench_logs
            perf_counters = $runManifest.perf_counters
        }
    }
}

$manifest = [ordered]@{
    timestamp_utc = (Get-Date).ToUniversalTime().ToString('o')
    machine = $env:COMPUTERNAME
    target = 'x86_64-pc-windows-msvc'
    rustflags = $env:RUSTFLAGS
    include_baseline = [bool]$IncludeBaseline
    include_smt_on_comparative = [bool]$IncludeSmtOnComparative
    baseline_compatibility = if ($IncludeBaseline) { 'included' } else { 'unchanged' }
    perf_counters_requested = [bool]$CapturePerfCounters
    benches = @($BenchName)
    runs = $manifestRuns
}
$manifest | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $ManifestPath -Encoding utf8

Write-Host "Topology wave manifest: $ManifestPath"
Write-Host "Runs generated: $($manifestRuns.Count)"
