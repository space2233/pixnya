param(
    [Parameter(Mandatory)]
    [string]$Executable,

    [Parameter(Mandatory)]
    [ValidateSet('x64', 'arm64')]
    [string]$ExpectedArchitecture
)

$ErrorActionPreference = 'Stop'

$resolvedExecutable = (Resolve-Path -LiteralPath $Executable -ErrorAction Stop).Path
$bytes = [System.IO.File]::ReadAllBytes($resolvedExecutable)

function Assert-AvailableRange {
    param(
        [int64]$Offset,
        [int64]$Length
    )
    if ($Offset -lt 0 -or $Length -lt 0 -or $Offset -gt ($bytes.LongLength - $Length)) {
        throw "$resolvedExecutable is not a valid PE executable."
    }
}

Assert-AvailableRange -Offset 0 -Length 64
if ([BitConverter]::ToUInt16($bytes, 0) -ne 0x5A4D) {
    throw "$resolvedExecutable is not a valid PE executable (missing DOS signature)."
}

$peOffset = [BitConverter]::ToUInt32($bytes, 0x3C)
Assert-AvailableRange -Offset $peOffset -Length 24
if (
    $bytes[$peOffset] -ne 0x50 -or
    $bytes[$peOffset + 1] -ne 0x45 -or
    $bytes[$peOffset + 2] -ne 0 -or
    $bytes[$peOffset + 3] -ne 0
) {
    throw "$resolvedExecutable is not a valid PE executable (missing PE signature)."
}

$machine = [BitConverter]::ToUInt16($bytes, $peOffset + 4)
$expectedMachine = if ($ExpectedArchitecture -eq 'arm64') { 0xAA64 } else { 0x8664 }
$architectureLabel = $ExpectedArchitecture.ToUpperInvariant()
if ($machine -ne $expectedMachine) {
    throw ('Expected {0} PE machine 0x{1:X4}, found 0x{2:X4} in {3}' -f `
        $architectureLabel, $expectedMachine, $machine, $resolvedExecutable)
}

$optionalHeaderSize = [BitConverter]::ToUInt16($bytes, $peOffset + 20)
$optionalHeaderOffset = $peOffset + 24
Assert-AvailableRange -Offset $optionalHeaderOffset -Length $optionalHeaderSize
if ($optionalHeaderSize -lt 70) {
    throw "$resolvedExecutable is not a valid PE executable (optional header is truncated)."
}

$optionalHeaderMagic = [BitConverter]::ToUInt16($bytes, $optionalHeaderOffset)
if ($optionalHeaderMagic -ne 0x20B) {
    throw ('Expected a PE32+ executable, found optional-header magic 0x{0:X4} in {1}' -f `
        $optionalHeaderMagic, $resolvedExecutable)
}

$subsystem = [BitConverter]::ToUInt16($bytes, $optionalHeaderOffset + 68)
if ($subsystem -ne 2) {
    throw "Expected Windows GUI subsystem (2), found $subsystem in $resolvedExecutable"
}

Write-Output ('Windows PE check passed: {0} (machine 0x{1:X4}, GUI subsystem 2).' -f `
    $resolvedExecutable, $machine)
