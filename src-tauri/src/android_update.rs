#![cfg_attr(not(target_os = "android"), allow(dead_code))]

use base64::{engine::general_purpose::STANDARD, Engine as _};
use minisign_verify::{PublicKey, Signature};
use reqwest::{
    blocking::{Client, Response},
    Url,
};
use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};
use zip::ZipArchive;

use crate::update_http::{
    update_redirect_policy, UPDATE_CONNECT_TIMEOUT, UPDATE_REQUEST_TIMEOUT, UPDATE_USER_AGENT,
};

const PACKAGE_NAME: &str = "io.github.space2233.pixnya";
const MANIFEST_SCHEMA_VERSION: u32 = 1;
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
const MAX_SIGNATURE_BYTES: u64 = 16 * 1024;
const MAX_APK_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AndroidUpdateCandidate {
    pub version: String,
    pub version_code: u64,
    pub notes: Option<String>,
    pub published_at: Option<String>,
    pub abi: String,
    pub url: Url,
    pub size: u64,
    pub sha256: String,
    pub certificate_sha256: String,
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedAndroidUpdate {
    pub candidate: AndroidUpdateCandidate,
    pub apk_path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AndroidUpdateError {
    Cancelled,
    InvalidManifest,
    InvalidSignature,
    Network,
    Storage,
    Verification,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AndroidUpdateManifest {
    schema_version: u32,
    version_name: String,
    version_code: u64,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    notes: Option<String>,
    min_sdk: u32,
    artifacts: Vec<AndroidUpdateArtifact>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AndroidUpdateArtifact {
    abi: String,
    url: String,
    size: u64,
    sha256: String,
    package_name: String,
    certificate_sha256: String,
}

pub(crate) fn fetch_candidate(
    manifest_url: &str,
    public_key: &str,
    repository: &str,
    current_version: &str,
    architecture: &str,
    runtime_sdk: u32,
) -> Result<Option<AndroidUpdateCandidate>, AndroidUpdateError> {
    let manifest_url = Url::parse(manifest_url).map_err(|_| AndroidUpdateError::InvalidManifest)?;
    if !crate::updates::is_github_release_url(&manifest_url, repository) {
        return Err(AndroidUpdateError::InvalidManifest);
    }
    let signature_url = Url::parse(&format!("{}.minisig", manifest_url.as_str()))
        .map_err(|_| AndroidUpdateError::InvalidManifest)?;
    let client = update_client(repository)?;
    let manifest_bytes = fetch_bounded(&client, manifest_url, MAX_MANIFEST_BYTES)?;
    let signature_bytes = fetch_bounded(&client, signature_url, MAX_SIGNATURE_BYTES)?;
    let signature_text =
        std::str::from_utf8(&signature_bytes).map_err(|_| AndroidUpdateError::InvalidSignature)?;
    verify_manifest_signature(&manifest_bytes, signature_text, public_key)?;
    let manifest: AndroidUpdateManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|_| AndroidUpdateError::InvalidManifest)?;
    select_candidate(
        manifest,
        repository,
        current_version,
        architecture,
        runtime_sdk,
    )
}

pub(crate) fn download_candidate<F>(
    candidate: &AndroidUpdateCandidate,
    repository: &str,
    update_directory: &Path,
    cancelled: &AtomicBool,
    mut on_progress: F,
) -> Result<PreparedAndroidUpdate, AndroidUpdateError>
where
    F: FnMut(u64, u64),
{
    if candidate.size == 0
        || candidate.size > MAX_APK_BYTES
        || !crate::updates::is_github_release_url(&candidate.url, repository)
    {
        return Err(AndroidUpdateError::InvalidManifest);
    }
    fs::create_dir_all(update_directory).map_err(|_| AndroidUpdateError::Storage)?;
    let safe_version = candidate.version.replace(
        |character: char| {
            !character.is_ascii_alphanumeric() && character != '.' && character != '-'
        },
        "_",
    );
    let safe_abi = candidate.abi.replace(
        |character: char| !character.is_ascii_alphanumeric() && character != '-',
        "_",
    );
    let target = update_directory.join(format!("pixnya-{safe_version}-{safe_abi}.apk"));
    let staging = update_directory.join(format!("pixnya-{safe_version}-{safe_abi}.apk.part"));
    remove_owned_file_if_present(&staging)?;

    let client = update_client(repository)?;
    let mut response = client
        .get(candidate.url.clone())
        .send()
        .map_err(|_| AndroidUpdateError::Network)?;
    ensure_success(&response)?;
    if response
        .content_length()
        .is_some_and(|length| length != candidate.size)
    {
        return Err(AndroidUpdateError::Verification);
    }

    let mut output = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&staging)
        .map_err(|_| AndroidUpdateError::Storage)?;
    let mut hasher = Sha256::new();
    let mut downloaded = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        if cancelled.load(Ordering::Acquire) {
            drop(output);
            let _ = fs::remove_file(&staging);
            return Err(AndroidUpdateError::Cancelled);
        }
        let read = response
            .read(&mut buffer)
            .map_err(|_| AndroidUpdateError::Network)?;
        if read == 0 {
            break;
        }
        downloaded = downloaded
            .checked_add(read as u64)
            .ok_or(AndroidUpdateError::Verification)?;
        if downloaded > candidate.size || downloaded > MAX_APK_BYTES {
            drop(output);
            let _ = fs::remove_file(&staging);
            return Err(AndroidUpdateError::Verification);
        }
        output
            .write_all(&buffer[..read])
            .map_err(|_| AndroidUpdateError::Storage)?;
        hasher.update(&buffer[..read]);
        on_progress(downloaded, candidate.size);
    }
    output.flush().map_err(|_| AndroidUpdateError::Storage)?;
    output.sync_all().map_err(|_| AndroidUpdateError::Storage)?;
    drop(output);

    let actual_hash = lowercase_hex(&hasher.finalize());
    if downloaded != candidate.size || actual_hash != candidate.sha256 {
        let _ = fs::remove_file(&staging);
        return Err(AndroidUpdateError::Verification);
    }
    verify_apk_abi(&staging, &candidate.abi)?;
    remove_owned_file_if_present(&target)?;
    fs::rename(&staging, &target).map_err(|_| AndroidUpdateError::Storage)?;

    Ok(PreparedAndroidUpdate {
        candidate: candidate.clone(),
        apk_path: target,
    })
}

fn select_candidate(
    manifest: AndroidUpdateManifest,
    repository: &str,
    current_version: &str,
    architecture: &str,
    runtime_sdk: u32,
) -> Result<Option<AndroidUpdateCandidate>, AndroidUpdateError> {
    if manifest.schema_version != MANIFEST_SCHEMA_VERSION
        || manifest.min_sdk > runtime_sdk
        || manifest.artifacts.is_empty()
        || manifest
            .notes
            .as_ref()
            .is_some_and(|notes| notes.len() > 64 * 1024)
    {
        return Err(AndroidUpdateError::InvalidManifest);
    }
    let current =
        Version::parse(current_version).map_err(|_| AndroidUpdateError::InvalidManifest)?;
    let offered =
        Version::parse(&manifest.version_name).map_err(|_| AndroidUpdateError::InvalidManifest)?;
    if !offered.pre.is_empty() || !offered.build.is_empty() {
        return Err(AndroidUpdateError::InvalidManifest);
    }
    let current_code = version_code(&current)?;
    if offered <= current {
        return Ok(None);
    }
    if manifest.version_code <= current_code {
        return Err(AndroidUpdateError::InvalidManifest);
    }

    let abi = android_abi(architecture).ok_or(AndroidUpdateError::InvalidManifest)?;
    let artifact = manifest
        .artifacts
        .into_iter()
        .find(|artifact| artifact.abi == abi)
        .ok_or(AndroidUpdateError::InvalidManifest)?;
    let url = Url::parse(&artifact.url).map_err(|_| AndroidUpdateError::InvalidManifest)?;
    let sha256 = normalize_digest(&artifact.sha256)?;
    let certificate_sha256 = normalize_digest(&artifact.certificate_sha256)?;
    if artifact.package_name != PACKAGE_NAME
        || artifact.size == 0
        || artifact.size > MAX_APK_BYTES
        || !crate::updates::is_github_release_url(&url, repository)
    {
        return Err(AndroidUpdateError::InvalidManifest);
    }

    Ok(Some(AndroidUpdateCandidate {
        version: offered.to_string(),
        version_code: manifest.version_code,
        notes: manifest.notes,
        published_at: manifest.published_at,
        abi: abi.to_owned(),
        url,
        size: artifact.size,
        sha256,
        certificate_sha256,
    }))
}

fn verify_manifest_signature(
    manifest: &[u8],
    signature_text: &str,
    public_key_text: &str,
) -> Result<(), AndroidUpdateError> {
    let public_key = decode_public_key(public_key_text)?;
    let signature = decode_signature(signature_text)?;
    public_key
        .verify(manifest, &signature, false)
        .map_err(|_| AndroidUpdateError::InvalidSignature)
}

fn decode_signature(signature_text: &str) -> Result<Signature, AndroidUpdateError> {
    let trimmed = signature_text.trim();
    if let Ok(signature) = Signature::decode(trimmed) {
        return Ok(signature);
    }
    let decoded = STANDARD
        .decode(trimmed)
        .map_err(|_| AndroidUpdateError::InvalidSignature)?;
    let decoded_text =
        std::str::from_utf8(&decoded).map_err(|_| AndroidUpdateError::InvalidSignature)?;
    Signature::decode(decoded_text).map_err(|_| AndroidUpdateError::InvalidSignature)
}

fn decode_public_key(public_key_text: &str) -> Result<PublicKey, AndroidUpdateError> {
    let trimmed = public_key_text.trim();
    if let Ok(public_key) = PublicKey::decode(trimmed) {
        return Ok(public_key);
    }
    if let Ok(public_key) = PublicKey::from_base64(trimmed) {
        return Ok(public_key);
    }
    let decoded = STANDARD
        .decode(trimmed)
        .map_err(|_| AndroidUpdateError::InvalidSignature)?;
    let decoded_text =
        std::str::from_utf8(&decoded).map_err(|_| AndroidUpdateError::InvalidSignature)?;
    PublicKey::decode(decoded_text).map_err(|_| AndroidUpdateError::InvalidSignature)
}

fn update_client(repository: &str) -> Result<Client, AndroidUpdateError> {
    Client::builder()
        .connect_timeout(UPDATE_CONNECT_TIMEOUT)
        .timeout(UPDATE_REQUEST_TIMEOUT)
        .redirect(update_redirect_policy(repository))
        .user_agent(UPDATE_USER_AGENT)
        .build()
        .map_err(|_| AndroidUpdateError::Network)
}

fn fetch_bounded(client: &Client, url: Url, limit: u64) -> Result<Vec<u8>, AndroidUpdateError> {
    let response = client
        .get(url)
        .send()
        .map_err(|_| AndroidUpdateError::Network)?;
    ensure_success(&response)?;
    if response
        .content_length()
        .is_some_and(|length| length > limit)
    {
        return Err(AndroidUpdateError::InvalidManifest);
    }
    let mut bytes = Vec::new();
    response
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| AndroidUpdateError::Network)?;
    if bytes.len() as u64 > limit {
        return Err(AndroidUpdateError::InvalidManifest);
    }
    Ok(bytes)
}

