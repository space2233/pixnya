import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";

const root = process.cwd();
const read = (relativePath) => readFile(path.join(root, relativePath), "utf8");

test("desktop configuration initializes the updater plugin with an object", async () => {
  const tauri = JSON.parse(await read("src-tauri/tauri.conf.json"));

  assert.equal(typeof tauri.plugins?.updater, "object");
  assert.notEqual(tauri.plugins.updater, null);
  assert.deepEqual(tauri.plugins.updater.endpoints, []);
  assert.equal(tauri.plugins.updater.pubkey, "");
});

test("PixNya exposes one normalized update interface to the settings page", async () => {
  const [backend, frontend, settings, types] = await Promise.all([
    read("src-tauri/src/updates.rs"),
    read("src/lib/updates.ts"),
    read("src/routes/settings/+page.svelte"),
    read("src/lib/types.ts"),
  ]);

  assert.match(backend, /pub async fn check_for_updates/);
  assert.match(backend, /pub fn set_update_preferences/);
  assert.match(backend, /pub async fn download_update/);
  assert.match(backend, /pub async fn install_update/);
  assert.match(backend, /pub fn cancel_update/);
  assert.match(frontend, /invoke<UpdateSnapshot>\("check_for_updates"/);
  assert.match(frontend, /invoke<UpdateSnapshot>\("download_update"/);
  assert.match(frontend, /invoke<UpdateSnapshot>\("install_update"/);
  assert.match(types, /export interface UpdateSnapshot/);
  assert.match(settings, /<section id="updates"/);
  assert.match(settings, /m\.settings_auto_check\(\)/);
  assert.match(settings, /m\.settings_auto_download\(\)/);
  assert.match(settings, /m\.settings_check_now\(\)/);
});

test("automatic checks are safe defaults, rate limited, and independent from Pixiv transports", async () => {
  const backend = await read("src-tauri/src/updates.rs");

  assert.match(backend, /auto_check: true/);
  assert.match(backend, /auto_download: false/);
  assert.match(backend, /const AUTOMATIC_CHECK_INTERVAL_SECONDS: u64 = 24 \* 60 \* 60/);
  assert.match(backend, /last_attempted_at_unix_seconds/);
  assert.match(backend, /stored\.last_attempted_at_unix_seconds = Some\(attempted_at\)[\s\S]*?persist_state\(app, &stored\)[\s\S]*?platform_check/);
  assert.match(backend, /url\.scheme\(\) == "https"/);
  assert.match(backend, /url\.host_str\(\) == Some\("github\.com"\)/);
  assert.doesNotMatch(backend, /ConnectionMode|NetworkGateway|compatible_direct|Ech/);
});

test("corrupt update state fails closed and full local-data clearing resets it", async () => {
  const [updates, application, settings, types] = await Promise.all([
    read("src-tauri/src/updates.rs"),
    read("src-tauri/src/lib.rs"),
    read("src/routes/settings/+page.svelte"),
    read("src/lib/types.ts"),
  ]);

  assert.doesNotMatch(updates, /load_state\([^)]*\)\.unwrap_or_default\(\)/);
  assert.match(updates, /StoredUpdateState::fail_closed\(\)/);
  assert.match(updates, /pub fn clear_update_state/);
  assert.match(application, /updates::clear_update_state\(&app, update_manager\.inner\(\)\)/);
  assert.match(application, /LocalDataClearFailure::UpdateSettings/);
  assert.match(types, /\| "update_settings"/);
  assert.match(settings, /update_settings: m\.settings_failure_updates\(\)/);
  assert.match(settings, /loadUpdateSnapshot\(\)/);
});

test("desktop updates use Tauri signatures and Android delegates verified APKs to the system", async () => {
  const [cargo, backend, desktopAdapter, androidAdapter, manifest, providerPaths, androidPlugin] = await Promise.all([
    read("src-tauri/Cargo.toml"),
    read("src-tauri/src/updates.rs"),
    read("src-tauri/src/desktop_update.rs"),
    read("src-tauri/src/android_update.rs"),
    read("src-tauri/gen/android/app/src/main/AndroidManifest.xml"),
    read("src-tauri/gen/android/app/src/main/res/xml/file_paths.xml"),
    read("src-tauri/gen/android/app/src/main/java/io/github/space2233/pixnya/UpdateInstallerPlugin.kt"),
  ]);

  assert.match(cargo, /\[target\.'cfg\(not\(target_os = "android"\)\)'\.dependencies\][\s\S]*tauri-plugin-updater = "2"/);
  assert.match(desktopAdapter, /tauri_plugin_updater::\{Update, UpdaterExt\}/);
  assert.match(backend, /PIXNYA_UPDATER_PUBKEY/);
  assert.match(desktopAdapter, /verify_tauri_signature/);
  assert.match(desktopAdapter, /cancelled\.load\(Ordering::Acquire\)/);
  assert.match(desktopAdapter, /space2233\/pixnya\/releases/);
  assert.match(androidAdapter, /verify_manifest_signature/);
  assert.match(androidAdapter, /download_candidate/);
  assert.match(androidAdapter, /verify_apk_abi/);
  assert.match(androidAdapter, /downloaded != candidate\.size \|\| actual_hash != candidate\.sha256/);
  assert.match(manifest, /android\.permission\.REQUEST_INSTALL_PACKAGES/);
  assert.match(providerPaths, /<cache-path name="update_packages" path="updates\/"/);
  assert.doesNotMatch(providerPaths, /external-path/);
  assert.match(androidPlugin, /canRequestPackageInstalls\(\)/);
  assert.match(androidPlugin, /ACTION_MANAGE_UNKNOWN_APP_SOURCES/);
  assert.match(androidPlugin, /FileProvider\.getUriForFile/);
  assert.match(androidPlugin, /Intent\.ACTION_VIEW/);
  assert.match(androidPlugin, /requiresUserConfirmation/);
  assert.match(androidPlugin, /archive\.packageName == activity\.packageName/);
  assert.match(androidPlugin, /archive\.longVersionCode > installed\.longVersionCode/);
  assert.match(androidPlugin, /archiveCertificate == installedCertificate/);
  assert.match(androidPlugin, /sha256\(apk\) == expectedSha256/);
});

test("the selected PixNya identity is consistent across product metadata", async () => {
  const [packageJson, tauri, rustManifest, androidGradle, androidStrings] = await Promise.all([
    read("package.json"),
    read("src-tauri/tauri.conf.json"),
    read("src-tauri/Cargo.toml"),
    read("src-tauri/gen/android/app/build.gradle.kts"),
    read("src-tauri/gen/android/app/src/main/res/values/strings.xml"),
  ]);

  assert.equal(JSON.parse(packageJson).name, "pixnya");
  assert.equal(JSON.parse(tauri).productName, "PixNya");
  assert.equal(JSON.parse(tauri).identifier, "io.github.space2233.pixnya");
  assert.match(rustManifest, /name = "pixnya"/);
  assert.match(androidGradle, /applicationId = "io\.github\.space2233\.pixnya"/);
  assert.match(androidStrings, />"PixNya"</);
});

test("release builds fail closed unless updater and Android signing are configured", async () => {
  const [releaseConfig, androidGradle, androidManifestGenerator, desktopManifestGenerator, workflow, environmentExample] = await Promise.all([
    read("src-tauri/tauri.release.conf.json"),
    read("src-tauri/gen/android/app/build.gradle.kts"),
    read("scripts/generate-android-update-manifest.mjs"),
    read("scripts/generate-desktop-update-manifest.mjs"),
    read(".github/workflows/release.yml"),
    read(".env.example"),
  ]);

  assert.equal(JSON.parse(releaseConfig).bundle.createUpdaterArtifacts, true);
  assert.match(androidGradle, /PIXNYA_ANDROID_KEYSTORE_PATH/);
  assert.match(androidGradle, /PIXNYA_ANDROID_KEYSTORE_PASSWORD/);
  assert.match(androidGradle, /PIXNYA_ANDROID_KEY_ALIAS/);
  assert.match(androidGradle, /PIXNYA_ANDROID_KEY_PASSWORD/);
  assert.match(androidGradle, /Release builds require all four Android signing variables/);
  assert.match(androidManifestGenerator, /schemaVersion:\s*1/);
  assert.match(androidManifestGenerator, /io\.github\.space2233\.pixnya/);
  assert.match(androidManifestGenerator, /createHash\("sha256"\)/);
  assert.match(desktopManifestGenerator, /"windows-x86_64"/);
  assert.match(desktopManifestGenerator, /"linux-x86_64"/);
  assert.match(desktopManifestGenerator, /Base64-encoded Tauri updater signature/);
  assert.match(workflow, /base64 --decode dist\/android-latest\.json\.sig/);
  assert.match(workflow, /workflow_dispatch:/);
  assert.match(workflow, /draft:\s*true/);
  assert.match(workflow, /generate-android-update-manifest\.mjs/);
  assert.match(workflow, /generate-desktop-update-manifest\.mjs/);
  assert.doesNotMatch(workflow, /tauriTarget:\s*armv7/);
  assert.doesNotMatch(workflow, /PIXNYA_GITHUB_TOKEN|PRIVATE_REPOSITORY_TOKEN/);
  assert.match(environmentExample, /Base64 of the complete minisign public-key file/);
});
