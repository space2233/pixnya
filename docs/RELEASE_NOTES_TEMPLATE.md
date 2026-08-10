# PixNya stable Release notes template

Stable versions (`1.0.0` and later) must keep all five section headings below exactly as written. Candidate `0.x` Draft Releases may use shorter internal notes.

For a stable Draft, replace every bracketed prompt that is already known. Because upgrade tests require the signed Draft artifacts, the three platform result lines, `Failure-path coverage`, and `Known limitations` may temporarily contain the literal text `PENDING after Draft artifacts` (without template placeholder markers). The Draft workflow accepts that explicit pending state but never publishes it. After testing, edit the Draft body with the real devices, `PASS` results, failure-path results, and limitations, then run **Publish verified stable release**. That final workflow rejects every `PENDING`, placeholder, missing attachment, checksum mismatch, signature mismatch, or wrong source commit.

## Unofficial status and platforms

PixNya is an unofficial, community-maintained client and is not affiliated with, endorsed by, or supported by pixiv Inc.

Supported artifacts in this Release:

- Windows x64: NSIS installer
- Linux x64: AppImage
- Android ARM64 (`arm64-v8a`), Android 10 / API 29 or later: APK

## API and OAuth boundary

PixNya opens the official Pixiv sign-in page in an isolated WebView, but browsing and account actions use a non-public App API that may change or stop working without notice. OAuth build parameters are present in distributed binaries and must not be treated as secrets.

## Low-security connections

The compatibility connection mode is disabled by default and never selected as an automatic fallback. Enabling it weakens upstream certificate verification and may allow a network intermediary to observe or modify traffic. {{Describe any connection-mode changes in this Release, or write “No change”.}}

## Source, licenses, SBOM, and checksums

- Source commit: `{{full Git commit SHA}}`
- License: `GPL-3.0-only` (`LICENSE.txt` attachment)
- Source archive: `pixnya-{{version}}-source.tar.gz`
- Third-party licenses: `pixnya-{{version}}-third-party-licenses.tar.gz`
- SPDX SBOMs: `pixnya-{{version}}.spdx.json` and `pixnya-{{version}}-android-runtime.spdx.json`
- Checksums: `SHA256SUMS.txt`

The source tag, explicit source archive, binaries, manifests, SBOMs, license evidence, and checksums are published in this same repository.

## Upgrade verification and limitations

Verified in-place upgrades:

- Windows x64: `{{baseline}} -> {{target}}; {{OS/device}}; PASS`
- Linux x64: `{{baseline}} -> {{target}}; {{distribution}}; PASS`
- Android ARM64: `{{baseline}} -> {{target}}; {{device/OS}}; PASS`

Failure-path coverage: `{{wrong signature, corrupted manifest, interrupted download, low space, cancelled install, and retry results}}`

Known limitations: `{{include every limitation or untested environment explicitly; do not leave blank}}`