fn ensure_success(response: &Response) -> Result<(), AndroidUpdateError> {
    if response.status().is_success() {
        Ok(())
    } else {
        Err(AndroidUpdateError::Network)
    }
}

fn is_allowed_download_url(url: &Url, repository: &str) -> bool {
    if crate::updates::is_github_release_url(url, repository) {
        return true;
    }
    url.scheme() == "https"
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && matches!(
            url.host_str(),
            Some("release-assets.githubusercontent.com" | "objects.githubusercontent.com")
        )
}

fn android_abi(architecture: &str) -> Option<&'static str> {
    match architecture {
        "aarch64" => Some("arm64-v8a"),
        "arm" | "armv7" => Some("armeabi-v7a"),
        _ => None,
    }
}

fn version_code(version: &Version) -> Result<u64, AndroidUpdateError> {
    version
        .major
        .checked_mul(1_000_000)
        .and_then(|value| {
            version
                .minor
                .checked_mul(1_000)
                .and_then(|minor| value.checked_add(minor))
        })
        .and_then(|value| value.checked_add(version.patch))
        .ok_or(AndroidUpdateError::InvalidManifest)
}

fn normalize_digest(value: &str) -> Result<String, AndroidUpdateError> {
    let normalized = value.replace(':', "").to_ascii_lowercase();
    if normalized.len() == 64 && normalized.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(normalized)
    } else {
        Err(AndroidUpdateError::InvalidManifest)
    }
}

