$ErrorActionPreference = 'Stop'

function Get-ActiveTauriAndroidLibrary {
    param(
        [Parameter(Mandatory)]
        [string]$RustLoader
    )

    if (-not (Test-Path -LiteralPath $RustLoader -PathType Leaf)) {
        throw 'The generated Rust library loader was not found. Run tauri android init first.'
    }
    $source = Get-Content -LiteralPath $RustLoader -Raw
    $matches = [regex]::Matches($source, 'System\.loadLibrary\("(?<name>[A-Za-z0-9_]+)"\)')
    if ($matches.Count -ne 1) {
        throw 'Could not determine exactly one active Rust library from the generated Android loader.'
    }
    "lib$($matches[0].Groups['name'].Value).so"
}

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
