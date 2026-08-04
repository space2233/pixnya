function Import-OAuthEnvironment {
    param(
        [Parameter(Mandatory)]
        [string]$EnvironmentFile,

        [switch]$RequireComplete
    )

    $allowedNames = @(
        'PIXIV_OAUTH_CLIENT_ID',
        'PIXIV_OAUTH_CLIENT_SECRET',
        'PIXIV_OAUTH_HASH_SALT'
    )

    if (Test-Path -LiteralPath $EnvironmentFile -PathType Leaf) {
        foreach ($rawLine in Get-Content -LiteralPath $EnvironmentFile) {
            $line = $rawLine.Trim()
            if (-not $line -or $line.StartsWith('#')) {
                continue
            }

            $separator = $line.IndexOf('=')
            if ($separator -le 0) {
                throw "Invalid OAuth environment line in $EnvironmentFile"
            }

            $name = $line.Substring(0, $separator).Trim()
            if ($name -notin $allowedNames) {
                throw "Unexpected OAuth environment variable: $name"
            }

            $value = $line.Substring($separator + 1).Trim()
            if (
                $value.Length -ge 2 -and
                (($value.StartsWith('"') -and $value.EndsWith('"')) -or
                 ($value.StartsWith("'") -and $value.EndsWith("'")))
            ) {
                $value = $value.Substring(1, $value.Length - 2)
            }
            if ($value) {
                Set-Item -Path "Env:$name" -Value $value
            }
        }
    }

    if ($RequireComplete) {
        $missing = @($allowedNames | Where-Object {
            -not (Get-Item -Path "Env:$_" -ErrorAction SilentlyContinue).Value
        })
        if ($missing.Count -gt 0) {
            throw "OAuth compatibility configuration is incomplete. Copy .env.example to .env.oauth.local and fill: $($missing -join ', ')"
        }
    }
}
