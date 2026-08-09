param(
    [ValidateRange(0, 100000)]
    [double]$WarnAboveGiB = 80,

    [ValidateRange(0, 100000)]
    [double]$FailAboveGiB = 0
)

$ErrorActionPreference = 'Stop'

$projectRoot = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot)).TrimEnd('\', '/')
$targetRoot = [System.IO.Path]::GetFullPath((Join-Path $projectRoot 'target')).TrimEnd('\', '/')

function Get-DirectoryStats {
    param(
        [Parameter(Mandatory)]
        [string]$Name,

        [Parameter(Mandatory)]
        [string]$Path
    )

    if (-not (Test-Path -LiteralPath $Path -PathType Container)) {
        return [pscustomobject]@{
            Name  = $Name
            Bytes = [int64]0
            GiB   = [double]0
            Files = [int64]0
        }
    }

    $measurement = Get-ChildItem -LiteralPath $Path -File -Recurse -Force -ErrorAction SilentlyContinue |
        Measure-Object -Property Length -Sum
    $bytes = if ($null -eq $measurement.Sum) { [int64]0 } else { [int64]$measurement.Sum }

    [pscustomobject]@{
        Name  = $Name
        Bytes = $bytes
        GiB   = [math]::Round($bytes / 1GB, 3)
        Files = [int64]$measurement.Count
    }
}

if (-not (Test-Path -LiteralPath $targetRoot -PathType Container)) {
    Write-Output "Cargo target directory does not exist: $targetRoot"
    exit 0
}

$rows = @(
    Get-DirectoryStats -Name 'Windows debug' -Path (Join-Path $targetRoot 'debug')
    Get-DirectoryStats -Name 'Windows incremental' -Path (Join-Path $targetRoot 'debug\incremental')
    Get-DirectoryStats -Name 'Windows deps' -Path (Join-Path $targetRoot 'debug\deps')
    Get-DirectoryStats -Name 'Windows build' -Path (Join-Path $targetRoot 'debug\build')
    Get-DirectoryStats -Name 'Android ARM64' -Path (Join-Path $targetRoot 'aarch64-linux-android')
    Get-DirectoryStats -Name 'ARM64 incremental' -Path (Join-Path $targetRoot 'aarch64-linux-android\debug\incremental')
    Get-DirectoryStats -Name 'ARM64 deps' -Path (Join-Path $targetRoot 'aarch64-linux-android\debug\deps')
    Get-DirectoryStats -Name 'ARM64 build' -Path (Join-Path $targetRoot 'aarch64-linux-android\debug\build')
    Get-DirectoryStats -Name 'Android ARM32' -Path (Join-Path $targetRoot 'armv7-linux-androideabi')
)

$majorRows = $rows | Where-Object { $_.Name -in @('Windows debug', 'Android ARM64', 'Android ARM32') }
$knownMajorBytes = ($majorRows | Measure-Object -Property Bytes -Sum).Sum
$knownMajorFiles = ($majorRows | Measure-Object -Property Files -Sum).Sum
$otherRootFiles = Get-ChildItem -LiteralPath $targetRoot -File -Force -ErrorAction SilentlyContinue
$otherRootMeasurement = $otherRootFiles | Measure-Object -Property Length -Sum
$otherRootBytes = if ($null -eq $otherRootMeasurement.Sum) { [int64]0 } else { [int64]$otherRootMeasurement.Sum }
$otherRootFileCount = [int64]$otherRootMeasurement.Count
$otherRootDirectories = Get-ChildItem -LiteralPath $targetRoot -Directory -Force |
    Where-Object { $_.Name -notin @('debug', 'aarch64-linux-android', 'armv7-linux-androideabi') }

foreach ($directory in $otherRootDirectories) {
    $row = Get-DirectoryStats -Name "Other: $($directory.Name)" -Path $directory.FullName
    $knownMajorBytes += $row.Bytes
    $knownMajorFiles += $row.Files
}

$targetBytes = [int64]$knownMajorBytes + $otherRootBytes
$targetFiles = [int64]$knownMajorFiles + $otherRootFileCount
$targetGiB = [math]::Round($targetBytes / 1GB, 3)

Write-Output "Cargo target storage: $targetRoot"
$rows | Select-Object Name, GiB, Files | Format-Table -AutoSize
[pscustomobject]@{
    Name  = 'TOTAL target'
    GiB   = $targetGiB
    Files = $targetFiles
} | Format-Table -AutoSize

if ($WarnAboveGiB -gt 0 -and $targetGiB -ge $WarnAboveGiB) {
    Write-Warning "target/ is $targetGiB GiB (warning threshold: $WarnAboveGiB GiB). Run 'npm run storage:cleanup:preview' before it grows further."
}

if ($FailAboveGiB -gt 0 -and $targetGiB -ge $FailAboveGiB) {
    Write-Error "target/ is $targetGiB GiB (failure threshold: $FailAboveGiB GiB)."
    exit 2
}
