$ErrorActionPreference = 'Stop'

$projectRoot = Split-Path -Parent $PSScriptRoot
$workspaceRoot = Split-Path -Parent $projectRoot
$rustRoot = Join-Path $workspaceRoot '.toolchains\rust'

# Delivery builds are one-off artifacts. Reusing deps/build is useful, while
# accumulating rustc incremental sessions makes target/ grow very quickly.
$env:CARGO_INCREMENTAL = '0'

if (Test-Path -LiteralPath $rustRoot -PathType Container) {
    $env:CARGO_HOME = Join-Path $rustRoot 'cargo'
    $env:RUSTUP_HOME = Join-Path $rustRoot 'rustup'
    $cargoBin = Join-Path $env:CARGO_HOME 'bin'
    $env:Path = "$cargoBin;$env:Path"
}

. (Join-Path $PSScriptRoot 'import-oauth-env.ps1')
Import-OAuthEnvironment `
    -EnvironmentFile (Join-Path $projectRoot '.env.oauth.local') `
    -RequireComplete

Push-Location $projectRoot
try {
    & (Join-Path $projectRoot 'node_modules\.bin\tauri.cmd') build --debug --no-bundle
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    & (Join-Path $PSScriptRoot 'collect-artifacts.ps1') -Kind Windows
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    & (Join-Path $PSScriptRoot 'audit-target-storage.ps1') -WarnAboveGiB 80
}
finally {
    Pop-Location
}
