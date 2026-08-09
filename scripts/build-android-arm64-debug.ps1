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
$jniRoot = Join-Path $androidProject 'app\src\main\jniLibs'
$rustLoader = Join-Path $androidProject 'app\src\main\java\io\github\space2233\pixnya\generated\Rust.kt'

# Keep reusable dependencies but do not accumulate incremental sessions during
# one-off APK builds.
$env:CARGO_INCREMENTAL = '0'

function Remove-StaleTauriNativeLibraryLinks {
    param(
        [Parameter(Mandatory)]
        [string]$JniRoot,

        [Parameter(Mandatory)]
        [string]$ExpectedLibrary,

        [Parameter(Mandatory)]
        [string]$TargetRoot
    )

    if (-not (Test-Path -LiteralPath $JniRoot -PathType Container)) {
        return
    }

    $resolvedTargetRoot = [System.IO.Path]::GetFullPath($TargetRoot).TrimEnd('\', '/')
    $targetPrefix = $resolvedTargetRoot + [System.IO.Path]::DirectorySeparatorChar
    $applicationLibraries = @(
        Get-ChildItem -LiteralPath $JniRoot -Recurse -File -Filter 'lib*_lib.so'
    )

    foreach ($library in $applicationLibraries) {
        if ($library.Name -eq $ExpectedLibrary) {
            continue
        }
        if ($library.LinkType -ne 'SymbolicLink') {
            throw "Refusing to remove unexpected native library because it is not a symbolic link: $($library.FullName)"
        }

        $linkTargets = @($library.Target)
        if ($linkTargets.Count -ne 1) {
            throw "Refusing to remove native library with an ambiguous link target: $($library.FullName)"
        }
        $linkTarget = $linkTargets[0]
        if (-not [System.IO.Path]::IsPathRooted($linkTarget)) {
            $linkTarget = Join-Path $library.DirectoryName $linkTarget
        }
        $resolvedLinkTarget = [System.IO.Path]::GetFullPath($linkTarget)
        if (-not $resolvedLinkTarget.StartsWith($targetPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
            throw "Refusing to remove native library linked outside the project target directory: $($library.FullName)"
        }

        Write-Output "Removing stale Tauri native library link: $($library.FullName)"
        Remove-Item -LiteralPath $library.FullName -Force
    }
}

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

if (-not (Test-Path -LiteralPath $rustLoader -PathType Leaf)) {
    throw 'The generated Rust library loader was not found. Run tauri android init first.'
}

$rustLoaderSource = Get-Content -LiteralPath $rustLoader -Raw
$rustLibraryMatches = [regex]::Matches($rustLoaderSource, 'System\.loadLibrary\("(?<name>[A-Za-z0-9_]+)"\)')
if ($rustLibraryMatches.Count -ne 1) {
    throw 'Could not determine exactly one active Rust library from the generated Android loader.'
}
$expectedRustLibrary = "lib$($rustLibraryMatches[0].Groups['name'].Value).so"

# Make the F-drive toolchain self-contained even when this PowerShell session
# was opened before the user-level environment variables were installed.
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

# Keep debug APKs suitable for device testing without shipping large Rust debug symbols.
$env:CARGO_PROFILE_DEV_DEBUG = '0'
$env:CARGO_PROFILE_DEV_STRIP = 'debuginfo'

Push-Location $projectRoot
try {
    Remove-StaleTauriNativeLibraryLinks `
        -JniRoot $jniRoot `
        -ExpectedLibrary $expectedRustLibrary `
        -TargetRoot (Join-Path $projectRoot 'target')

    # Gradle's incremental APK task can leave unreferenced ZIP data behind and
    # make a debug package tens of MiB larger after repeated builds.
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

    & $tauri android build --debug --target aarch64 --split-per-abi --apk --ci
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    & (Join-Path $PSScriptRoot 'check-android-arm64-apk.ps1') `
        -ApkPath (Join-Path $androidProject 'app\build\outputs\apk\arm64\debug\app-arm64-debug.apk') `
        -ExpectedRustLibrary $expectedRustLibrary

    & (Join-Path $PSScriptRoot 'collect-artifacts.ps1') -Kind Android
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    & (Join-Path $PSScriptRoot 'audit-target-storage.ps1') -WarnAboveGiB 80
}
finally {
    Pop-Location
}
