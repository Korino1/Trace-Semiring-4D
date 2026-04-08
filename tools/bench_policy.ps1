# Zen 4-only benchmark policy helper.
[CmdletBinding()]
param(
    [ValidateSet('basic', 'criterion')]
    [string[]]$BenchName = @('basic'),

    [ValidateSet('baseline-smt-off', 'same-ccd-proxy', 'cross-ccd-proxy', 'smt-on-comparative', 'pending-smt-on')]
    [string]$TopologyWave = 'baseline-smt-off',

    [ValidateSet('auto', 'baseline-unpinned', 'same-ccd-primary-proxy', 'same-ccd-secondary-proxy', 'cross-ccd-proxy', 'smt-on-unpinned', 'custom-mask')]
    [string]$AffinityClass = 'auto',

    [UInt64]$AffinityMask = 0,

    [string]$RunId,

    [switch]$CapturePerfCounters,

    [switch]$EmitRunObject
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path $PSScriptRoot -Parent
Set-Location $repoRoot
$env:RUSTFLAGS = "-C target-cpu=znver4"

$policyDir = Join-Path $repoRoot 'target/bench-policy'
New-Item -ItemType Directory -Force -Path $policyDir | Out-Null

function Convert-TimeToNs {
    param(
        [double]$Value,
        [string]$Unit
    )

    switch ($Unit) {
        'ns' { return $Value }
        'us' { return $Value * 1000.0 }
        'ms' { return $Value * 1000.0 * 1000.0 }
        's' { return $Value * 1000.0 * 1000.0 * 1000.0 }
        default { return $null }
    }
}

function Parse-BasicBenchResults {
    param([string]$LogPath)

    if (-not (Test-Path -LiteralPath $LogPath)) {
        return $null
    }

    $results = [ordered]@{}
    $pattern = '^test\s+(?<name>\S+)\s+\.\.\.\s+bench:\s+(?<value>[\d\.,]+)\s+(?<unit>ns|us|ms|s)\/iter\s+\(\+\/-\s+(?<pm>[\d\.,]+)\)'
    foreach ($line in (Get-Content -LiteralPath $LogPath)) {
        $m = [regex]::Match($line, $pattern)
        if (-not $m.Success) {
            continue
        }
        $name = $m.Groups['name'].Value
        $value = [double]($m.Groups['value'].Value -replace ',', '')
        $unit = $m.Groups['unit'].Value
        $pm = [double]($m.Groups['pm'].Value -replace ',', '')
        $ns = Convert-TimeToNs -Value $value -Unit $unit
        $pmNs = Convert-TimeToNs -Value $pm -Unit $unit

        $results[$name] = [ordered]@{
            value = $value
            unit = $unit
            ns_per_iter = $ns
            plus_minus = $pm
            plus_minus_ns = $pmNs
        }
    }

    return $results
}

function Format-MaskHex {
    param([UInt64]$Value)
    return ('0x{0:X16}' -f $Value)
}

function Get-FullMask {
    param([int]$BitCount)
    if ($BitCount -le 0) {
        return [UInt64]0
    }
    if ($BitCount -ge 64) {
        return [UInt64]::MaxValue
    }
    return (([UInt64]1 -shl $BitCount) - 1)
}

function Convert-ToMaskValue {
    param($Value)
    return [UInt64]([Int64]$Value)
}

function New-ArtifactPath {
    param(
        [string]$Root,
        [string]$Wave,
        [string]$Class,
        [string]$Id
    )

    $runsDir = Join-Path $Root 'runs'
    $waveDir = Join-Path $runsDir $Wave
    $classDir = Join-Path $waveDir $Class
    $runDir = Join-Path $classDir $Id
    New-Item -ItemType Directory -Force -Path $classDir | Out-Null
    New-Item -ItemType Directory -Force -Path $runDir | Out-Null
    return [ordered]@{
        class_dir = $classDir
        run_dir = $runDir
    }
}

function Get-MaskBitIndices {
    param(
        [UInt64]$Mask,
        [int]$BitLimit
    )

    $bits = [System.Collections.Generic.List[int]]::new()
    for ($bit = 0; $bit -lt $BitLimit; $bit++) {
        if (($Mask -band ([UInt64]1 -shl $bit)) -ne 0) {
            [void]$bits.Add($bit)
        }
    }
    return @($bits)
}

function Initialize-NumaNative {
    if ('Ts4.NumaNative' -as [type]) {
        return
    }

    Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;

namespace Ts4
{
    [StructLayout(LayoutKind.Sequential)]
    public struct GroupAffinity
    {
        public UIntPtr Mask;
        public ushort Group;
        public ushort Reserved0;
        public ushort Reserved1;
        public ushort Reserved2;
    }

    public static class NumaNative
    {
        [DllImport("kernel32.dll", SetLastError = true)]
        public static extern bool GetNumaHighestNodeNumber(out uint highestNodeNumber);

        [DllImport("kernel32.dll", SetLastError = true)]
        public static extern bool GetNumaNodeProcessorMaskEx(ushort node, out GroupAffinity processorMask);
    }
}
"@
}

function Get-NumaNodeMasks {
    param([int]$LogicalProcessors)

    Initialize-NumaNative

    [uint32]$highestNodeNumber = 0
    $haveHighestNode = [Ts4.NumaNative]::GetNumaHighestNodeNumber([ref]$highestNodeNumber)
    if (-not $haveHighestNode) {
        $lastError = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
        throw "GetNumaHighestNodeNumber failed (Win32=$lastError)."
    }

    $nodes = [System.Collections.Generic.List[object]]::new()
    for ($rawNodeId = 0; $rawNodeId -le $highestNodeNumber; $rawNodeId++) {
        $nodeId = [UInt16]$rawNodeId
        $groupAffinity = New-Object Ts4.GroupAffinity
        $ok = [Ts4.NumaNative]::GetNumaNodeProcessorMaskEx($nodeId, [ref]$groupAffinity)
        if (-not $ok) {
            continue
        }

        if ($groupAffinity.Group -ne 0) {
            throw "bench policy currently requires processor group 0, but node $nodeId belongs to group $($groupAffinity.Group)."
        }

        $mask = [UInt64]$groupAffinity.Mask.ToUInt64()
        if ($mask -eq 0) {
            continue
        }

        $logicalList = @(Get-MaskBitIndices -Mask $mask -BitLimit $LogicalProcessors)
        if ($logicalList.Count -eq 0) {
            continue
        }

        [void]$nodes.Add([ordered]@{
            node_id = [int]$nodeId
            group = [int]$groupAffinity.Group
            mask_value = $mask
            mask_hex = Format-MaskHex $mask
            logical_processors = $logicalList
            logical_count = $logicalList.Count
        })
    }

    if ($nodes.Count -lt 2) {
        throw "bench policy requires at least 2 populated NUMA-node masks for 7950X 2-CCD policy, observed $($nodes.Count)."
    }

    return @($nodes | Sort-Object node_id)
}

function New-CrossCcdBalancedMask {
    param(
        [int[]]$PrimaryBits,
        [int[]]$SecondaryBits
    )

    $maxPairs = [Math]::Min($PrimaryBits.Count, $SecondaryBits.Count)
    if ($maxPairs -lt 2) {
        throw "cross-ccd-proxy requires at least two logical processors per selected CCD, got primary=$($PrimaryBits.Count), secondary=$($SecondaryBits.Count)."
    }

    $threadsPerCcd = [int][Math]::Floor($maxPairs / 2)
    if ($threadsPerCcd -lt 1) {
        $threadsPerCcd = 1
    }
    $fromPrimary = $threadsPerCcd
    $fromSecondary = $threadsPerCcd

    $selected = [System.Collections.Generic.List[int]]::new()
    foreach ($bit in ($PrimaryBits | Select-Object -First $fromPrimary)) {
        [void]$selected.Add([int]$bit)
    }
    foreach ($bit in ($SecondaryBits | Select-Object -First $fromSecondary)) {
        [void]$selected.Add([int]$bit)
    }

    if ($selected.Count -lt 2) {
        throw 'cross-ccd-proxy produced an empty/insufficient balanced mask.'
    }

    [UInt64]$mask = 0
    foreach ($bit in $selected) {
        $mask = $mask -bor ([UInt64]1 -shl $bit)
    }

    return [ordered]@{
        mask_value = $mask
        mask_hex = Format-MaskHex $mask
        threads_per_ccd = $threadsPerCcd
        selected_bits = @($selected | Sort-Object)
    }
}

function Get-PerfCounterPaths {
    $requested = @(
        '\Processor Information(_Total)\% Processor Utility',
        '\Processor Information(_Total)\% of Maximum Frequency',
        '\Processor Information(_Total)\Processor Frequency',
        '\System\Processor Queue Length',
        '\System\Context Switches/sec',
        '\Memory\Available MBytes'
    )

    $available = [System.Collections.Generic.List[string]]::new()
    foreach ($counterPath in $requested) {
        try {
            [void](Get-Counter -Counter $counterPath -MaxSamples 1 -ErrorAction Stop)
            [void]$available.Add($counterPath)
        } catch {
            continue
        }
    }

    return [ordered]@{
        requested = $requested
        available = @($available)
    }
}

function Get-PerfCounterSnapshot {
    param([string[]]$CounterPaths)

    if ($CounterPaths.Count -eq 0) {
        return $null
    }

    $sample = Get-Counter -Counter $CounterPaths -MaxSamples 1 -ErrorAction Stop
    $counters = [ordered]@{}
    foreach ($entry in $sample.CounterSamples) {
        $path = $entry.Path
        $counters[$path] = [ordered]@{
            cooked_value = [double]$entry.CookedValue
            instance_name = $entry.InstanceName
            counter_name = $entry.CounterName
        }
    }

    return [ordered]@{
        timestamp_utc = (Get-Date).ToUniversalTime().ToString('o')
        counters = $counters
    }
}

$cpu = Get-CimInstance Win32_Processor | Select-Object -First 1
if (-not $cpu) {
    throw 'bench policy requires Win32_Processor metadata.'
}

if ($cpu.Name -notmatch '7950X') {
    throw "bench policy requires Ryzen 9 7950X hardware, got '$($cpu.Name)'."
}

$os = Get-CimInstance Win32_OperatingSystem | Select-Object -First 1
$computer = Get-CimInstance Win32_ComputerSystem | Select-Object -First 1
$bios = Get-CimInstance Win32_BIOS -ErrorAction SilentlyContinue | Select-Object -First 1
$baseBoard = Get-CimInstance Win32_BaseBoard -ErrorAction SilentlyContinue | Select-Object -First 1
$process = [System.Diagnostics.Process]::GetCurrentProcess()
$currentAffinityMask = Convert-ToMaskValue $process.ProcessorAffinity.ToInt64()

$physicalCores = [int]$cpu.NumberOfCores
$logicalProcessors = [int]$cpu.NumberOfLogicalProcessors
if ($physicalCores -lt 1 -or $logicalProcessors -lt 1) {
    throw 'bench policy requires positive core topology metadata.'
}

$logicalMask = Get-FullMask $logicalProcessors
$invalidAffinityMaskBits = $AffinityMask -band ([UInt64]::MaxValue -bxor $logicalMask)
if ($AffinityMask -ne 0 -and $invalidAffinityMaskBits -ne 0) {
    throw "requested affinity mask $(Format-MaskHex $AffinityMask) exceeds the available logical processor mask $(Format-MaskHex $logicalMask)."
}

if ($logicalProcessors -gt 64) {
    throw "bench policy currently supports up to 64 logical processors for affinity masks, got $logicalProcessors."
}

$numaNodeMasks = @(Get-NumaNodeMasks -LogicalProcessors $logicalProcessors)
$ccdNodes = @($numaNodeMasks | Sort-Object @{ Expression = 'logical_count'; Descending = $true }, @{ Expression = 'node_id'; Descending = $false } | Select-Object -First 2)
if ($ccdNodes.Count -lt 2) {
    throw 'bench policy could not determine two CCD candidates from NUMA topology.'
}
$sameCcdPrimaryMask = [UInt64]$ccdNodes[0].mask_value
$sameCcdSecondaryMask = [UInt64]$ccdNodes[1].mask_value
$crossMaskPlan = New-CrossCcdBalancedMask -PrimaryBits $ccdNodes[0].logical_processors -SecondaryBits $ccdNodes[1].logical_processors
$crossCcdProxyMask = [UInt64]$crossMaskPlan.mask_value

$baselineState = if ($logicalProcessors -eq $physicalCores) {
    'SMT-off physical-cores-only'
} elseif ($logicalProcessors -eq ($physicalCores * 2)) {
    'SMT-on (not baseline for this checkout)'
} else {
    'mixed-or-virtualized topology'
}

$resolvedTopologyWave = if ($TopologyWave -eq 'pending-smt-on') {
    'smt-on-comparative'
} else {
    $TopologyWave
}

if ($TopologyWave -eq 'pending-smt-on') {
    Write-Host "[bench_policy] TopologyWave 'pending-smt-on' is deprecated and now aliases to 'smt-on-comparative'."
}

if ($resolvedTopologyWave -eq 'baseline-smt-off') {
    if ($logicalProcessors -ne $physicalCores) {
        throw "baseline-smt-off requires SMT-off physical-cores-only topology, but observed $logicalProcessors logical processors across $physicalCores physical cores."
    }
} elseif ($resolvedTopologyWave -eq 'smt-on-comparative') {
    if ($logicalProcessors -ne ($physicalCores * 2)) {
        throw "smt-on-comparative requires SMT-on topology (logical == 2 x physical), but observed $logicalProcessors logical processors across $physicalCores physical cores."
    }
}

if ($AffinityMask -ne 0 -and $AffinityClass -notin @('auto', 'custom-mask')) {
    throw "AffinityMask can only be combined with AffinityClass 'auto' or 'custom-mask', got '$AffinityClass'."
}

$resolvedAffinityClass = if ($AffinityClass -ne 'auto') {
    $AffinityClass
} elseif ($AffinityMask -ne 0) {
    'custom-mask'
} elseif ($resolvedTopologyWave -eq 'same-ccd-proxy') {
    'same-ccd-primary-proxy'
} elseif ($resolvedTopologyWave -eq 'cross-ccd-proxy') {
    'cross-ccd-proxy'
} elseif ($resolvedTopologyWave -eq 'smt-on-comparative') {
    'smt-on-unpinned'
} else {
    'baseline-unpinned'
}

$allowedAffinityClasses = switch ($resolvedTopologyWave) {
    'baseline-smt-off' { @('baseline-unpinned', 'custom-mask') }
    'same-ccd-proxy' { @('same-ccd-primary-proxy', 'same-ccd-secondary-proxy', 'custom-mask') }
    'cross-ccd-proxy' { @('cross-ccd-proxy', 'custom-mask') }
    'smt-on-comparative' { @('smt-on-unpinned', 'custom-mask') }
    default { throw "unsupported TopologyWave '$resolvedTopologyWave'" }
}

if ($resolvedAffinityClass -notin $allowedAffinityClasses) {
    throw "AffinityClass '$resolvedAffinityClass' is not valid for TopologyWave '$resolvedTopologyWave'. Allowed: $($allowedAffinityClasses -join ', ')."
}

$appliedAffinityMask = switch ($resolvedAffinityClass) {
    'baseline-unpinned' { [UInt64]0 }
    'same-ccd-primary-proxy' { $sameCcdPrimaryMask }
    'same-ccd-secondary-proxy' { $sameCcdSecondaryMask }
    'cross-ccd-proxy' { $crossCcdProxyMask }
    'smt-on-unpinned' { [UInt64]0 }
    'custom-mask' { $AffinityMask }
    default { throw "unsupported AffinityClass '$resolvedAffinityClass'" }
}

if ($appliedAffinityMask -ne 0) {
    $process.ProcessorAffinity = [IntPtr]::new([int64]$appliedAffinityMask)
}

if (-not $RunId) {
    $RunId = "{0}-pid{1}" -f (Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssfffZ'), $PID
}
$RunId = $RunId -replace '[^A-Za-z0-9._-]', '_'
if ([string]::IsNullOrWhiteSpace($RunId)) {
    throw 'RunId resolved to an empty value after sanitization.'
}

$artifactPaths = New-ArtifactPath -Root $policyDir -Wave $resolvedTopologyWave -Class $resolvedAffinityClass -Id $RunId
$artifactClassDir = $artifactPaths.class_dir
$artifactDir = $artifactPaths.run_dir

$fingerprint = [ordered]@{
    timestamp_utc = (Get-Date).ToUniversalTime().ToString('o')
    machine = $env:COMPUTERNAME
    system_manufacturer = $computer.Manufacturer
    system_model = $computer.Model
    os_caption = $os.Caption
    os_version = $os.Version
    os_build = $os.BuildNumber
    cpu_name = $cpu.Name
    processor_id = $cpu.ProcessorId
    physical_cores = $physicalCores
    logical_processors = $logicalProcessors
    numa_nodes = $numaNodeMasks.Count
    l2_cache_kb = $cpu.L2CacheSize
    l3_cache_kb = $cpu.L3CacheSize
    current_process_affinity_mask_hex = Format-MaskHex $currentAffinityMask
    applied_affinity_mask_hex = Format-MaskHex $appliedAffinityMask
    requested_topology_wave = $TopologyWave
    topology_wave = $resolvedTopologyWave
    affinity_class = $resolvedAffinityClass
    run_id = $RunId
    baseline_state = $baselineState
    target = 'x86_64-pc-windows-msvc'
    rustflags = $env:RUSTFLAGS
    benches = @($BenchName)
    artifact_dir = $artifactDir
}

$topologyPlan = [ordered]@{
    policy = '7950X benchmark policy with NUMA-derived same-CCD/cross-CCD affinity and explicit SMT-off baseline'
    baseline_state = $baselineState
    requested_wave = $TopologyWave
    current_wave = $resolvedTopologyWave
    affinity_class = $resolvedAffinityClass
    run_id = $RunId
    requested_affinity_mask_hex = Format-MaskHex $AffinityMask
    logical_processor_mask_hex = Format-MaskHex $logicalMask
    same_ccd_primary_proxy_mask_hex = Format-MaskHex $sameCcdPrimaryMask
    same_ccd_secondary_proxy_mask_hex = Format-MaskHex $sameCcdSecondaryMask
    cross_ccd_proxy_mask_hex = Format-MaskHex $crossCcdProxyMask
    ccd_nodes = @($ccdNodes | ForEach-Object {
            [ordered]@{
                node_id = $_.node_id
                group = $_.group
                mask_hex = $_.mask_hex
                logical_processors = $_.logical_processors
                logical_count = $_.logical_count
            }
        })
    numa_nodes = @($numaNodeMasks | ForEach-Object {
            [ordered]@{
                node_id = $_.node_id
                group = $_.group
                mask_hex = $_.mask_hex
                logical_processors = $_.logical_processors
                logical_count = $_.logical_count
            }
        })
    cross_ccd_balanced_selection = [ordered]@{
        threads_per_ccd = $crossMaskPlan.threads_per_ccd
        selected_logical_processors = $crossMaskPlan.selected_bits
        mask_hex = $crossMaskPlan.mask_hex
    }
    interpretation = 'same-CCD and cross-CCD masks are derived from Windows NUMA-node processor masks (2-CCD policy on 7950X); memory placement remains an explicit operator responsibility.'
    wave_separation = 'baseline-smt-off and smt-on-comparative runs are recorded in separate wave-scoped artifact trees and are never mixed.'
}

$machine = [ordered]@{
    timestamp_utc = $fingerprint.timestamp_utc
    machine = $env:COMPUTERNAME
    os_caption = $os.Caption
    os_version = $os.Version
    os_build = $os.BuildNumber
    bios_version = if ($bios) { @($bios.SMBIOSBIOSVersion) } else { @() }
    bios_release_date = if ($bios) { $bios.ReleaseDate } else { $null }
    baseboard_manufacturer = if ($baseBoard) { $baseBoard.Manufacturer } else { $null }
    baseboard_product = if ($baseBoard) { $baseBoard.Product } else { $null }
    cpu_name = $cpu.Name
    cpu_socket = $cpu.SocketDesignation
    l2_cache_kb = $cpu.L2CacheSize
    l3_cache_kb = $cpu.L3CacheSize
    physical_cores = $physicalCores
    logical_processors = $logicalProcessors
    numa_nodes = @($numaNodeMasks | ForEach-Object {
            [ordered]@{
                node_id = $_.node_id
                group = $_.group
                mask_hex = $_.mask_hex
                logical_processors = $_.logical_processors
            }
        })
}

$fingerprintPath = Join-Path $policyDir 'fingerprint.json'
$topologyPath = Join-Path $policyDir 'topology.json'
$machinePath = Join-Path $policyDir 'machine.json'
$runFingerprintPath = Join-Path $artifactDir 'fingerprint.json'
$runTopologyPath = Join-Path $artifactDir 'topology.json'
$runMachinePath = Join-Path $artifactDir 'machine.json'
$runMetadataPath = Join-Path $artifactDir 'run.json'
$fingerprint | ConvertTo-Json -Depth 6 | Set-Content -Encoding utf8 $runFingerprintPath
$topologyPlan | ConvertTo-Json -Depth 6 | Set-Content -Encoding utf8 $runTopologyPath
$machine | ConvertTo-Json -Depth 6 | Set-Content -Encoding utf8 $runMachinePath

$runMetadata = [ordered]@{
    timestamp_utc = $fingerprint.timestamp_utc
    run_id = $RunId
    requested_topology_wave = $TopologyWave
    topology_wave = $resolvedTopologyWave
    affinity_class = $resolvedAffinityClass
    benchmark_artifact_dir = $artifactDir
    applied_affinity_mask_hex = Format-MaskHex $appliedAffinityMask
    ccd_node_ids = @($ccdNodes | ForEach-Object { $_.node_id })
    same_ccd_primary_mask_hex = Format-MaskHex $sameCcdPrimaryMask
    same_ccd_secondary_mask_hex = Format-MaskHex $sameCcdSecondaryMask
    cross_ccd_proxy_mask_hex = Format-MaskHex $crossCcdProxyMask
    benches = @($BenchName)
    execution_policy = 'execute-benchmarks'
    bench_logs = [ordered]@{}
    bench_results = [ordered]@{}
    perf_counters = [ordered]@{
        requested = [bool]$CapturePerfCounters
        mode = 'windows-os-counter-snapshots'
    }
}
$runMetadata | ConvertTo-Json -Depth 8 | Set-Content -Encoding utf8 $runMetadataPath

$latestRunPath = Join-Path $artifactClassDir 'latest-run.json'
[ordered]@{
    timestamp_utc = $fingerprint.timestamp_utc
    run_id = $RunId
    run_dir = $artifactDir
    run_json = $runMetadataPath
} | ConvertTo-Json -Depth 4 | Set-Content -Encoding utf8 $latestRunPath

$writeSharedCompatibilityFiles = (
    $resolvedTopologyWave -eq 'baseline-smt-off' -and $resolvedAffinityClass -eq 'baseline-unpinned'
)
if ($writeSharedCompatibilityFiles) {
    $fingerprint | ConvertTo-Json -Depth 6 | Set-Content -Encoding utf8 $fingerprintPath
    $topologyPlan | ConvertTo-Json -Depth 6 | Set-Content -Encoding utf8 $topologyPath
    $machine | ConvertTo-Json -Depth 6 | Set-Content -Encoding utf8 $machinePath
}

Write-Host "Bench topology policy: baseline state is $baselineState; SMT-off baseline and SMT-on comparative runs are kept in separate wave artifacts."
if ($writeSharedCompatibilityFiles) {
    Write-Host "Fingerprint: $fingerprintPath"
    Write-Host "Topology plan: $topologyPath"
    Write-Host "Machine snapshot: $machinePath"
} else {
    Write-Host "Fingerprint: $runFingerprintPath"
    Write-Host "Topology plan: $runTopologyPath"
    Write-Host "Machine snapshot: $runMachinePath"
}
Write-Host "Run artifacts: $artifactDir"

$perfCounterInfo = [ordered]@{
    requested = [bool]$CapturePerfCounters
    requested_paths = @()
    available_paths = @()
    status = 'disabled'
}
if ($CapturePerfCounters) {
    $paths = Get-PerfCounterPaths
    $perfCounterInfo.requested_paths = @($paths.requested)
    $perfCounterInfo.available_paths = @($paths.available)
    if ($paths.available.Count -gt 0) {
        $perfCounterInfo.status = 'enabled'
        Write-Host "Perf counters: enabled ($($paths.available.Count) available paths)"
    } else {
        $perfCounterInfo.status = 'no-supported-counters-found'
        Write-Host 'Perf counters: requested, but no supported counters were available on this host.'
    }
}
$runMetadata.perf_counters = $perfCounterInfo


foreach ($bench in $BenchName) {
    $logPath = Join-Path $artifactDir "$bench.log"
    $runMetadata.bench_logs[$bench] = $logPath

    $perfPrePath = $null
    $perfPostPath = $null
    if ($perfCounterInfo.status -eq 'enabled') {
        $perfPrePath = Join-Path $artifactDir "$bench.perf.pre.json"
        $perfPostPath = Join-Path $artifactDir "$bench.perf.post.json"
        $snapshotPre = Get-PerfCounterSnapshot -CounterPaths $perfCounterInfo.available_paths
        $snapshotPre | ConvertTo-Json -Depth 8 | Set-Content -Encoding utf8 $perfPrePath
    }

    Write-Host "Running cargo bench --bench $bench under $resolvedTopologyWave / $resolvedAffinityClass"
    & cargo bench --bench $bench 2>&1 | Tee-Object -FilePath $logPath | Out-Host
    if ($LASTEXITCODE -ne 0) {
        throw "cargo bench --bench $bench failed with exit code $LASTEXITCODE"
    }

    if ($perfCounterInfo.status -eq 'enabled') {
        $snapshotPost = Get-PerfCounterSnapshot -CounterPaths $perfCounterInfo.available_paths
        $snapshotPost | ConvertTo-Json -Depth 8 | Set-Content -Encoding utf8 $perfPostPath
        $runMetadata.perf_counters[$bench] = [ordered]@{
            pre = $perfPrePath
            post = $perfPostPath
        }
    }

    if ($bench -eq 'basic') {
        $parsed = Parse-BasicBenchResults -LogPath $logPath
        if ($parsed -ne $null -and $parsed.Count -gt 0) {
            $resultsPath = Join-Path $artifactDir 'basic.results.json'
            $parsed | ConvertTo-Json -Depth 6 | Set-Content -Encoding utf8 $resultsPath

            $runMetadata.bench_results['basic'] = $resultsPath

            if ($writeSharedCompatibilityFiles) {
                $sharedResultsPath = Join-Path $policyDir 'basic.results.json'
                $parsed | ConvertTo-Json -Depth 6 | Set-Content -Encoding utf8 $sharedResultsPath
            }
        }
    }
}

$runMetadata | ConvertTo-Json -Depth 8 | Set-Content -Encoding utf8 $runMetadataPath

if ($EmitRunObject) {
    [pscustomobject]@{
        run_id = $RunId
        topology_wave = $resolvedTopologyWave
        affinity_class = $resolvedAffinityClass
        run_artifact_dir = $artifactDir
        run_json = $runMetadataPath
        fingerprint_json = $runFingerprintPath
        topology_json = $runTopologyPath
        machine_json = $runMachinePath
        execution_policy = $runMetadata.execution_policy
    }
}
