# Compare machine-readable `basic.results.json` outputs in pairwise mode or topology-manifest mode.

[CmdletBinding()]
param(
    [string]$BaseResultsPath,
    [string]$HeadResultsPath,
    [string]$BaseRunDir,
    [string]$HeadRunDir,
    [string]$TopologyManifestPath,
    [string]$IncludeRegex,
    [string]$ExcludeRegex,
    [switch]$OnlyChanged,
    [switch]$SortByAbsDelta,
    [string]$OutReportPath,
    [switch]$EmitReportObject
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path $PSScriptRoot -Parent
Set-Location $repoRoot

function Resolve-ResultsPath {
    param(
        [string]$ExplicitPath,
        [string]$RunDir
    )

    if ($ExplicitPath) {
        return (Resolve-Path -LiteralPath $ExplicitPath).Path
    }

    if ($RunDir) {
        $candidate = Join-Path $RunDir 'basic.results.json'
        return (Resolve-Path -LiteralPath $candidate).Path
    }

    return $null
}

function Read-JsonObject {
    param([string]$Path)

    if (-not (Test-Path -LiteralPath $Path)) {
        throw "JSON file not found: $Path"
    }

    return (Get-Content -Raw -LiteralPath $Path | ConvertFrom-Json)
}

function Get-DeltaPercent {
    param(
        [double]$Base,
        [double]$Head
    )

    if ([double]::IsNaN($Base) -or [double]::IsNaN($Head) -or $Base -eq 0) {
        return [double]::NaN
    }

    return (($Head - $Base) / $Base) * 100.0
}

function Get-GeomeanDeltaPct {
    param(
        [object[]]$Rows,
        [string]$NumeratorField,
        [string]$DenominatorField
    )

    $logs = [System.Collections.Generic.List[double]]::new()
    foreach ($row in $Rows) {
        if ($null -eq $row) {
            continue
        }

        $num = [double]$row.$NumeratorField
        $den = [double]$row.$DenominatorField
        if ([double]::IsNaN($num) -or [double]::IsNaN($den) -or $num -le 0 -or $den -le 0) {
            continue
        }

        [void]$logs.Add([Math]::Log($num / $den))
    }

    if ($logs.Count -eq 0) {
        return [double]::NaN
    }

    $avg = ($logs | Measure-Object -Average).Average
    return ([Math]::Exp($avg) - 1.0) * 100.0
}

function Get-JoinedBenchRows {
    param(
        $Base,
        $Head,
        [string]$IncludeRegex,
        [string]$ExcludeRegex,
        [bool]$OnlyChanged
    )

    $allKeys = @()
    $allKeys += $Base.PSObject.Properties.Name
    $allKeys += $Head.PSObject.Properties.Name
    $keys = $allKeys | Sort-Object -Unique

    $rows = foreach ($key in $keys) {
        if ($IncludeRegex -and ($key -notmatch $IncludeRegex)) {
            continue
        }
        if ($ExcludeRegex -and ($key -match $ExcludeRegex)) {
            continue
        }

        $b = $Base.$key
        $h = $Head.$key
        $bNs = if ($b) { [double]$b.ns_per_iter } else { [double]::NaN }
        $hNs = if ($h) { [double]$h.ns_per_iter } else { [double]::NaN }
        $delta = $hNs - $bNs
        $pct = Get-DeltaPercent -Base $bNs -Head $hNs

        $basePresent = -not [double]::IsNaN($bNs)
        $headPresent = -not [double]::IsNaN($hNs)
        $status = if ($basePresent -and $headPresent) {
            'ok'
        } elseif ($basePresent -and -not $headPresent) {
            'missing_in_head'
        } elseif (-not $basePresent -and $headPresent) {
            'missing_in_base'
        } else {
            'missing_both'
        }

        if ($OnlyChanged) {
            if ($status -eq 'ok' -and $delta -eq 0) {
                continue
            }
            if ($status -eq 'missing_both') {
                continue
            }
        }

        [pscustomobject]@{
            bench = $key
            status = $status
            base_ns = $bNs
            head_ns = $hNs
            delta_ns = $delta
            delta_pct = $pct
        }
    }

    return @($rows)
}

function New-RunSummary {
    param(
        [object[]]$Rows,
        [string]$BasePath,
        [string]$HeadPath,
        [string]$TopologyWave,
        [string]$AffinityClass,
        [string]$RunId
    )

    $okRows = @($Rows | Where-Object status -eq 'ok')
    [pscustomobject]@{
        topology_wave = $TopologyWave
        affinity_class = $AffinityClass
        run_id = $RunId
        base_path = $BasePath
        head_path = $HeadPath
        benches_total = $Rows.Count
        benches_compared = $okRows.Count
        improved = @($okRows | Where-Object { $_.delta_pct -lt 0 }).Count
        worsened = @($okRows | Where-Object { $_.delta_pct -gt 0 }).Count
        unchanged = @($okRows | Where-Object { $_.delta_pct -eq 0 }).Count
        missing_in_head = @($Rows | Where-Object status -eq 'missing_in_head').Count
        missing_in_base = @($Rows | Where-Object status -eq 'missing_in_base').Count
        geomean_delta_pct = Get-GeomeanDeltaPct -Rows $okRows -NumeratorField 'head_ns' -DenominatorField 'base_ns'
    }
}

function Resolve-BaselineResultsPath {
    param([string]$ExplicitBasePath)

    if ($ExplicitBasePath) {
        return (Resolve-Path -LiteralPath $ExplicitBasePath).Path
    }

    $latestRunPath = Join-Path $repoRoot 'target/bench-policy/runs/baseline-smt-off/baseline-unpinned/latest-run.json'
    if (Test-Path -LiteralPath $latestRunPath) {
        $latestRun = Read-JsonObject -Path $latestRunPath

        if ($latestRun.run_json -and (Test-Path -LiteralPath $latestRun.run_json)) {
            $run = Read-JsonObject -Path $latestRun.run_json
            if ($run.bench_results) {
                $basicProp = $run.bench_results.PSObject.Properties['basic']
                if ($basicProp -and $basicProp.Value -and (Test-Path -LiteralPath $basicProp.Value)) {
                    return (Resolve-Path -LiteralPath $basicProp.Value).Path
                }
            }
        }

        if ($latestRun.run_dir) {
            $candidate = Join-Path $latestRun.run_dir 'basic.results.json'
            if (Test-Path -LiteralPath $candidate) {
                return (Resolve-Path -LiteralPath $candidate).Path
            }
        }
    }

    $compatBase = Join-Path $repoRoot 'target/bench-policy/basic.results.json'
    if (Test-Path -LiteralPath $compatBase) {
        return (Resolve-Path -LiteralPath $compatBase).Path
    }

    return $null
}

function Resolve-ManifestRunResultsPath {
    param($Run)

    if ($Run.bench_results) {
        $basicProp = $Run.bench_results.PSObject.Properties['basic']
        if ($basicProp -and $basicProp.Value -and (Test-Path -LiteralPath $basicProp.Value)) {
            return (Resolve-Path -LiteralPath $basicProp.Value).Path
        }
    }

    if ($Run.artifact_dir) {
        $candidate = Join-Path $Run.artifact_dir 'basic.results.json'
        if (Test-Path -LiteralPath $candidate) {
            return (Resolve-Path -LiteralPath $candidate).Path
        }
    }

    return $null
}

function Get-TopologyRows {
    param(
        $Base,
        $SamePrimary,
        $SameSecondary,
        $Cross,
        [string]$IncludeRegex,
        [string]$ExcludeRegex,
        [bool]$OnlyChanged
    )

    $allKeys = @()
    $allKeys += $Base.PSObject.Properties.Name
    if ($SamePrimary) { $allKeys += $SamePrimary.PSObject.Properties.Name }
    if ($SameSecondary) { $allKeys += $SameSecondary.PSObject.Properties.Name }
    if ($Cross) { $allKeys += $Cross.PSObject.Properties.Name }
    $keys = $allKeys | Sort-Object -Unique

    $rows = foreach ($key in $keys) {
        if ($IncludeRegex -and ($key -notmatch $IncludeRegex)) {
            continue
        }
        if ($ExcludeRegex -and ($key -match $ExcludeRegex)) {
            continue
        }

        $baseEntry = $Base.$key
        $samePrimaryEntry = if ($SamePrimary) { $SamePrimary.$key } else { $null }
        $sameSecondaryEntry = if ($SameSecondary) { $SameSecondary.$key } else { $null }
        $crossEntry = if ($Cross) { $Cross.$key } else { $null }

        $baseNs = if ($baseEntry) { [double]$baseEntry.ns_per_iter } else { [double]::NaN }
        $samePrimaryNs = if ($samePrimaryEntry) { [double]$samePrimaryEntry.ns_per_iter } else { [double]::NaN }
        $sameSecondaryNs = if ($sameSecondaryEntry) { [double]$sameSecondaryEntry.ns_per_iter } else { [double]::NaN }
        $crossNs = if ($crossEntry) { [double]$crossEntry.ns_per_iter } else { [double]::NaN }

        $sameValues = @()
        if (-not [double]::IsNaN($samePrimaryNs)) {
            $sameValues += $samePrimaryNs
        }
        if (-not [double]::IsNaN($sameSecondaryNs)) {
            $sameValues += $sameSecondaryNs
        }
        $sameAvgNs = if ($sameValues.Count -gt 0) { ($sameValues | Measure-Object -Average).Average } else { [double]::NaN }

        $sameAvgVsBasePct = Get-DeltaPercent -Base $baseNs -Head $sameAvgNs
        $crossVsBasePct = Get-DeltaPercent -Base $baseNs -Head $crossNs
        $crossVsSameAvgPct = Get-DeltaPercent -Base $sameAvgNs -Head $crossNs

        $status = if ([double]::IsNaN($baseNs)) {
            'missing_baseline'
        } elseif ([double]::IsNaN($sameAvgNs) -and [double]::IsNaN($crossNs)) {
            'missing_same_and_cross'
        } elseif ([double]::IsNaN($sameAvgNs)) {
            'missing_same'
        } elseif ([double]::IsNaN($crossNs)) {
            'missing_cross'
        } else {
            'ok'
        }

        if ($OnlyChanged -and $status -eq 'ok') {
            $changed = $false
            foreach ($value in @($sameAvgVsBasePct, $crossVsBasePct, $crossVsSameAvgPct)) {
                if (-not [double]::IsNaN($value) -and $value -ne 0) {
                    $changed = $true
                    break
                }
            }
            if (-not $changed) {
                continue
            }
        }

        [pscustomobject]@{
            bench = $key
            status = $status
            base_ns = $baseNs
            same_primary_ns = $samePrimaryNs
            same_secondary_ns = $sameSecondaryNs
            same_avg_ns = $sameAvgNs
            cross_ns = $crossNs
            same_avg_vs_base_pct = $sameAvgVsBasePct
            cross_vs_base_pct = $crossVsBasePct
            cross_vs_same_avg_pct = $crossVsSameAvgPct
        }
    }

    return @($rows)
}

$basePath = Resolve-ResultsPath -ExplicitPath $BaseResultsPath -RunDir $BaseRunDir
$headPath = Resolve-ResultsPath -ExplicitPath $HeadResultsPath -RunDir $HeadRunDir

if ($TopologyManifestPath) {
    $manifestPath = (Resolve-Path -LiteralPath $TopologyManifestPath).Path
    $manifest = Read-JsonObject -Path $manifestPath
    $runs = @($manifest.runs)
    if ($runs.Count -eq 0) {
        throw "Topology manifest has no runs: $manifestPath"
    }

    if (-not $basePath) {
        $basePath = Resolve-BaselineResultsPath -ExplicitBasePath $null
    }
    if (-not $basePath) {
        throw 'Unable to resolve baseline basic.results.json for topology-manifest mode.'
    }

    $base = Read-JsonObject -Path $basePath
    $runSummaries = @()
    $runResultsByClass = @{}

    foreach ($run in $runs) {
        $runPath = Resolve-ManifestRunResultsPath -Run $run
        if (-not $runPath) {
            Write-Warning ("Skipping run without basic.results.json: wave={0}, class={1}, run_id={2}" -f $run.topology_wave, $run.affinity_class, $run.run_id)
            continue
        }

        $head = Read-JsonObject -Path $runPath
        $rows = Get-JoinedBenchRows -Base $base -Head $head -IncludeRegex $IncludeRegex -ExcludeRegex $ExcludeRegex -OnlyChanged:$OnlyChanged

        $summary = New-RunSummary -Rows $rows -BasePath $basePath -HeadPath $runPath -TopologyWave $run.topology_wave -AffinityClass $run.affinity_class -RunId $run.run_id
        $runSummaries += $summary

        $runResultsByClass[[string]$run.affinity_class] = [ordered]@{
            results = $head
            rows = $rows
            path = $runPath
            run_id = [string]$run.run_id
            topology_wave = [string]$run.topology_wave
        }
    }

    if ($runSummaries.Count -eq 0) {
        throw 'No comparable runs were found in topology manifest mode.'
    }

    $samePrimary = if ($runResultsByClass.ContainsKey('same-ccd-primary-proxy')) { $runResultsByClass['same-ccd-primary-proxy'].results } else { $null }
    $sameSecondary = if ($runResultsByClass.ContainsKey('same-ccd-secondary-proxy')) { $runResultsByClass['same-ccd-secondary-proxy'].results } else { $null }
    $cross = if ($runResultsByClass.ContainsKey('cross-ccd-proxy')) { $runResultsByClass['cross-ccd-proxy'].results } else { $null }
    $smtOn = if ($runResultsByClass.ContainsKey('smt-on-unpinned')) { $runResultsByClass['smt-on-unpinned'] } else { $null }

    $topologyRows = Get-TopologyRows -Base $base -SamePrimary $samePrimary -SameSecondary $sameSecondary -Cross $cross -IncludeRegex $IncludeRegex -ExcludeRegex $ExcludeRegex -OnlyChanged:$OnlyChanged

    $topologySummary = [pscustomobject]@{
        rows_total = $topologyRows.Count
        same_avg_vs_base_geomean_pct = Get-GeomeanDeltaPct -Rows $topologyRows -NumeratorField 'same_avg_ns' -DenominatorField 'base_ns'
        cross_vs_base_geomean_pct = Get-GeomeanDeltaPct -Rows $topologyRows -NumeratorField 'cross_ns' -DenominatorField 'base_ns'
        cross_vs_same_avg_geomean_pct = Get-GeomeanDeltaPct -Rows $topologyRows -NumeratorField 'cross_ns' -DenominatorField 'same_avg_ns'
        interpretation = 'positive delta means slower (higher ns/iter), negative delta means faster.'
    }

    $smtOnRows = if ($smtOn) { @($smtOn.rows) } else { @() }
    $smtOnSummary = if ($smtOn) {
        [pscustomobject]@{
            present = $true
            topology_wave = $smtOn.topology_wave
            affinity_class = 'smt-on-unpinned'
            run_id = $smtOn.run_id
            rows_total = $smtOnRows.Count
            smt_on_vs_smt_off_geomean_pct = Get-GeomeanDeltaPct -Rows $smtOnRows -NumeratorField 'head_ns' -DenominatorField 'base_ns'
            interpretation = 'positive delta means SMT-on is slower (higher ns/iter), negative delta means faster.'
        }
    } else {
        [pscustomobject]@{
            present = $false
            note = 'No smt-on-comparative run with affinity class smt-on-unpinned was found in this manifest.'
        }
    }

    Write-Host "Mode: topology-manifest"
    Write-Host "Manifest: $manifestPath"
    Write-Host "Baseline: $basePath"
    Write-Host "Run summaries: $($runSummaries.Count)"
    Write-Host ""
    Write-Host 'Run summary vs baseline:'
    $runSummaries |
        Sort-Object topology_wave, affinity_class |
        Select-Object `
            topology_wave, affinity_class, run_id, benches_compared, improved, worsened, unchanged, missing_in_head, missing_in_base, `
            @{ Name = 'geomean_delta_pct'; Expression = { if ([double]::IsNaN($_.geomean_delta_pct)) { [double]::NaN } else { [Math]::Round($_.geomean_delta_pct, 3) } } } |
        Format-Table -AutoSize

    Write-Host ""
    Write-Host 'Locality summary (geomean deltas):'
    $sameGeomeanText = if ([double]::IsNaN($topologySummary.same_avg_vs_base_geomean_pct)) { 'NaN' } else { ('{0:N3}' -f $topologySummary.same_avg_vs_base_geomean_pct) }
    $crossGeomeanText = if ([double]::IsNaN($topologySummary.cross_vs_base_geomean_pct)) { 'NaN' } else { ('{0:N3}' -f $topologySummary.cross_vs_base_geomean_pct) }
    $crossVsSameGeomeanText = if ([double]::IsNaN($topologySummary.cross_vs_same_avg_geomean_pct)) { 'NaN' } else { ('{0:N3}' -f $topologySummary.cross_vs_same_avg_geomean_pct) }
    Write-Host ("same_avg_vs_base_geomean_pct: {0}" -f $sameGeomeanText)
    Write-Host ("cross_vs_base_geomean_pct: {0}" -f $crossGeomeanText)
    Write-Host ("cross_vs_same_avg_geomean_pct: {0}" -f $crossVsSameGeomeanText)

    Write-Host ""
    Write-Host 'SMT-on summary (vs SMT-off baseline):'
    if ($smtOnSummary.present) {
        $smtOnGeomeanText = if ([double]::IsNaN($smtOnSummary.smt_on_vs_smt_off_geomean_pct)) { 'NaN' } else { ('{0:N3}' -f $smtOnSummary.smt_on_vs_smt_off_geomean_pct) }
        Write-Host ("run_id: {0}" -f $smtOnSummary.run_id)
        Write-Host ("affinity_class: {0}" -f $smtOnSummary.affinity_class)
        Write-Host ("smt_on_vs_smt_off_geomean_pct: {0}" -f $smtOnGeomeanText)
    } else {
        Write-Host $smtOnSummary.note
    }

    Write-Host ""
    Write-Host ("Locality rows: {0}" -f $topologyRows.Count)
    $sortedTopologyRows = if ($SortByAbsDelta) {
        $topologyRows | Sort-Object { if ([double]::IsNaN($_.cross_vs_same_avg_pct)) { -1 } else { [Math]::Abs($_.cross_vs_same_avg_pct) } } -Descending
    } else {
        $topologyRows | Sort-Object { if ([double]::IsNaN($_.cross_vs_same_avg_pct)) { -1 } else { $_.cross_vs_same_avg_pct } } -Descending
    }

    $sortedTopologyRows |
        Select-Object `
            bench, status, base_ns, same_primary_ns, same_secondary_ns, same_avg_ns, cross_ns, `
            @{ Name = 'same_avg_vs_base_pct'; Expression = { if ([double]::IsNaN($_.same_avg_vs_base_pct)) { [double]::NaN } else { [Math]::Round($_.same_avg_vs_base_pct, 3) } } }, `
            @{ Name = 'cross_vs_base_pct'; Expression = { if ([double]::IsNaN($_.cross_vs_base_pct)) { [double]::NaN } else { [Math]::Round($_.cross_vs_base_pct, 3) } } }, `
            @{ Name = 'cross_vs_same_avg_pct'; Expression = { if ([double]::IsNaN($_.cross_vs_same_avg_pct)) { [double]::NaN } else { [Math]::Round($_.cross_vs_same_avg_pct, 3) } } } |
        Format-Table -AutoSize

    $report = [ordered]@{
        mode = 'topology-manifest'
        generated_utc = (Get-Date).ToUniversalTime().ToString('o')
        manifest_path = $manifestPath
        baseline_path = $basePath
        include_regex = $IncludeRegex
        exclude_regex = $ExcludeRegex
        only_changed = [bool]$OnlyChanged
        run_summaries = $runSummaries
        locality_summary = $topologySummary
        locality_rows = $topologyRows
        smt_on_summary = $smtOnSummary
        smt_on_rows = $smtOnRows
    }

    if ($OutReportPath) {
        $resolvedOut = if ([System.IO.Path]::IsPathRooted($OutReportPath)) {
            $OutReportPath
        } else {
            Join-Path $repoRoot $OutReportPath
        }

        $report | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $resolvedOut -Encoding utf8
        Write-Host "Report JSON: $resolvedOut"
    }

    if ($EmitReportObject) {
        [pscustomobject]$report
    }

    return
}

if (-not $basePath) {
    $defaultBase = Join-Path $repoRoot 'target/bench-policy/basic.results.json'
    if (Test-Path -LiteralPath $defaultBase) {
        $basePath = (Resolve-Path -LiteralPath $defaultBase).Path
    }
}

if (-not $basePath -or -not $headPath) {
    throw 'compare_basic_results requires Base and Head results. Provide -BaseResultsPath/-HeadResultsPath or -BaseRunDir/-HeadRunDir.'
}

$base = Read-JsonObject -Path $basePath
$head = Read-JsonObject -Path $headPath
$rows = Get-JoinedBenchRows -Base $base -Head $head -IncludeRegex $IncludeRegex -ExcludeRegex $ExcludeRegex -OnlyChanged:$OnlyChanged

Write-Host "Mode: pairwise"
Write-Host "Base: $basePath"
Write-Host "Head: $headPath"
Write-Host ("Rows: {0}, missing_in_head: {1}, missing_in_base: {2}" -f `
    ($rows.Count), `
    (($rows | Where-Object status -eq 'missing_in_head').Count), `
    (($rows | Where-Object status -eq 'missing_in_base').Count) `
)

if ($SortByAbsDelta) {
    $rows |
        Sort-Object { if ([double]::IsNaN($_.delta_ns)) { -1 } else { [Math]::Abs($_.delta_ns) } } -Descending |
        Format-Table -AutoSize
} else {
    $rows |
        Sort-Object { if ([double]::IsNaN($_.delta_pct)) { -1 } else { $_.delta_pct } } -Descending |
        Format-Table -AutoSize
}
