param(
    [Parameter(Mandatory)]
    [string]$ApkPath,

    [Parameter(Mandatory)]
    [ValidateSet('arm64-v8a', 'armeabi-v7a')]
    [string]$ExpectedAbi,

    [string]$ExpectedRustLibrary = 'libpixnya_lib.so',

    [long]$MaximumBytes = 90MB
)

$ErrorActionPreference = 'Stop'

$apk = Get-Item -LiteralPath $ApkPath -ErrorAction Stop
if ($apk.Length -gt $MaximumBytes) {
    throw "APK is $([math]::Round($apk.Length / 1MB, 2)) MiB; the limit is $([math]::Round($MaximumBytes / 1MB, 2)) MiB."
}

Add-Type -AssemblyName System.IO.Compression.FileSystem

function Assert-NativeLibraryArchitecture {
    param(
        [Parameter(Mandatory)]
        $Entry,

        [Parameter(Mandatory)]
        [int]$ExpectedElfClass,

        [Parameter(Mandatory)]
        [int]$ExpectedMachine,

        [Parameter(Mandatory)]
        [string]$ArchitectureLabel
    )

    # ELF identification plus e_machine ends at byte 19. Read through the ZIP
    # stream so compressed and stored APK entries follow the same validation.
    $header = [byte[]]::new(20)
    $stream = $Entry.Open()
    try {
        $read = 0
        while ($read -lt $header.Length) {
            $count = $stream.Read($header, $read, $header.Length - $read)
            if ($count -eq 0) {
                break
            }
            $read += $count
        }
    }
    finally {
        $stream.Dispose()
    }

    if ($read -lt $header.Length) {
        throw "Native library has a truncated ELF header: $($Entry.FullName)"
    }
    if (
        $header[0] -ne 0x7F -or
        $header[1] -ne 0x45 -or
        $header[2] -ne 0x4C -or
        $header[3] -ne 0x46
    ) {
        throw "Native library has invalid ELF magic: $($Entry.FullName)"
    }
    if ($header[5] -ne 1) {
        throw "Native library is not little-endian ELF: $($Entry.FullName)"
    }
    if ($header[4] -ne $ExpectedElfClass) {
        throw "Expected $ArchitectureLabel ELF class $ExpectedElfClass, found $($header[4]): $($Entry.FullName)"
    }

    $machine = [int]$header[18] -bor ([int]$header[19] -shl 8)
    if ($machine -ne $ExpectedMachine) {
        throw ('Expected {0} ELF machine 0x{1:X2}, found 0x{2:X2}: {3}' -f `
            $ArchitectureLabel, $ExpectedMachine, $machine, $Entry.FullName)
    }
}

$expectedElfClass = if ($ExpectedAbi -eq 'arm64-v8a') { 2 } else { 1 }
$expectedMachine = if ($ExpectedAbi -eq 'arm64-v8a') { 0xB7 } else { 0x28 }
$architectureLabel = if ($ExpectedAbi -eq 'arm64-v8a') { 'ARM64' } else { 'ARMv7' }

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

    foreach ($nativeEntry in $nativeEntries) {
        Assert-NativeLibraryArchitecture `
            -Entry $nativeEntry `
            -ExpectedElfClass $expectedElfClass `
            -ExpectedMachine $expectedMachine `
            -ArchitectureLabel $architectureLabel
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
