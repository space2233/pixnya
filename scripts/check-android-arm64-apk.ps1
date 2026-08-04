param(
    [Parameter(Mandatory)]
    [string]$ApkPath,

    [string]$ExpectedAbi = 'arm64-v8a',

    [string]$ExpectedRustLibrary = 'libpixnya_lib.so',

    [long]$MaximumBytes = 90MB
)

$ErrorActionPreference = 'Stop'

$apk = Get-Item -LiteralPath $ApkPath -ErrorAction Stop
if ($apk.Length -gt $MaximumBytes) {
    throw "APK is $([math]::Round($apk.Length / 1MB, 2)) MiB; the limit is $([math]::Round($MaximumBytes / 1MB, 2)) MiB."
}

Add-Type -AssemblyName System.IO.Compression.FileSystem
$archive = [System.IO.Compression.ZipFile]::OpenRead($apk.FullName)
try {
    $nativeEntries = @($archive.Entries | Where-Object { $_.FullName -match '^lib/[^/]+/[^/]+\.so$' })
    $expectedPath = "lib/$ExpectedAbi/$ExpectedRustLibrary"
    if ($nativeEntries.FullName -notcontains $expectedPath) {
        throw "APK does not contain the expected Rust library: $expectedPath"
    }

    $unexpectedAbis = @(
        $nativeEntries.FullName |
            ForEach-Object { ($_ -split '/')[1] } |
            Where-Object { $_ -ne $ExpectedAbi } |
            Sort-Object -Unique
    )
    if ($unexpectedAbis.Count -gt 0) {
        throw "APK contains unexpected ABIs: $($unexpectedAbis -join ', ')"
    }

    $applicationLibraries = @(
        $nativeEntries |
            Where-Object { $_.Name -like 'lib*_lib.so' }
    )
    if ($applicationLibraries.Count -ne 1) {
        throw "APK must contain exactly one Rust application library, found: $($applicationLibraries.FullName -join ', ')"
    }
}
finally {
    $archive.Dispose()
}

Write-Output "Android APK check passed: $($apk.FullName) ($([math]::Round($apk.Length / 1MB, 2)) MiB, $ExpectedAbi)."
