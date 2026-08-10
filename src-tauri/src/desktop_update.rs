use base64::{engine::general_purpose::STANDARD, Engine as _};
use minisign_verify::{PublicKey, Signature};
use reqwest::{blocking::Client, Url};
use std::{
    io::Read,
    sync::atomic::{AtomicBool, Ordering},
};
use tauri::AppHandle;
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::{
    update_http::{
        update_redirect_policy, UPDATE_CONNECT_TIMEOUT, UPDATE_REQUEST_TIMEOUT, UPDATE_USER_AGENT,
    },
    updates::is_github_release_url,
};

const MAX_DESKTOP_UPDATE_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DesktopUpdateSummary {
    pub version: String,
    pub notes: Option<String>,
    pub published_at: Option<String>,
}

pub(crate) struct DesktopUpdateCandidate {
    update: Update,
    repository: String,
    pub summary: DesktopUpdateSummary,
}

pub(crate) struct PreparedDesktopUpdate {
    update: Update,
    bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DesktopUpdateError {
    Cancelled,
    InvalidConfiguration,
    Network,
    Platform,
    Verification,
}

pub(crate) async fn check(
    app: &AppHandle,
    endpoint: &str,
    public_key: &str,
    repository: &str,
) -> Result<Option<DesktopUpdateCandidate>, DesktopUpdateError> {
    let endpoint = Url::parse(endpoint).map_err(|_| DesktopUpdateError::InvalidConfiguration)?;
    let updater = app
        .updater_builder()
        .endpoints(vec![endpoint])
        .map_err(|_| DesktopUpdateError::InvalidConfiguration)?
        .pubkey(public_key.to_owned())
        .configure_client({
            let repository = repository.to_owned();
            move |client| {
                let repository = repository.clone();
                client
                    .connect_timeout(UPDATE_CONNECT_TIMEOUT)
                    .timeout(UPDATE_REQUEST_TIMEOUT)
                    .redirect(update_redirect_policy(&repository))
                    .user_agent(UPDATE_USER_AGENT)
            }
        })
        .build()
        .map_err(|_| DesktopUpdateError::Platform)?;
    let update = updater
        .check()
        .await
        .map_err(|_| DesktopUpdateError::Network)?;
    update
        .map(|update| {
            if !is_github_release_url(&update.download_url, repository) {
                return Err(DesktopUpdateError::InvalidConfiguration);
            }
            let summary = DesktopUpdateSummary {
                version: update.version.clone(),
                notes: update.body.clone(),
                published_at: update.date.map(|date| date.to_string()),
            };
            Ok(DesktopUpdateCandidate {
                update,
                repository: repository.to_owned(),
                summary,
            })
        })
        .transpose()
}

pub(crate) fn download<F>(
    candidate: DesktopUpdateCandidate,
    public_key: &str,
    cancelled: &AtomicBool,
    mut on_progress: F,
) -> Result<PreparedDesktopUpdate, DesktopUpdateError>
where
    F: FnMut(u64, Option<u64>),
{
    let client = update_client(&candidate.repository)?;
    let mut response = client
        .get(candidate.update.download_url.clone())
        .send()
        .map_err(|_| DesktopUpdateError::Network)?;
    if !response.status().is_success() {
        return Err(DesktopUpdateError::Network);
    }
    let content_length = response.content_length();
    if content_length.is_some_and(|length| length > MAX_DESKTOP_UPDATE_BYTES) {
        return Err(DesktopUpdateError::Verification);
    }

    let mut bytes =
        Vec::with_capacity(content_length.unwrap_or_default().min(usize::MAX as u64) as usize);
    let mut downloaded = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        if cancelled.load(Ordering::Acquire) {
            return Err(DesktopUpdateError::Cancelled);
        }
        let read = response
            .read(&mut buffer)
            .map_err(|_| DesktopUpdateError::Network)?;
        if read == 0 {
            break;
        }
        downloaded = downloaded
            .checked_add(read as u64)
            .ok_or(DesktopUpdateError::Verification)?;
        if downloaded > MAX_DESKTOP_UPDATE_BYTES
            || content_length.is_some_and(|length| downloaded > length)
        {
            return Err(DesktopUpdateError::Verification);
        }
        bytes.extend_from_slice(&buffer[..read]);
        on_progress(downloaded, content_length);
    }
    if content_length.is_some_and(|length| length != downloaded) {
        return Err(DesktopUpdateError::Verification);
    }
    verify_tauri_signature(&bytes, &candidate.update.signature, public_key)?;

    Ok(PreparedDesktopUpdate {
        update: candidate.update,
        bytes,
    })
}

pub(crate) fn install(prepared: &PreparedDesktopUpdate) -> Result<(), DesktopUpdateError> {
    prepared
        .update
        .install(&prepared.bytes)
        .map_err(|_| DesktopUpdateError::Platform)
}

fn verify_tauri_signature(
    bytes: &[u8],
    encoded_signature: &str,
    encoded_public_key: &str,
) -> Result<(), DesktopUpdateError> {
    let public_key_text = STANDARD
        .decode(encoded_public_key)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .ok_or(DesktopUpdateError::InvalidConfiguration)?;
    let signature_text = STANDARD
        .decode(encoded_signature)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .ok_or(DesktopUpdateError::Verification)?;
    let public_key = PublicKey::decode(&public_key_text)
        .map_err(|_| DesktopUpdateError::InvalidConfiguration)?;
    let signature =
        Signature::decode(&signature_text).map_err(|_| DesktopUpdateError::Verification)?;
    public_key
        .verify(bytes, &signature, true)
        .map_err(|_| DesktopUpdateError::Verification)
}

fn update_client(repository: &str) -> Result<Client, DesktopUpdateError> {
    Client::builder()
        .connect_timeout(UPDATE_CONNECT_TIMEOUT)
        .timeout(UPDATE_REQUEST_TIMEOUT)
        .redirect(update_redirect_policy(repository))
        .user_agent(UPDATE_USER_AGENT)
        .build()
        .map_err(|_| DesktopUpdateError::Network)
}

#[cfg(test)]
mod tests {
    use crate::updates::is_github_release_url;

    #[test]
    fn desktop_assets_are_pinned_to_the_configured_release_repository() {
        let accepted = reqwest::Url::parse(
            "https://github.com/space2233/pixnya-releases/releases/download/v0.25.0/PixNya.nsis.zip",
        )
        .unwrap();
        let rejected = reqwest::Url::parse(
            "https://github.com/space2233/pixnya/releases/download/v0.25.0/PixNya.nsis.zip",
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
}
