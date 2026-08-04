$ErrorActionPreference = 'Stop'

$projectRoot = Split-Path -Parent $PSScriptRoot
$workspaceRoot = Split-Path -Parent $projectRoot
$toolchainRoot = Join-Path $workspaceRoot '.toolchains'
$rustRoot = Join-Path $toolchainRoot 'rust'
$androidRoot = Join-Path $toolchainRoot 'android'
$sdkRoot = Join-Path $androidRoot 'sdk'
$jdkRoot = Join-Path $toolchainRoot 'jdk-17'
$tauri = Join-Path $projectRoot 'node_modules\.bin\tauri.cmd'
$androidProject = Join-Path $projectRoot 'src-tauri\gen\android'
$gradleWrapper = Join-Path $androidProject 'gradlew.bat'

. (Join-Path $PSScriptRoot 'import-oauth-env.ps1')
Import-OAuthEnvironment `
    -EnvironmentFile (Join-Path $projectRoot '.env.oauth.local') `
    -RequireComplete

if (-not (Test-Path -LiteralPath $tauri -PathType Leaf)) {
    throw 'Tauri CLI was not found. Run npm install first.'
}

if (-not (Test-Path -LiteralPath $gradleWrapper -PathType Leaf)) {
    throw 'The generated Android project was not found. Run tauri android init first.'
}

if (Test-Path -LiteralPath $jdkRoot -PathType Container) {
    $env:JAVA_HOME = $jdkRoot
}
if (Test-Path -LiteralPath $sdkRoot -PathType Container) {
    $env:ANDROID_HOME = $sdkRoot
    $env:ANDROID_SDK_ROOT = $sdkRoot
    $env:ANDROID_USER_HOME = Join-Path $androidRoot 'user'
    $env:ANDROID_AVD_HOME = Join-Path $androidRoot 'avd'
    $env:GRADLE_USER_HOME = Join-Path $toolchainRoot 'gradle'
    $env:NDK_HOME = Join-Path $sdkRoot 'ndk\29.0.14206865'
    $env:ANDROID_NDK_ROOT = $env:NDK_HOME
}
if (Test-Path -LiteralPath $rustRoot -PathType Container) {
    $env:CARGO_HOME = Join-Path $rustRoot 'cargo'
    $env:RUSTUP_HOME = Join-Path $rustRoot 'rustup'
}

$pathAdditions = @(
    (Join-Path $rustRoot 'cargo\bin'),
    (Join-Path $jdkRoot 'bin'),
    (Join-Path $sdkRoot 'platform-tools')
) | Where-Object { Test-Path -LiteralPath $_ -PathType Container }
$env:Path = (($pathAdditions + @($env:Path)) -join ';')

$env:CARGO_PROFILE_DEV_DEBUG = '0'
$env:CARGO_PROFILE_DEV_STRIP = 'debuginfo'

Push-Location $projectRoot
try {
    Push-Location $androidProject
    try {
        & $gradleWrapper clean --no-daemon
        if ($LASTEXITCODE -ne 0) {
            exit $LASTEXITCODE
        }
    }
    finally {
        Pop-Location
    }

    & $tauri android build --debug --target armv7 --split-per-abi --apk --ci
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    & (Join-Path $PSScriptRoot 'collect-artifacts.ps1') -Kind Android
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
}
finally {
    Pop-Location
}
