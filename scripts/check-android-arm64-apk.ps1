param(
    [Parameter(Mandatory)]
    [string]$ApkPath,

    [string]$ExpectedAbi = 'arm64-v8a',

    [string]$ExpectedRustLibrary = 'libpixnya_lib.so',

    [long]$MaximumBytes = 90MB
)

& (Join-Path $PSScriptRoot 'check-android-apk.ps1') `
    -ApkPath $ApkPath `
    -ExpectedAbi $ExpectedAbi `
    -ExpectedRustLibrary $ExpectedRustLibrary `
    -MaximumBytes $MaximumBytes
