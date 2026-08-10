[CmdletBinding()]
param(
    [string]$Repository = 'space2233/pixnya',
    [string]$Environment = 'production-release',
    [string]$Destination,
    [string]$OAuthEnvironmentFile,
    [string]$KeytoolPath,
    [switch]$UploadSecrets,
    [switch]$UploadExisting
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
if ([string]::IsNullOrWhiteSpace($Destination)) {
    $Destination = Join-Path (Split-Path $projectRoot -Parent) '.release-secrets\pixnya'
}
if ([string]::IsNullOrWhiteSpace($OAuthEnvironmentFile)) {
    $OAuthEnvironmentFile = Join-Path $projectRoot '.env.oauth.local'
}
if ($Repository -notmatch '^[A-Za-z0-9](?:[A-Za-z0-9-]{0,37}[A-Za-z0-9])?/[A-Za-z0-9._-]{1,100}$') {
    throw 'Repository must use the GitHub owner/repository form.'
}
if ($Environment -notmatch '^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$') {
    throw 'Environment must be a simple GitHub environment name.'
}
if ($UploadExisting -and -not $UploadSecrets) {
    throw '-UploadExisting requires -UploadSecrets.'
}

$resolvedDestination = [IO.Path]::GetFullPath($Destination)
$projectBoundary = $projectRoot.TrimEnd([IO.Path]::DirectorySeparatorChar, [IO.Path]::AltDirectorySeparatorChar) +
    [IO.Path]::DirectorySeparatorChar
if ($resolvedDestination.Equals($projectRoot, [StringComparison]::OrdinalIgnoreCase) -or
    $resolvedDestination.StartsWith($projectBoundary, [StringComparison]::OrdinalIgnoreCase)) {
    throw 'Release signing material must be stored outside the Git worktree.'
}
$resolvedParent = [IO.Path]::GetDirectoryName($resolvedDestination)
if ([string]::IsNullOrWhiteSpace($resolvedParent)) {
    throw 'Release signing destination must have a parent directory.'
}
$ancestor = $resolvedParent
while ($ancestor) {
    if (Test-Path -LiteralPath $ancestor) {
        $ancestorItem = Get-Item -LiteralPath $ancestor -Force
        if (($ancestorItem.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
            throw "Release signing paths cannot traverse a junction or symbolic link: $ancestor"
        }
    }
    $parentInfo = [IO.Directory]::GetParent($ancestor)
    $ancestor = if ($null -eq $parentInfo) { $null } else { $parentInfo.FullName }
}
if (-not (Test-Path -LiteralPath $resolvedParent -PathType Container)) {
    New-Item -ItemType Directory -Path $resolvedParent -Force | Out-Null
}
if (Test-Path -LiteralPath $resolvedDestination) {
    $destinationItem = Get-Item -LiteralPath $resolvedDestination -Force
    if (($destinationItem.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw "Release signing destination cannot be a junction or symbolic link: $resolvedDestination"
    }
}
if (Test-Path -LiteralPath $resolvedDestination -PathType Leaf) {
    throw "Release signing destination is a file: $resolvedDestination"
}
if ($UploadExisting) {
    if (-not (Test-Path -LiteralPath $resolvedDestination -PathType Container)) {
        throw "Existing signing material was not found: $resolvedDestination"
    }
    $workingDestination = $resolvedDestination
    $stagingDestination = $null
} else {
    if (Test-Path -LiteralPath $resolvedDestination -PathType Container) {
        $existing = @(Get-ChildItem -LiteralPath $resolvedDestination -Force)
        if ($existing.Count -gt 0) {
            throw "Refusing to overwrite the non-empty signing directory: $resolvedDestination"
        }
    }
    $stagingDestination = Join-Path $resolvedParent ".pixnya-signing-$([Guid]::NewGuid().ToString('N'))"
    New-Item -ItemType Directory -Path $stagingDestination | Out-Null
    $workingDestination = $stagingDestination
}

function Read-ConfirmedSecret {
    param([Parameter(Mandatory)][string]$Prompt)

    $first = Read-Host "$Prompt (store it in your password manager)" -AsSecureString
    $second = Read-Host 'Enter it again' -AsSecureString
    $firstPointer = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($first)
    $secondPointer = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($second)
    try {
        $firstText = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($firstPointer)
        $secondText = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($secondPointer)
        if ($firstText -cne $secondText) { throw 'The two password entries do not match.' }
        if ($firstText.Length -lt 16) { throw 'Signing passwords must contain at least 16 characters.' }
        return $firstText
    } finally {
        [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($firstPointer)
        [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($secondPointer)
    }
}

function Invoke-Checked {
    param(
        [Parameter(Mandatory)][string]$FilePath,
        [Parameter(Mandatory)][string[]]$ArgumentList
    )
    & $FilePath @ArgumentList
    if ($LASTEXITCODE -ne 0) {
        throw "$FilePath failed with exit code $LASTEXITCODE."
    }
}

function Assert-TauriSigningKeyPassword {
    param(
        [Parameter(Mandatory)][string]$CargoPath,
        [Parameter(Mandatory)][string]$PrivateKeyPath,
        [Parameter(Mandatory)][string]$Password,
        [Parameter(Mandatory)][string]$Label,
        [Parameter(Mandatory)][string]$ProbeDirectory
    )

    $probePath = Join-Path $ProbeDirectory ".pixnya-$Label-$([Guid]::NewGuid().ToString('N')).probe"
    $signaturePath = "$probePath.sig"
    $previousKeyPath = [Environment]::GetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY_PATH', 'Process')
    $previousPassword = [Environment]::GetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY_PASSWORD', 'Process')
    [IO.File]::WriteAllText(
        $probePath,
        "PixNya $Label signing key password check",
        [Text.UTF8Encoding]::new($false)
    )
    try {
        $env:TAURI_SIGNING_PRIVATE_KEY_PATH = $PrivateKeyPath
        $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = $Password
        Invoke-Checked $CargoPath @('tauri', 'signer', 'sign', $probePath)
        if (-not (Test-Path -LiteralPath $signaturePath -PathType Leaf)) {
            throw "Tauri did not create the $Label password-check signature."
        }
    } finally {
        if ($null -eq $previousKeyPath) {
            Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY_PATH -ErrorAction SilentlyContinue
        } else {
            $env:TAURI_SIGNING_PRIVATE_KEY_PATH = $previousKeyPath
        }
        if ($null -eq $previousPassword) {
            Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD -ErrorAction SilentlyContinue
        } else {
            $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = $previousPassword
        }
        Remove-Item -LiteralPath $probePath, $signaturePath -Force -ErrorAction SilentlyContinue
    }
}

function Assert-TauriPublicKeyFormat {
    param(
        [Parameter(Mandatory)][string]$Path,
        [Parameter(Mandatory)][string]$Label
    )

    try {
        $encoded = [IO.File]::ReadAllText($Path).Trim()
        $decoded = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($encoded))
    } catch {
        throw "$Label is not the single-Base64 Tauri public-key format: $Path"
    }
    if (-not $decoded.StartsWith('untrusted comment:') -or -not $decoded.Contains("`nRW")) {
        throw "$Label does not decode to a complete minisign public-key file: $Path"
    }
}

function Assert-AndroidSigningKeyPasswords {
    param(
        [Parameter(Mandatory)][string]$Keytool,
        [Parameter(Mandatory)][string]$KeyStore,
        [Parameter(Mandatory)][string]$Alias,
        [Parameter(Mandatory)][string]$ProbeDirectory
    )

    $probeStore = Join-Path $ProbeDirectory ".pixnya-keystore-$([Guid]::NewGuid().ToString('N')).jks"
    try {
        Invoke-Checked $Keytool @(
            '-J-Duser.language=en', '-importkeystore', '-noprompt',
            '-srckeystore', $KeyStore, '-srcstoretype', 'JKS',
            '-srcstorepass:env', 'PIXNYA_PROVISION_STORE_PASSWORD',
            '-srcalias', $Alias, '-srckeypass:env', 'PIXNYA_PROVISION_KEY_PASSWORD',
            '-destkeystore', $probeStore, '-deststoretype', 'JKS',
            '-deststorepass:env', 'PIXNYA_PROVISION_STORE_PASSWORD',
            '-destkeypass:env', 'PIXNYA_PROVISION_KEY_PASSWORD'
        )
        if (-not (Test-Path -LiteralPath $probeStore -PathType Leaf)) {
            throw 'keytool did not create the Android key password-check keystore.'
        }
    } finally {
        Remove-Item -LiteralPath $probeStore -Force -ErrorAction SilentlyContinue
    }
}

function Read-OAuthEnvironment {
    param([Parameter(Mandatory)][string]$Path)

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "OAuth build parameters were not found at $Path"
    }
    $required = @(
        'PIXIV_OAUTH_CLIENT_ID',
        'PIXIV_OAUTH_CLIENT_SECRET',
        'PIXIV_OAUTH_HASH_SALT'
    )
    $values = @{}
    foreach ($line in Get-Content -LiteralPath $Path -Encoding UTF8) {
        $trimmed = $line.Trim()
        if (-not $trimmed -or $trimmed.StartsWith('#') -or -not $trimmed.Contains('=')) { continue }
        $parts = $trimmed.Split('=', 2)
        if ($required -contains $parts[0]) {
            $value = $parts[1].Trim()
            if (($value.StartsWith('"') -and $value.EndsWith('"')) -or
                ($value.StartsWith("'") -and $value.EndsWith("'"))) {
                $value = $value.Substring(1, $value.Length - 2)
            }
            $values[$parts[0]] = $value
        }
    }
    foreach ($name in $required) {
        if (-not $values.ContainsKey($name) -or [string]::IsNullOrWhiteSpace($values[$name])) {
            throw "$Path does not contain a non-empty $name value."
        }
    }
    return $values
}

function Set-GitHubSecretFromMemory {
    param(
        [Parameter(Mandatory)][string]$GhPath,
        [Parameter(Mandatory)][string]$Name,
        [Parameter(Mandatory)][string]$Value
    )

    $startInfo = [Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $GhPath
    $startInfo.Arguments = "secret set $Name --repo $Repository --env $Environment"
    $startInfo.UseShellExecute = $false
    $startInfo.RedirectStandardInput = $true
    $startInfo.RedirectStandardError = $true
    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $startInfo
    if (-not $process.Start()) { throw "Could not start gh for $Name." }
    $process.StandardInput.Write($Value)
    $process.StandardInput.Close()
    $errorText = $process.StandardError.ReadToEnd()
    $process.WaitForExit()
    if ($process.ExitCode -ne 0) {
        throw "GitHub rejected $Name`: $errorText"
    }
}

function Assert-GitHubEnvironmentSecretNames {
    param(
        [Parameter(Mandatory)][string]$GhPath,
        [Parameter(Mandatory)][string[]]$ExpectedNames
    )

    $secretListJson = & $GhPath secret list --repo $Repository --env $Environment --json name
    if ($LASTEXITCODE -ne 0) { throw 'Could not verify the GitHub environment secret names.' }
    $actualNames = @($secretListJson | ConvertFrom-Json | ForEach-Object { $_.name })
    foreach ($name in $ExpectedNames) {
        if ($actualNames -notcontains $name) {
            throw "GitHub environment secret verification did not find $name."
        }
    }
}

$cargo = (Get-Command cargo -ErrorAction Stop).Source
$gh = if ($UploadSecrets) { (Get-Command gh -ErrorAction Stop).Source } else { $null }
$oauth = $null
if ($UploadSecrets) {
    Invoke-Checked $gh @('auth', 'status')
    Invoke-Checked $gh @('api', "repos/$Repository/environments/$Environment", '--silent')
    $oauth = Read-OAuthEnvironment $OAuthEnvironmentFile
}
if ([string]::IsNullOrWhiteSpace($KeytoolPath)) {
    $candidatePaths = @(
        $(if ($env:JAVA_HOME) { Join-Path $env:JAVA_HOME 'bin\keytool.exe' }),
        'F:\ACM\.toolchains\android\android-studio\jbr\bin\keytool.exe'
    ) | Where-Object { $_ -and (Test-Path -LiteralPath $_ -PathType Leaf) }
    $KeytoolPath = $candidatePaths | Select-Object -First 1
}
if (-not $KeytoolPath -or -not (Test-Path -LiteralPath $KeytoolPath -PathType Leaf)) {
    throw 'keytool.exe was not found. Pass -KeytoolPath explicitly.'
}

$updaterPassword = Read-ConfirmedSecret 'Desktop updater key password'
$manifestPassword = Read-ConfirmedSecret 'Android manifest key password'
$keystorePassword = Read-ConfirmedSecret 'Android keystore password'
$androidKeyPassword = Read-ConfirmedSecret 'Android signing key password'
$androidAlias = 'pixnya-release'
$updaterKey = Join-Path $workingDestination 'pixnya-updater.key'
$manifestKey = Join-Path $workingDestination 'pixnya-android-manifest.key'
$keystore = Join-Path $workingDestination 'pixnya-release.jks'

try {
    $env:PIXNYA_PROVISION_STORE_PASSWORD = $keystorePassword
    $env:PIXNYA_PROVISION_KEY_PASSWORD = $androidKeyPassword

    if (-not $UploadExisting) {
        Write-Host 'Tauri will now ask for the desktop updater password. Enter the same password you just confirmed.'
        Invoke-Checked $cargo @('tauri', 'signer', 'generate', '--write-keys', $updaterKey)

        Write-Host 'Tauri will now ask for the Android manifest password. Enter the same password you just confirmed.'
        Invoke-Checked $cargo @('tauri', 'signer', 'generate', '--write-keys', $manifestKey)

        Invoke-Checked $KeytoolPath @(
            '-J-Duser.language=en', '-genkeypair', '-v', '-storetype', 'JKS',
            '-keystore', $keystore, '-storepass:env', 'PIXNYA_PROVISION_STORE_PASSWORD',
            '-keypass:env', 'PIXNYA_PROVISION_KEY_PASSWORD', '-alias', $androidAlias,
            '-keyalg', 'RSA', '-keysize', '4096', '-sigalg', 'SHA256withRSA',
            '-validity', '10000', '-dname', 'CN=PixNya Release, OU=PixNya, O=PixNya'
        )
    }

    $updaterPublicKey = "$updaterKey.pub"
    $manifestPublicKey = "$manifestKey.pub"
    foreach ($path in @($updaterKey, $updaterPublicKey, $manifestKey, $manifestPublicKey, $keystore)) {
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
            throw "Expected signing output was not created: $path"
        }
    }

    Assert-TauriSigningKeyPassword $cargo $updaterKey $updaterPassword 'desktop-updater' $workingDestination
    Assert-TauriSigningKeyPassword $cargo $manifestKey $manifestPassword 'android-manifest' $workingDestination
    Assert-TauriPublicKeyFormat $updaterPublicKey 'Desktop updater public key'
    Assert-TauriPublicKeyFormat $manifestPublicKey 'Android manifest public key'
    Assert-AndroidSigningKeyPasswords $KeytoolPath $keystore $androidAlias $workingDestination

    $previousKeytoolErrorActionPreference = $ErrorActionPreference
    try {
        # Windows PowerShell 5.1 turns keytool's normal JKS warning on stderr into
        # NativeCommandError when stderr is captured under Stop. The process exit
        # code remains the authoritative success signal for this native command.
        $ErrorActionPreference = 'Continue'
        $certificateOutput = & $KeytoolPath '-J-Duser.language=en' -list -v `
            -keystore $keystore -storepass:env PIXNYA_PROVISION_STORE_PASSWORD -alias $androidAlias 2>&1
        $certificateExitCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $previousKeytoolErrorActionPreference
    }
    $certificateLines = @($certificateOutput | ForEach-Object { $_.ToString() })
    if ($certificateExitCode -ne 0) {
        $certificateDetails = $certificateLines -join [Environment]::NewLine
        throw "Could not inspect the generated Android certificate (exit code $certificateExitCode): $certificateDetails"
    }
    $certificateSha256 = ($certificateLines | Select-String 'SHA256:' | Select-Object -First 1).Line
    if (-not $certificateSha256) { throw 'Android certificate SHA-256 was not reported by keytool.' }

    if (-not $UploadExisting) {
        @"
# PixNya production signing recovery record

Generated: $([DateTime]::UtcNow.ToString('yyyy-MM-ddTHH:mm:ssZ'))
Repository: $Repository
GitHub environment: $Environment
Android alias: $androidAlias
$certificateSha256

The four passwords are deliberately not stored in this directory. Keep them in a
password manager, then copy this complete directory to two encrypted offline media.
Losing either the files or their passwords prevents trustworthy in-place updates.
"@ | Set-Content -LiteralPath (Join-Path $workingDestination 'RECOVERY.md') -Encoding UTF8

        if (Test-Path -LiteralPath $resolvedDestination -PathType Container) {
            $destinationItems = @(Get-ChildItem -LiteralPath $resolvedDestination -Force)
            if ($destinationItems.Count -gt 0) {
                throw "Signing destination became non-empty while keys were generated: $resolvedDestination"
            }
            Remove-Item -LiteralPath $resolvedDestination -Force
        }
        Move-Item -LiteralPath $workingDestination -Destination $resolvedDestination
        $stagingDestination = $null
        $workingDestination = $resolvedDestination
        $updaterKey = Join-Path $workingDestination 'pixnya-updater.key'
        $manifestKey = Join-Path $workingDestination 'pixnya-android-manifest.key'
        $keystore = Join-Path $workingDestination 'pixnya-release.jks'
        $updaterPublicKey = "$updaterKey.pub"
        $manifestPublicKey = "$manifestKey.pub"
    }

    $recoveryRecord = Join-Path $workingDestination 'RECOVERY.md'
    if (-not (Test-Path -LiteralPath $recoveryRecord -PathType Leaf)) {
        throw "Signing recovery record was not found: $recoveryRecord"
    }
    if (-not (Select-String -LiteralPath $recoveryRecord -SimpleMatch $certificateSha256 -Quiet)) {
        throw 'The recovery record certificate fingerprint does not match the Android keystore.'
    }

    if ($UploadSecrets) {
        $secretValues = [ordered]@{
            PIXIV_OAUTH_CLIENT_ID = $oauth.PIXIV_OAUTH_CLIENT_ID
            PIXIV_OAUTH_CLIENT_SECRET = $oauth.PIXIV_OAUTH_CLIENT_SECRET
            PIXIV_OAUTH_HASH_SALT = $oauth.PIXIV_OAUTH_HASH_SALT
            TAURI_SIGNING_PRIVATE_KEY = [IO.File]::ReadAllText($updaterKey)
            TAURI_SIGNING_PRIVATE_KEY_PASSWORD = $updaterPassword
            PIXNYA_UPDATER_PUBKEY = [IO.File]::ReadAllText($updaterPublicKey).Trim()
            PIXNYA_ANDROID_KEYSTORE_BASE64 = [Convert]::ToBase64String([IO.File]::ReadAllBytes($keystore))
            PIXNYA_ANDROID_KEYSTORE_PASSWORD = $keystorePassword
            PIXNYA_ANDROID_KEY_ALIAS = $androidAlias
            PIXNYA_ANDROID_KEY_PASSWORD = $androidKeyPassword
            PIXNYA_ANDROID_MANIFEST_PRIVATE_KEY_BASE64 = [Convert]::ToBase64String([IO.File]::ReadAllBytes($manifestKey))
            PIXNYA_ANDROID_MANIFEST_PRIVATE_KEY_PASSWORD = $manifestPassword
            PIXNYA_ANDROID_UPDATE_PUBKEY = [IO.File]::ReadAllText($manifestPublicKey).Trim()
        }
        foreach ($entry in $secretValues.GetEnumerator()) {
            Set-GitHubSecretFromMemory $gh $entry.Key $entry.Value
        }
        Assert-GitHubEnvironmentSecretNames $gh @($secretValues.Keys)
        Write-Host "Uploaded $($secretValues.Count) protected GitHub Actions secrets to $Repository/$Environment."
    }

    $completionVerb = if ($UploadExisting) { 'verified' } else { 'created' }
    Write-Host "Production signing material $completionVerb at $resolvedDestination"
    Write-Host 'Back up the whole directory twice before running a signed release.'
} finally {
    Remove-Item Env:PIXNYA_PROVISION_STORE_PASSWORD -ErrorAction SilentlyContinue
    Remove-Item Env:PIXNYA_PROVISION_KEY_PASSWORD -ErrorAction SilentlyContinue
    $updaterPassword = $null
    $manifestPassword = $null
    $keystorePassword = $null
    $androidKeyPassword = $null
    $oauth = $null
    if ($stagingDestination -and (Test-Path -LiteralPath $stagingDestination -PathType Container)) {
        $stagingFullPath = [IO.Path]::GetFullPath($stagingDestination)
        if ([IO.Path]::GetDirectoryName($stagingFullPath) -cne $resolvedParent -or
            -not [IO.Path]::GetFileName($stagingFullPath).StartsWith('.pixnya-signing-')) {
            throw "Refusing to clean an unexpected staging path: $stagingFullPath"
        }
        Remove-Item -LiteralPath $stagingFullPath -Recurse -Force
    }
}
