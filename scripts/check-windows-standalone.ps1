param(
  [string]$Executable = (Join-Path $PSScriptRoot '..\target\debug\pixnya.exe'),
    [int]$StartupTimeoutSeconds = 15
)

$resolvedExecutable = (Resolve-Path -LiteralPath $Executable).Path
$bytes = [System.IO.File]::ReadAllBytes($resolvedExecutable)
$peOffset = [BitConverter]::ToInt32($bytes, 0x3c)
$subsystem = [BitConverter]::ToUInt16($bytes, $peOffset + 24 + 68)

if ($subsystem -ne 2) {
    throw "Expected Windows GUI subsystem (2), but found $subsystem in $resolvedExecutable"
}

function Test-LocalTcpPort {
    param([int]$Port)

    $client = [Net.Sockets.TcpClient]::new()
    try {
        $attempt = $client.BeginConnect('127.0.0.1', $Port, $null, $null)
        if (-not $attempt.AsyncWaitHandle.WaitOne(300)) {
            return $false
        }
        $client.EndConnect($attempt)
        return $client.Connected
    }
    catch {
        return $false
    }
    finally {
        $client.Dispose()
    }
}

if (Test-LocalTcpPort -Port 1420) {
    throw 'The Vite development server is running on port 1420; stop it before the standalone check.'
}

$readyTitle = 'PixNya — Unofficial'
$app = $null
$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$targetRoot = (Resolve-Path -LiteralPath (Join-Path $projectRoot 'target')).Path
$runtimeRoot = Join-Path $targetRoot "windows-standalone-runtime-$PID"
$previousTestRoot = $env:PIXIV_CLIENT_TEST_ROOT
$previousWebViewRoot = $env:WEBVIEW2_USER_DATA_FOLDER
New-Item -ItemType Directory -Force -Path $runtimeRoot | Out-Null
$env:PIXIV_CLIENT_TEST_ROOT = Join-Path $runtimeRoot 'application'
$env:WEBVIEW2_USER_DATA_FOLDER = Join-Path $runtimeRoot 'webview'

try {
    $app = Start-Process -FilePath $resolvedExecutable -PassThru -WindowStyle Hidden
    $deadline = [DateTime]::UtcNow.AddSeconds($StartupTimeoutSeconds)
    $ready = $false

    while ([DateTime]::UtcNow -lt $deadline -and -not $ready) {
        Start-Sleep -Milliseconds 250
        $app.Refresh()
        if ($app.HasExited) {
            throw "Standalone client exited before the frontend became ready (exit code $($app.ExitCode))"
        }
        $ready = $app.MainWindowTitle -eq $readyTitle
    }

    if (-not $ready) {
        throw "Bundled frontend did not signal readiness within $StartupTimeoutSeconds seconds"
    }

    Write-Host "PASS: standalone Windows GUI mounted its bundled frontend: $resolvedExecutable"
} finally {
    if ($app -and -not $app.HasExited) {
        Stop-Process -Id $app.Id -Force
    }
    $env:PIXIV_CLIENT_TEST_ROOT = $previousTestRoot
    $env:WEBVIEW2_USER_DATA_FOLDER = $previousWebViewRoot
    $resolvedRuntimeRoot = [IO.Path]::GetFullPath($runtimeRoot)
    if ($resolvedRuntimeRoot.StartsWith($targetRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
        Remove-Item -LiteralPath $resolvedRuntimeRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}
