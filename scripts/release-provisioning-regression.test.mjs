import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { readFileSync } from "node:fs";
import { mkdtemp, mkdir, readFile, rm, symlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import { promisify } from "node:util";

const script = readFileSync(new URL("./provision-release-signing.ps1", import.meta.url), "utf8");
const execFileAsync = promisify(execFile);
const root = process.cwd();

test("release provisioning keeps signing material outside Git and passwords only in memory", () => {
  assert.match(script, /\.release-secrets\\pixnya/);
  assert.match(script, /must be stored outside the Git worktree/);
  assert.match(script, /\$projectBoundary/);
  assert.match(script, /FileAttributes\]::ReparsePoint/);
  assert.match(script, /New-Item -ItemType Directory -Path \$resolvedParent -Force/);
  assert.match(script, /\[switch\]\$UploadExisting/);
  assert.match(script, /\.pixnya-signing-/);
  assert.match(script, /Move-Item -LiteralPath \$workingDestination -Destination \$resolvedDestination/);
  assert.match(script, /Assert-AndroidSigningKeyPasswords/);
  assert.match(script, /Assert-GitHubEnvironmentSecretNames/);
  assert.match(script, /Read-Host[\s\S]*-AsSecureString/);
  assert.doesNotMatch(script, /--password', '@env:/);
  assert.doesNotMatch(script, /--password', \$(updater|manifest)Password/);
  assert.match(script, /'tauri', 'signer', 'generate', '--write-keys', \$updaterKey/);
  assert.match(script, /'tauri', 'signer', 'generate', '--write-keys', \$manifestKey/);
  assert.match(script, /function Assert-TauriSigningKeyPassword/);
  assert.match(script, /TAURI_SIGNING_PRIVATE_KEY_PATH/);
  assert.match(script, /TAURI_SIGNING_PRIVATE_KEY_PASSWORD/);
  assert.match(script, /'tauri', 'signer', 'sign', \$probePath/);
  assert.match(script, /RedirectStandardInput = \$true/);
  assert.match(script, /secret set \$Name --repo \$Repository --env \$Environment/);
  assert.match(script, /production-release/);
  assert.doesNotMatch(script, /github-actions-secrets\.json|passwords?\.txt/i);
  assert.doesNotMatch(script, /Set-Content[^\n]*(Password|password)/);
});

test("keytool JVM options stay intact when PowerShell invokes the certificate check", () => {
  const certificateCheck = script.match(
    /\$certificateOutput = & \$KeytoolPath[\s\S]*?2>&1/,
  )?.[0];

  assert.ok(certificateCheck, "certificate inspection call is missing");
  assert.match(certificateCheck, /'-J-Duser\.language=en'/);
  assert.doesNotMatch(certificateCheck, /& \$KeytoolPath -J-Duser\.language=en/);
  assert.match(script, /\$previousKeytoolErrorActionPreference = \$ErrorActionPreference/);
  assert.match(script, /\$ErrorActionPreference = 'Continue'[\s\S]*?\$certificateExitCode = \$LASTEXITCODE/);
  assert.match(script, /\$ErrorActionPreference = \$previousKeytoolErrorActionPreference/);
  assert.match(script, /if \(\$certificateExitCode -ne 0\)/);
});

test("release provisioning rejects a destination that is itself a reparse point", async (t) => {
  if (process.platform !== "win32") {
    t.skip("Windows junction behavior is verified on Windows runners");
    return;
  }

  const temporaryRoot = await mkdtemp(path.join(tmpdir(), "pixnya-signing-link-"));
  const realDestination = path.join(temporaryRoot, "real-destination");
  const linkedDestination = path.join(temporaryRoot, "linked-destination");
  try {
    await mkdir(realDestination);
    await symlink(realDestination, linkedDestination, "junction");
    await assert.rejects(
      execFileAsync("powershell.exe", [
        "-NoProfile",
        "-ExecutionPolicy", "Bypass",
        "-File", path.join(root, "scripts", "provision-release-signing.ps1"),
        "-Destination", linkedDestination,
      ], { cwd: root, windowsHide: true }),
      /cannot be a junction or symbolic link/,
    );
  } finally {
    await rm(temporaryRoot, { recursive: true, force: true });
  }
});

test("provisioning preserves Tauri's single-Base64 public key format", async () => {
  const temporaryRoot = await mkdtemp(path.join(tmpdir(), "pixnya-tauri-pub-"));
  const privateKey = path.join(temporaryRoot, "format-check.key");
  try {
    await execFileAsync(process.execPath, [
      path.join(root, "node_modules", "@tauri-apps", "cli", "tauri.js"),
      "signer", "generate",
      "--password", "throwaway-format-check-password",
      "--write-keys", privateKey,
      "--ci",
    ], { cwd: root, windowsHide: true });
    const publicKeySecret = (await readFile(`${privateKey}.pub`, "utf8")).trim();
    const decodedPublicKeyFile = Buffer.from(publicKeySecret, "base64").toString("utf8");
    assert.match(decodedPublicKeyFile, /^untrusted comment:/);
    assert.match(script, /ReadAllText\(\$updaterPublicKey\)\.Trim\(\)/);
    assert.match(script, /ReadAllText\(\$manifestPublicKey\)\.Trim\(\)/);
    assert.doesNotMatch(script, /ToBase64String\(\[IO\.File\]::ReadAllBytes\(\$(updater|manifest)PublicKey\)\)/);
  } finally {
    await rm(temporaryRoot, { recursive: true, force: true });
  }
});

test("release provisioning covers every protected build and signing secret", () => {
  for (const name of [
    "PIXIV_OAUTH_CLIENT_ID",
    "PIXIV_OAUTH_CLIENT_SECRET",
    "PIXIV_OAUTH_HASH_SALT",
    "TAURI_SIGNING_PRIVATE_KEY",
    "TAURI_SIGNING_PRIVATE_KEY_PASSWORD",
    "PIXNYA_UPDATER_PUBKEY",
    "PIXNYA_ANDROID_KEYSTORE_BASE64",
    "PIXNYA_ANDROID_KEYSTORE_PASSWORD",
    "PIXNYA_ANDROID_KEY_ALIAS",
    "PIXNYA_ANDROID_KEY_PASSWORD",
    "PIXNYA_ANDROID_MANIFEST_PRIVATE_KEY_BASE64",
    "PIXNYA_ANDROID_MANIFEST_PRIVATE_KEY_PASSWORD",
    "PIXNYA_ANDROID_UPDATE_PUBKEY",
  ]) {
    assert.match(script, new RegExp(`\\b${name}\\b`));
  }
  assert.match(script, /copy this complete directory to two encrypted offline media/);
});
