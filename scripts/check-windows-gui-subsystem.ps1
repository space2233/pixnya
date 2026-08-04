param(
  [string]$Executable = (Join-Path $PSScriptRoot '..\target\debug\pixnya.exe')
)

$resolvedExecutable = (Resolve-Path -LiteralPath $Executable).Path
$bytes = [System.IO.File]::ReadAllBytes($resolvedExecutable)
$peOffset = [BitConverter]::ToInt32($bytes, 0x3c)
$subsystem = [BitConverter]::ToUInt16($bytes, $peOffset + 24 + 68)

if ($subsystem -ne 2) {
    throw "Expected Windows GUI subsystem (2), but found $subsystem in $resolvedExecutable"
}

Write-Host "PASS: Windows GUI subsystem (2): $resolvedExecutable"
