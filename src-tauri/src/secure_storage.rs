use pixiv_client_domain::ConnectionMode;
use serde::{Deserialize, Serialize};
#[cfg(target_os = "android")]
use tauri::Manager;
use zeroize::Zeroizing;

#[cfg(not(target_os = "android"))]
const SERVICE: &str = "io.github.space2233.pixnya";
#[cfg(not(target_os = "android"))]
const ACCOUNT: &str = "pixiv-refresh-token";
#[cfg(not(target_os = "android"))]
const INVALIDATION_ACCOUNT: &str = "pixiv-refresh-token-invalid";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SecureStorageError {
    Unavailable,
}

pub(crate) struct StoredCredential {
    refresh_token: Zeroizing<String>,
    connection_mode: ConnectionMode,
}

impl StoredCredential {
    pub(crate) fn into_parts(self) -> (Zeroizing<String>, ConnectionMode) {
        (self.refresh_token, self.connection_mode)
    }
}

#[cfg(target_os = "android")]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SaveRefreshTokenPayload<'token> {
    refresh_token: &'token str,
    connection_mode: ConnectionMode,
}

#[cfg(target_os = "android")]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoadRefreshTokenResult {
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    connection_mode: Option<ConnectionMode>,
}

#[cfg(not(target_os = "android"))]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StoredCredentialRef<'token> {
    refresh_token: &'token str,
    connection_mode: ConnectionMode,
}

#[cfg(not(target_os = "android"))]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredCredentialOwned {
    refresh_token: String,
    connection_mode: ConnectionMode,
}

#[cfg(target_os = "android")]
#[derive(Serialize)]
struct EmptyPayload {}

#[cfg(target_os = "android")]
pub(crate) async fn save_refresh_token(
    app: &tauri::AppHandle,
    token: &str,
    connection_mode: ConnectionMode,
) -> Result<(), SecureStorageError> {
    let plugin = app.state::<crate::AndroidLoginWebViewPlugin>().0.clone();
    plugin
        .run_mobile_plugin_async::<()>(
            "saveRefreshToken",
            SaveRefreshTokenPayload {
                refresh_token: token,
                connection_mode,
            },
        )
        .await
        .map_err(|_| SecureStorageError::Unavailable)
}

#[cfg(target_os = "android")]
pub(crate) async fn load_refresh_token(
    app: &tauri::AppHandle,
) -> Result<Option<StoredCredential>, SecureStorageError> {
    let plugin = app.state::<crate::AndroidLoginWebViewPlugin>().0.clone();
    let result = plugin
        .run_mobile_plugin_async::<LoadRefreshTokenResult>("loadRefreshToken", EmptyPayload {})
        .await
        .map_err(|_| SecureStorageError::Unavailable)?;
    Ok(result.refresh_token.map(|refresh_token| StoredCredential {
        refresh_token: Zeroizing::new(refresh_token),
        connection_mode: result.connection_mode.unwrap_or(ConnectionMode::Standard),
    }))
}

#[cfg(target_os = "android")]
pub(crate) async fn delete_refresh_token(app: &tauri::AppHandle) -> Result<(), SecureStorageError> {
    let plugin = app.state::<crate::AndroidLoginWebViewPlugin>().0.clone();
    plugin
        .run_mobile_plugin_async::<()>("deleteRefreshToken", EmptyPayload {})
        .await
        .map_err(|_| SecureStorageError::Unavailable)
}

#[cfg(not(target_os = "android"))]
pub(crate) async fn save_refresh_token(
    _app: &tauri::AppHandle,
    token: &str,
    connection_mode: ConnectionMode,
) -> Result<(), SecureStorageError> {
    let credential = Zeroizing::new(
        serde_json::to_string(&StoredCredentialRef {
            refresh_token: token,
            connection_mode,
        })
        .map_err(|_| SecureStorageError::Unavailable)?,
    );
    tauri::async_runtime::spawn_blocking(move || {
        mark_desktop_credentials_invalid()?;
        let entry =
            keyring::Entry::new(SERVICE, ACCOUNT).map_err(|_| SecureStorageError::Unavailable)?;
        entry
            .set_password(credential.as_str())
            .map_err(|_| SecureStorageError::Unavailable)?;
        clear_desktop_invalidation_marker()
    })
    .await
    .map_err(|_| SecureStorageError::Unavailable)?
}

#[cfg(not(target_os = "android"))]
pub(crate) async fn load_refresh_token(
    _app: &tauri::AppHandle,
) -> Result<Option<StoredCredential>, SecureStorageError> {
    tauri::async_runtime::spawn_blocking(move || {
        if desktop_credentials_invalidated()? {
            if delete_desktop_credential().is_ok() {
                let _ = clear_desktop_invalidation_marker();
            }
            return Ok(None);
        }
        let entry =
            keyring::Entry::new(SERVICE, ACCOUNT).map_err(|_| SecureStorageError::Unavailable)?;
        match entry.get_password() {
            Ok(value) => {
                let value = Zeroizing::new(value);
                let credential: StoredCredentialOwned = serde_json::from_str(value.as_str())
                    .map_err(|_| SecureStorageError::Unavailable)?;
                Ok(Some(StoredCredential {
                    refresh_token: Zeroizing::new(credential.refresh_token),
                    connection_mode: credential.connection_mode,
                }))
            }
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(SecureStorageError::Unavailable),
        }
    })
    .await
    .map_err(|_| SecureStorageError::Unavailable)?
}

#[cfg(not(target_os = "android"))]
pub(crate) async fn delete_refresh_token(
    _app: &tauri::AppHandle,
) -> Result<(), SecureStorageError> {
    tauri::async_runtime::spawn_blocking(move || {
        mark_desktop_credentials_invalid()?;
        delete_desktop_credential()?;
        clear_desktop_invalidation_marker()
    })
    .await
    .map_err(|_| SecureStorageError::Unavailable)?
}

#[cfg(not(target_os = "android"))]
fn mark_desktop_credentials_invalid() -> Result<(), SecureStorageError> {
    let entry = keyring::Entry::new(SERVICE, INVALIDATION_ACCOUNT)
        .map_err(|_| SecureStorageError::Unavailable)?;
    entry
        .set_password("invalid")
        .map_err(|_| SecureStorageError::Unavailable)
}

#[cfg(not(target_os = "android"))]
fn desktop_credentials_invalidated() -> Result<bool, SecureStorageError> {
    let entry = keyring::Entry::new(SERVICE, INVALIDATION_ACCOUNT)
        .map_err(|_| SecureStorageError::Unavailable)?;
    match entry.get_password() {
        Ok(_) => Ok(true),
        Err(keyring::Error::NoEntry) => Ok(false),
        Err(_) => Err(SecureStorageError::Unavailable),
    }
}

#[cfg(not(target_os = "android"))]
fn clear_desktop_invalidation_marker() -> Result<(), SecureStorageError> {
    let entry = keyring::Entry::new(SERVICE, INVALIDATION_ACCOUNT)
        .map_err(|_| SecureStorageError::Unavailable)?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err(SecureStorageError::Unavailable),
    }
}

#[cfg(not(target_os = "android"))]
fn delete_desktop_credential() -> Result<(), SecureStorageError> {
    let entry =
        keyring::Entry::new(SERVICE, ACCOUNT).map_err(|_| SecureStorageError::Unavailable)?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err(SecureStorageError::Unavailable),
    }
}
