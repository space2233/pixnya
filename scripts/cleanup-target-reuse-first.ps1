param(
    [switch]$Execute
)

$ErrorActionPreference = 'Stop'

$projectRoot = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot)).TrimEnd('\', '/')
$targetRoot = [System.IO.Path]::GetFullPath((Join-Path $projectRoot 'target')).TrimEnd('\', '/')
$expectedTargetRoot = [System.IO.Path]::GetFullPath("$projectRoot\target").TrimEnd('\', '/')

if (-not $targetRoot.Equals($expectedTargetRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing to continue because the target root is not the repository target directory: $targetRoot"
}

if (-not (Test-Path -LiteralPath $targetRoot -PathType Container)) {
    Write-Output "Cargo target directory does not exist: $targetRoot"
    exit 0
}

$targetPrefix = $targetRoot + [System.IO.Path]::DirectorySeparatorChar
$candidates = [System.Collections.Generic.List[object]]::new()
$seen = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)

function Add-CleanupCandidate {
    param(
        [Parameter(Mandatory)]
        [System.IO.FileSystemInfo]$Item,

        [Parameter(Mandatory)]
        [string]$Category
    )

    $fullPath = [System.IO.Path]::GetFullPath($Item.FullName)
    if (-not $fullPath.StartsWith($targetPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing candidate outside target/: $fullPath"
    }
    if ($fullPath.Equals($targetRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw 'Refusing to select the target root itself.'
    }
    if (($Item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw "Refusing symbolic-link or junction candidate: $fullPath"
    }
    if ($seen.Add($fullPath)) {
        $candidates.Add([pscustomobject]@{
            Category = $Category
            Item     = $Item
            FullPath = $fullPath
        })
    }
}

function Add-PathIfPresent {
    param(
        [Parameter(Mandatory)]
        [string]$Path,

        [Parameter(Mandatory)]
        [string]$Category
    )

    $item = Get-Item -LiteralPath $Path -Force -ErrorAction SilentlyContinue
    if ($null -ne $item) {
        Add-CleanupCandidate -Item $item -Category $Category
    }
}

function Add-OldMainApplicationArtifacts {
    param(
        [Parameter(Mandatory)]
        [string]$DebugRoot,

        [Parameter(Mandatory)]
        [string]$PlatformName
    )

    $incrementalRoot = Join-Path $DebugRoot 'incremental'
    if (Test-Path -LiteralPath $incrementalRoot -PathType Container) {
        Get-ChildItem -LiteralPath $incrementalRoot -Directory -Force |
            Where-Object { $_.Name -match '^pixiv_client_lib-[0-9a-z]+$' } |
            ForEach-Object { Add-CleanupCandidate -Item $_ -Category "$PlatformName old incremental" }
    }

    $depsRoot = Join-Path $DebugRoot 'deps'
    if (Test-Path -LiteralPath $depsRoot -PathType Container) {
        Get-ChildItem -LiteralPath $depsRoot -File -Force |
            Where-Object {
                $_.Name -match '^(lib)?pixiv_client_lib(?:[-.]|$)' -or
                $_.Name -match '^pixiv_client(?:[-.]|$)' -or
                $_.Name -match '^pixiv-client(?:[-.]|$)'
            } |
            ForEach-Object { Add-CleanupCandidate -Item $_ -Category "$PlatformName old deps" }
    }

    foreach ($directoryName in @('build', '.fingerprint')) {
        $directoryRoot = Join-Path $DebugRoot $directoryName
        if (Test-Path -LiteralPath $directoryRoot -PathType Container) {
            Get-ChildItem -LiteralPath $directoryRoot -Directory -Force |
                Where-Object { $_.Name -match '^pixiv-client-[0-9a-f]+$' } |
                ForEach-Object { Add-CleanupCandidate -Item $_ -Category "$PlatformName old $directoryName" }
        }
    }

    Get-ChildItem -LiteralPath $DebugRoot -File -Force -ErrorAction SilentlyContinue |
        Where-Object {
            $_.Name -match '^(lib)?pixiv_client_lib(?:[.-]|$)' -or
            $_.Name -match '^pixiv_client(?:[.-]|$)' -or
            $_.Name -match '^pixiv-client(?:[.-]|$)'
        } |
        ForEach-Object { Add-CleanupCandidate -Item $_ -Category "$PlatformName old root output" }
}

Add-PathIfPresent -Path (Join-Path $targetRoot 'armv7-linux-androideabi') -Category 'Paused Android ARM32 target'
Add-PathIfPresent -Path (Join-Path $targetRoot 'debug\examples') -Category 'Rebuildable examples'
Add-PathIfPresent -Path (Join-Path $targetRoot 'tmp') -Category 'Cargo temporary directory'

Get-ChildItem -LiteralPath $targetRoot -Directory -Force |
    Where-Object {
        $_.Name -match '^webview-e2e-' -or
        $_.Name -match '^windows-standalone-runtime-'
    } |
    ForEach-Object { Add-CleanupCandidate -Item $_ -Category 'Test runtime temporary directory' }

Add-OldMainApplicationArtifacts -DebugRoot (Join-Path $targetRoot 'debug') -PlatformName 'Windows'
Add-OldMainApplicationArtifacts -DebugRoot (Join-Path $targetRoot 'aarch64-linux-android\debug') -PlatformName 'Android ARM64'

$candidateStats = foreach ($candidate in $candidates) {
    if ($candidate.Item.PSIsContainer) {
        $measurement = Get-ChildItem -LiteralPath $candidate.FullPath -File -Recurse -Force -ErrorAction SilentlyContinue |
            Measure-Object -Property Length -Sum
        $bytes = if ($null -eq $measurement.Sum) { [int64]0 } else { [int64]$measurement.Sum }
        $files = [int64]$measurement.Count
    }
    else {
        $bytes = [int64]$candidate.Item.Length
        $files = [int64]1
    }

    [pscustomobject]@{
        Category = $candidate.Category
        FullPath = $candidate.FullPath
        Bytes    = $bytes
        Files    = $files
    }
}

$summary = $candidateStats |
    Group-Object Category |
    ForEach-Object {
        $bytes = ($_.Group | Measure-Object -Property Bytes -Sum).Sum
        $files = ($_.Group | Measure-Object -Property Files -Sum).Sum
        [pscustomobject]@{
            Category = $_.Name
            GiB      = [math]::Round($bytes / 1GB, 3)
            Files    = [int64]$files
            Paths    = $_.Count
        }
    } |
    Sort-Object GiB -Descending

$totalMeasurement = $candidateStats | Measure-Object -Property Bytes -Sum
$fileMeasurement = $candidateStats | Measure-Object -Property Files -Sum
$totalBytes = if ($null -eq $totalMeasurement.Sum) { [int64]0 } else { [int64]$totalMeasurement.Sum }
$totalFiles = if ($null -eq $fileMeasurement.Sum) { [int64]0 } else { [int64]$fileMeasurement.Sum }
$totalGiB = [math]::Round($totalBytes / 1GB, 3)

Write-Output "Reuse-first cleanup root: $targetRoot"
$summary | Format-Table -AutoSize
Write-Output "Selected: $($candidates.Count) paths, $totalFiles files, $totalGiB GiB"

if (-not $Execute) {
    Write-Output 'DRY RUN: nothing was deleted. Re-run with -Execute after reviewing this summary.'
    exit 0
}

$activeBuildProcesses = Get-Process -Name cargo, rustc, gradle, gradlew, tauri -ErrorAction SilentlyContinue
if ($null -ne $activeBuildProcesses) {
    $processSummary = ($activeBuildProcesses | ForEach-Object { "$($_.ProcessName) (PID $($_.Id))" }) -join ', '
    throw "Refusing cleanup while build processes are active: $processSummary"
}

foreach ($candidate in $candidates | Sort-Object { $_.FullPath.Length } -Descending) {
    $resolvedPath = [System.IO.Path]::GetFullPath($candidate.FullPath)
    if (-not $resolvedPath.StartsWith($targetPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Candidate escaped target/ before deletion: $resolvedPath"
    }

    if ($candidate.Item.PSIsContainer) {
        Remove-Item -LiteralPath $resolvedPath -Recurse -Force
    }
    else {
        Remove-Item -LiteralPath $resolvedPath -Force
    }
}

$retainedPaths = @(
    (Join-Path $targetRoot 'debug\pixnya.exe'),
    (Join-Path $targetRoot 'debug\deps'),
    (Join-Path $targetRoot 'debug\build'),
    (Join-Path $targetRoot 'aarch64-linux-android\debug\libpixnya_lib.so'),
    (Join-Path $targetRoot 'aarch64-linux-android\debug\deps'),
    (Join-Path $targetRoot 'aarch64-linux-android\debug\build')
)
$missingRetainedPaths = @($retainedPaths | Where-Object { -not (Test-Path -LiteralPath $_) })
if ($missingRetainedPaths.Count -gt 0) {
    throw "Cleanup completed, but expected retained paths are missing: $($missingRetainedPaths -join ', ')"
}

Write-Output "Cleanup complete. Removed approximately $totalGiB GiB across $totalFiles files."
Write-Output 'Verified current Windows and Android ARM64 outputs/caches are still present.'
