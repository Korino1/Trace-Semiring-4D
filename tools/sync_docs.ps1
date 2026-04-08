param(
    [Parameter(Mandatory = $true)]
    [string]$SourcePath,
    [string]$DestinationPath
)

# Sync spec text into docs without hardcoded local drive assumptions.
$repoRoot = Split-Path $PSScriptRoot -Parent
if (-not $DestinationPath) {
    $DestinationPath = Join-Path $repoRoot 'docs\4d_sections_clean.txt'
}

$SourcePath = (Resolve-Path -LiteralPath $SourcePath).Path
$destinationDir = Split-Path -Parent $DestinationPath
if ($destinationDir -and -not (Test-Path -LiteralPath $destinationDir)) {
    New-Item -ItemType Directory -Path $destinationDir -Force | Out-Null
}

Copy-Item -LiteralPath $SourcePath -Destination $DestinationPath -Force