fn lowercase_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn verify_apk_abi(path: &Path, expected_abi: &str) -> Result<(), AndroidUpdateError> {
    let file = File::open(path).map_err(|_| AndroidUpdateError::Storage)?;
    let mut archive = ZipArchive::new(file).map_err(|_| AndroidUpdateError::Verification)?;
    let expected_prefix = format!("lib/{expected_abi}/");
    let mut found_expected = false;
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|_| AndroidUpdateError::Verification)?;
        let name = entry.name();
        if name.starts_with(&expected_prefix) && name.ends_with(".so") {
            found_expected = true;
        }
        if name.starts_with("lib/") && name.ends_with(".so") && !name.starts_with(&expected_prefix)
        {
            return Err(AndroidUpdateError::Verification);
        }
    }
    if found_expected {
        Ok(())
    } else {
        Err(AndroidUpdateError::Verification)
    }
}

fn remove_owned_file_if_present(path: &Path) -> Result<(), AndroidUpdateError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(AndroidUpdateError::Storage),
    }
}

#[cfg(test)]
mod tests {
    use base64::{engine::general_purpose::STANDARD, Engine as _};

    use super::{
        android_abi, decode_public_key, decode_signature, normalize_digest, select_candidate,
        AndroidUpdateArtifact, AndroidUpdateError, AndroidUpdateManifest,
    };
    use crate::updates::is_github_release_url;

