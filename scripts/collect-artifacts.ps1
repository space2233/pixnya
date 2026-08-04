param(
    [ValidateSet('All', 'Windows', 'Android')]
    [string]$Kind = 'All'
)

$ErrorActionPreference = 'Stop'

$projectRoot = Split-Path -Parent $PSScriptRoot
$artifactRoot = Join-Path $projectRoot 'artifacts'
$package = Get-Content -LiteralPath (Join-Path $projectRoot 'package.json') -Raw | ConvertFrom-Json
$version = $package.version
$published = [System.Collections.Generic.List[object]]::new()

function Publish-Artifact {
    param(
        [Parameter(Mandatory)]
        [string]$Source,

        [Parameter(Mandatory)]
        [string]$DestinationDirectory,

        [Parameter(Mandatory)]
        [string]$DestinationName
    )

    if (-not (Test-Path -LiteralPath $Source -PathType Leaf)) {
        return
    }

    New-Item -ItemType Directory -Force -Path $DestinationDirectory | Out-Null
    $destination = Join-Path $DestinationDirectory $DestinationName
    Copy-Item -LiteralPath $Source -Destination $destination -Force
    $item = Get-Item -LiteralPath $destination
    $published.Add([pscustomobject]@{
        Path = $item.FullName
        SizeMiB = [math]::Round($item.Length / 1MB, 2)
    })
}

if ($Kind -in @('All', 'Windows')) {
    $windowsOutput = Join-Path $artifactRoot 'windows'
    Publish-Artifact `
    -Source (Join-Path $projectRoot 'target\debug\pixnya.exe') `
        -DestinationDirectory $windowsOutput `
    -DestinationName "pixnya-$version-windows-x64-debug.exe"
    Publish-Artifact `
    -Source (Join-Path $projectRoot 'target\release\pixnya.exe') `
        -DestinationDirectory $windowsOutput `
    -DestinationName "pixnya-$version-windows-x64-release.exe"
}

if ($Kind -in @('All', 'Android')) {
    $androidOutput = Join-Path $artifactRoot 'android'
    $apkRoot = Join-Path $projectRoot 'src-tauri\gen\android\app\build\outputs\apk'

    if (Test-Path -LiteralPath $apkRoot -PathType Container) {
        Get-ChildItem -LiteralPath $apkRoot -Filter '*.apk' -File -Recurse | ForEach-Object {
            Publish-Artifact `
                -Source $_.FullName `
                -DestinationDirectory $androidOutput `
      -DestinationName "pixnya-$version-android-$($_.BaseName).apk"
        }
    }
}

if ($published.Count -eq 0) {
    throw "No $Kind build artifacts were found. Run the corresponding build first."
}

$checksumPath = Join-Path $artifactRoot 'SHA256SUMS.txt'
$checksumLines = Get-ChildItem -LiteralPath $artifactRoot -File -Recurse |
    Where-Object { $_.Extension -in @('.exe', '.apk') } |
    Sort-Object FullName |
    ForEach-Object {
        $relativePath = $_.FullName.Substring($artifactRoot.Length + 1).Replace('\', '/')
        $hash = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        "$hash  $relativePath"
    }

Set-Content -LiteralPath $checksumPath -Value $checksumLines -Encoding utf8

$published | Format-Table -AutoSize
Write-Output "Checksums: $checksumPath"
