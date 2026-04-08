param(
    [string]$SpecPath,
    [string]$OutPath
)

$repoRoot = Split-Path $PSScriptRoot -Parent
if (-not $SpecPath) {
    $SpecPath = Join-Path $repoRoot 'docs\4d_sections_clean.txt'
}
if (-not $OutPath) {
    $OutPath = Join-Path $repoRoot 'docs\RS_DOC.md'
}

$SpecPath = (Resolve-Path -LiteralPath $SpecPath).Path
$outDir = Split-Path -Parent $OutPath
if ($outDir -and -not (Test-Path -LiteralPath $outDir)) {
    New-Item -ItemType Directory -Path $outDir -Force | Out-Null
}

$lines = Get-Content -LiteralPath $SpecPath
$toc = @()
foreach ($line in $lines) {
    if ($line -match '^#\s+(\d+)\)\s+(.*)$') {
        $num = $matches[1]
        $title = $matches[2]
        $anchor = ($title.ToLower() -replace '[^a-z0-9\s-]', '') -replace '\s+', '-'
        $toc += "- $num — [$title](#$anchor)"
    }
}

@"
# Спецификация TS4

## Оглавление
"@ | Set-Content -LiteralPath $OutPath -Encoding utf8

if ($toc.Count -gt 0) {
    $toc | Add-Content -LiteralPath $OutPath -Encoding utf8
}

@"

---

"@ | Add-Content -LiteralPath $OutPath -Encoding utf8

Get-Content -LiteralPath $SpecPath | Add-Content -LiteralPath $OutPath -Encoding utf8