    fn manifest() -> AndroidUpdateManifest {
        AndroidUpdateManifest {
            schema_version: 1,
            version_name: "0.26.0".to_owned(),
            version_code: 26_000,
            published_at: Some("2026-08-03T00:00:00Z".to_owned()),
            notes: Some("Update".to_owned()),
            min_sdk: 29,
            artifacts: vec![AndroidUpdateArtifact {
                abi: "arm64-v8a".to_owned(),
                url:
                    "https://github.com/space2233/pixnya/releases/download/v0.26.0/pixnya-arm64.apk"
                        .to_owned(),
                size: 123,
                sha256: "a".repeat(64),
                package_name: "io.github.space2233.pixnya".to_owned(),
                certificate_sha256: "b".repeat(64),
            }],
        }
    }

    #[test]
    fn selects_only_the_runtime_abi_and_a_newer_stable_version() {
        let candidate = select_candidate(manifest(), "space2233/pixnya", "0.25.0", "aarch64", 29)
            .unwrap()
            .unwrap();
        assert_eq!(candidate.abi, "arm64-v8a");
        assert_eq!(candidate.version_code, 26_000);
    }

    #[test]
    fn rejects_a_manifest_that_cannot_upgrade_android_version_code() {
        let mut value = manifest();
        value.version_code = 25_000;
        assert_eq!(
            select_candidate(value, "space2233/pixnya", "0.25.0", "aarch64", 29),
            Err(AndroidUpdateError::InvalidManifest)
        );
    }

    #[test]
    fn release_urls_are_pinned_to_the_configured_repository() {
        let accepted = reqwest::Url::parse(
            "https://github.com/space2233/pixnya-releases/releases/download/v0.26.0/android-latest.json",
        )
        .unwrap();
        let rejected = reqwest::Url::parse(
            "https://github.com/space2233/pixnya/releases/download/v1/pixnya.apk",
        )
        .unwrap();
        assert!(is_github_release_url(
            &accepted,
            "space2233/pixnya-releases"
        ));
        assert!(!is_github_release_url(
            &rejected,
            "space2233/pixnya-releases"
        ));
    }

    #[test]
    fn digest_and_architecture_normalization_is_strict() {
        let colon_separated = "AA:".repeat(31) + "AA";
        assert_eq!(normalize_digest(&colon_separated).unwrap(), "aa".repeat(32));
        assert_eq!(android_abi("arm"), Some("armeabi-v7a"));
        assert_eq!(android_abi("x86_64"), None);
    }

    #[test]
    fn accepts_a_ci_safe_base64_encoded_minisign_public_key_file() {
        let public_key_file = concat!(
            "untrusted comment: minisign public key\n",
            "RWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3\n"
        );
        let encoded = STANDARD.encode(public_key_file);
        assert!(decode_public_key(&encoded).is_ok());
    }

    #[test]
    fn accepts_tauri_base64_and_standard_minisign_signature_files() {
        let signature_file = concat!(
            "untrusted comment: signature from minisign secret key\n",
            "RUQf6LRCGA9i559r3g7V1qNyJDApGip8MfqcadIgT9CuhV3EMhHoN1mGTkUidF/",
            "z7SrlQgXdy8ofjb7bNJJylDOocrCo8KLzZwo=\n",
            "trusted comment: timestamp:1556193335\tfile:test\n",
            "y/rUw2y8/hOUYjZU71eHp/Wo1KZ40fGy2VJEDl34XMJM+TX48Ss/17u3IvIfbVR1FkZZSNCisQbuQY+bHwhEBg=="
        );
        assert!(decode_signature(signature_file).is_ok());
        assert!(decode_signature(&STANDARD.encode(signature_file)).is_ok());
    }
}
