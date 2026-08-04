#[cfg(any(target_os = "android", test))]
mod android_update;
mod catalog;
#[cfg(not(target_os = "android"))]
mod desktop_update;
mod downloads;
mod exports;
mod history;
mod login_route;
mod paths;
mod secure_storage;
mod session;
mod updates;

use catalog::{
    create_local_collection, delete_local_collection, get_local_catalog_snapshot,
    organize_offline_entry, rename_local_collection, CatalogState,
};
use downloads::{
    enqueue_download, get_download_queue_stats, list_download_tasks, pause_download_task,
    remove_download_task, resume_download_task, DownloadProgress, DownloadWorkerState,
};
use exports::{
    clear_export_destination, export_offline_entry, get_export_destination_status,
    select_export_destination, set_auto_export_downloads, ExportState,
};
use history::{
    clear_browsing_history, get_browsing_history, record_browsing_history,
    remove_browsing_history_entry, set_browsing_history_enabled, HistoryState,
};
use login_route::{evaluate_login_route, LoginRouteError};
use pixiv_client_api::{
    validated_media_url, ApiError, Comment, CommentPage, IllustrationDetail, IllustrationPage,
    IllustrationSeriesPage, NovelContent, NovelDetail, NovelPage, NovelSeriesPage, PixivApiClient,
    TrendingTag, UgoiraMetadata, UserDetail, UserPreviewPage, API_HOST, MEDIA_REFERER,
    MEDIA_USER_AGENT,
};
use pixiv_client_auth::{
    CallbackTarget, ClientRequestSignature, LoginAttempt, LoginError, LoginStatus, OAuthClient,
    OAuthClientConfig, OAuthError, TokenSet,
};
use pixiv_client_diagnostic_log::{
    DiagnosticEntry, DiagnosticLog, DiagnosticLogError, DiagnosticLogSummary, LogComponent,
    LogConnectionMode, LogEvent, LogFailure, LogLevel, RuntimeArchitecture, RuntimePlatform,
};
use pixiv_client_domain::{
    ConnectionMode, PlatformCapabilities, PolicyError, RoutePlan, RouteRequest, TrafficClass,
};
use pixiv_client_library::{
    EntryDraft, LibraryError, LibraryStats, OfflineEntry, OfflineKind, OfflineLibrary,
};
use pixiv_client_local_catalog::CatalogError;
use pixiv_client_local_history::HistoryError;
use pixiv_client_media_cache::{CacheError, CacheKind, CacheScope, CacheStats, MediaCache};
use pixiv_client_network::{
    ConnectionDiagnosticReport, ConnectionPolicy, ConnectionProbe, DiagnosticContext, LoginProxy,
    LoginProxyMode, NetworkGateway, ProbeError, ProbeRequest,
};
use pixiv_client_storage_policy::{
    StorageError, StorageManager, StorageStatus, DEFAULT_CACHE_LIMIT_BYTES,
};
use secure_storage::{delete_refresh_token, load_refresh_token, save_refresh_token};
use serde::{Deserialize, Serialize};
use session::{AuthenticatedContext, SessionSnapshot, SessionState, SessionStateError};
use std::io::{Cursor, Read};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};
use tauri::{Emitter, Manager};
use updates::UpdateManagerState;
use zeroize::Zeroizing;

const LOGIN_HOST: &str = "app-api.pixiv.net";
const OAUTH_HOST: &str = "oauth.secure.pixiv.net";
const LOGIN_URL: &str = "https://app-api.pixiv.net/web/v1/login";
const CALLBACK_SCHEME: &str = "pixiv";
const CALLBACK_HOST: &str = "account";
const CALLBACK_PATH: &str = "/login";
const LOGIN_WINDOW_PREFIX: &str = "pixiv-login-";
const API_TOKEN_MINIMUM_TTL_SECONDS: u64 = 60;
const MAX_THUMBNAIL_BYTES: usize = 8 * 1024 * 1024;
const MAX_ARTWORK_ASSET_BYTES: usize = 96 * 1024 * 1024;
const MAX_UGOIRA_ARCHIVE_BYTES: usize = 256 * 1024 * 1024;
const MEDIA_REQUEST_CONCURRENCY: usize = 6;
#[cfg(not(target_os = "android"))]
const MAIN_WINDOW_READY_TITLE: &str = "PixNya — Unofficial";
static LOGIN_LAUNCH_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Default)]
struct InteractiveLoginState {
    attempt: Mutex<Option<PendingLogin>>,
    proxy: Mutex<Option<ActiveLoginProxy>>,
}

struct PendingLogin {
    attempt: LoginAttempt,
    active_launch_id: Option<u64>,
    mode: ConnectionMode,
    unsafe_acknowledged: bool,
    prepared_oauth_client: Option<PreparedOAuthClient>,
}

type PreparedOAuthClient =
    tauri::async_runtime::JoinHandle<Result<OAuthClient, LoginCompletionError>>;

struct ActiveLoginProxy {
    launch_id: u64,
    _proxy: LoginProxy,
}

#[derive(Clone)]
struct AuthenticatedDataState {
    clients: Arc<Mutex<Vec<CachedTransportClient>>>,
    media_gate: Arc<tokio::sync::Semaphore>,
    mutation_gate: Arc<tokio::sync::Semaphore>,
    library_gate: Arc<tokio::sync::Semaphore>,
    media_fallback_generation: Arc<AtomicU64>,
}

struct CachedTransportClient {
    mode: ConnectionMode,
    host: String,
    client: reqwest::blocking::Client,
}

#[derive(Clone, Default)]
struct MediaCacheState {
    gate: Arc<Mutex<()>>,
}

#[derive(Default)]
struct DiagnosticLogState {
    log: Mutex<Option<DiagnosticLog>>,
}

#[derive(Default)]
struct StoragePolicyState {
    manager: Mutex<Option<Arc<StorageManager>>>,
}

impl Default for AuthenticatedDataState {
    fn default() -> Self {
        Self {
            clients: Arc::new(Mutex::new(Vec::new())),
            media_gate: Arc::new(tokio::sync::Semaphore::new(MEDIA_REQUEST_CONCURRENCY)),
            mutation_gate: Arc::new(tokio::sync::Semaphore::new(1)),
            library_gate: Arc::new(tokio::sync::Semaphore::new(1)),
            media_fallback_generation: Arc::new(AtomicU64::new(0)),
        }
    }
}

impl AuthenticatedDataState {
    fn acknowledge_insecure_media_fallback(&self, generation: u64) {
        self.media_fallback_generation
            .store(generation, Ordering::Release);
    }

    fn media_mode_for(
        &self,
        session_mode: ConnectionMode,
        generation: u64,
    ) -> Result<ConnectionMode, ApiCommandError> {
        match session_mode {
            ConnectionMode::Ech
                if self.media_fallback_generation.load(Ordering::Acquire) != generation =>
            {
                Err(ApiCommandError::UnsafeMediaAcknowledgementRequired)
            }
            ConnectionMode::Ech => Ok(ConnectionMode::Compatible),
            mode => Ok(mode),
        }
    }

    fn client_for(
        &self,
        mode: ConnectionMode,
        traffic: TrafficClass,
        host: &str,
    ) -> Result<reqwest::blocking::Client, ApiCommandError> {
        if let Some(client) = self
            .clients
            .lock()
            .map_err(|_| ApiCommandError::StateUnavailable)?
            .iter()
            .find(|entry| entry.mode == mode && entry.host == host)
            .map(|entry| entry.client.clone())
        {
            return Ok(client);
        }

        let client = NetworkGateway::default()
            .build_client(&ProbeRequest {
                mode,
                traffic,
                host: host.to_owned(),
                unsafe_acknowledged: mode == ConnectionMode::Compatible,
            })
            .map_err(|_| ApiCommandError::TransportUnavailable)?;
        let mut clients = self
            .clients
            .lock()
            .map_err(|_| ApiCommandError::StateUnavailable)?;
        if let Some(existing) = clients
            .iter()
            .find(|entry| entry.mode == mode && entry.host == host)
        {
            return Ok(existing.client.clone());
        }
        clients.push(CachedTransportClient {
            mode,
            host: host.to_owned(),
            client: client.clone(),
        });
        Ok(client)
    }

    fn clear_transport_state(&self) -> Result<(), ApiCommandError> {
        self.clients
            .lock()
            .map_err(|_| ApiCommandError::StateUnavailable)?
            .clear();
        self.media_fallback_generation.store(0, Ordering::Release);
        Ok(())
    }
}

#[derive(Default)]
struct LoginProxyEndpoint {
    url: Option<tauri::Url>,
    port: Option<u16>,
    bridge_cert_sha256: Option<String>,
}

struct LoginSurfaceRequest {
    launch_id: u64,
    mode: ConnectionMode,
    authorization_url: tauri::Url,
    proxy_url: Option<tauri::Url>,
    proxy_port: Option<u16>,
    bridge_cert_sha256: Option<String>,
    ech_preflight: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppStatus {
    phase: &'static str,
    platform: &'static str,
    architecture: &'static str,
    version: &'static str,
    capabilities: PlatformCapabilities,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LoginPreparation {
    route: RoutePlan,
    pkce_method: &'static str,
    callback_target: String,
    oauth_configuration_ready: bool,
    replaced_existing_attempt: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LoginLaunchResult {
    launch_id: u64,
    mode: ConnectionMode,
    route: RoutePlan,
    target: &'static str,
    ech_preflight: &'static str,
    proxy_active: bool,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum LoginPreparationError {
    InvalidHost { host: String },
    EchUnavailable { host: String },
    CompatibleDirectUnavailable { host: String },
    InsecureTransportForbidden { host: String },
    WebViewProxyUnavailable { host: String },
    UnsafeAcknowledgementRequired { host: String },
    InvalidCallbackConfiguration,
    SecureRandomUnavailable,
    StateUnavailable,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum LoginLaunchError {
    InvalidHost {
        host: String,
    },
    EchUnavailable {
        host: String,
    },
    CompatibleDirectUnavailable {
        host: String,
    },
    InsecureTransportForbidden {
        host: String,
    },
    WebViewProxyUnavailable {
        host: String,
    },
    UnsafeAcknowledgementRequired {
        host: String,
    },
    DnsQueryFailed {
        host: String,
    },
    EchConfigUnavailable {
        host: String,
    },
    EchNotAccepted {
        host: String,
    },
    ConnectionFailed {
        host: String,
    },
    HttpProtocolError {
        host: String,
    },
    AttemptUnavailable,
    AttemptNotPending,
    InvalidAuthorizationUrl,
    OAuthConfigurationUnavailable,
    ProxyStartFailed,
    #[cfg(not(target_os = "android"))]
    WindowCreationFailed,
    #[cfg(target_os = "android")]
    MobilePluginUnavailable,
    StateUnavailable,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum LoginCompletionError {
    AttemptUnavailable,
    LaunchMismatch,
    InvalidCallback,
    CallbackStateMismatch,
    AuthorizationDenied,
    AttemptNotPending,
    OAuthConfigurationUnavailable,
    TokenClientUnavailable,
    TokenTransportUnavailable,
    TokenRequestFailed,
    TokenRejected { http_status: u16 },
    InvalidTokenResponse,
    SecureStorageUnavailable,
    SessionUnavailable,
    MobilePluginUnavailable,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LoginCompletionResult {
    status: &'static str,
    session: Option<SessionSnapshot>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LoginCompletionProgress {
    launch_id: u64,
    stage: &'static str,
    elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum SessionCommandError {
    OAuthConfigurationUnavailable,
    TokenClientUnavailable,
    TokenTransportUnavailable,
    TokenRequestFailed,
    TokenRejected { http_status: u16 },
    InvalidTokenResponse,
    SecureStorageUnavailable,
    SessionUnavailable,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ApiCommandError {
    AuthenticationRequired,
    UnsafeMediaAcknowledgementRequired,
    InvalidCursor,
    InvalidIdentifier,
    InvalidInput,
    InvalidMediaUrl,
    TransportUnavailable,
    RequestFailed,
    UpstreamRejected {
        http_status: u16,
    },
    InvalidResponse,
    MediaTooLarge,
    SecureStorageUnavailable,
    OAuthConfigurationUnavailable,
    TokenRefreshFailed,
    OfflineUnavailable,
    OfflineNotFound,
    CacheUnavailable,
    StateUnavailable,
    DiagnosticLogUnavailable,
    ExportUnavailable,
    ExportDestinationUnavailable,
    DownloadQueueUnavailable,
    DownloadTaskNotFound,
    DownloadTransitionInvalid,
    DownloadInterrupted,
    LocalCatalogUnavailable,
    LocalCollectionNotFound,
    LocalCollectionConflict,
    BrowsingHistoryUnavailable,
    StorageUnavailable,
    StorageCapacityExceeded {
        available_bytes: u64,
        required_bytes: u64,
        reserve_bytes: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum LocalDataClearFailure {
    SecureStorage,
    Session,
    LoginState,
    TransportState,
    OfflineLibrary,
    MediaCache,
    LoginWebView,
    DiagnosticLog,
    DownloadQueue,
    StorageSettings,
    ExportSettings,
    UpdateSettings,
    LocalCatalog,
    BrowsingHistory,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LocalDataClearReport {
    complete: bool,
    credentials_cleared: bool,
    session_cleared: bool,
    transport_state_cleared: bool,
    offline_entries_removed: u32,
    offline_bytes_removed: u64,
    cache_entries_removed: u32,
    cache_bytes_removed: u64,
    login_webview_data_cleared: bool,
    diagnostic_log_entries_removed: u32,
    download_tasks_removed: u32,
    storage_settings_reset: bool,
    export_settings_reset: bool,
    update_settings_reset: bool,
    local_collections_removed: u32,
    local_organized_entries_removed: u32,
    local_tags_removed: u32,
    browsing_history_entries_removed: u32,
    failed_steps: Vec<LocalDataClearFailure>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticLogExportResult {
    destination: String,
    entry_count: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LocalDataClearRequest {
    confirmation: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PreparedUgoira {
    entry: OfflineEntry,
    frames: Vec<PreparedUgoiraFrame>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PreparedUgoiraFrame {
    asset_name: String,
    delay_ms: u32,
}

#[cfg(target_os = "android")]
struct AndroidLoginWebViewPlugin(tauri::plugin::PluginHandle<tauri::Wry>);

#[cfg(target_os = "android")]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AndroidLoginPayload {
    launch_id: u64,
    url: String,
    mode: ConnectionMode,
    proxy_port: Option<u16>,
    bridge_cert_sha256: Option<String>,
    ech_preflight: &'static str,
}

#[cfg(target_os = "android")]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AndroidLoginResultPayload {
    launch_id: u64,
}

#[cfg(target_os = "android")]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AndroidLoginResult {
    #[serde(default)]
    callback_url: Option<String>,
}

#[cfg(target_os = "android")]
#[derive(Serialize)]
struct EmptyMobilePayload {}

#[cfg(target_os = "android")]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AndroidDiagnosticLogExportPayload {
    file_name: String,
    contents: String,
}

#[cfg(target_os = "android")]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AndroidDiagnosticLogExportResult {
    destination: String,
}

#[cfg(target_os = "android")]
fn android_login_webview_plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri::plugin::Builder::new("login_webview")
        .setup(|app, api| {
            let handle =
                api.register_android_plugin("io.github.space2233.pixnya", "LoginWebViewPlugin")?;
            app.manage(AndroidLoginWebViewPlugin(handle));
            Ok(())
        })
        .build()
}

fn with_diagnostic_log<T>(
    app: &tauri::AppHandle,
    operation: impl FnOnce(&mut DiagnosticLog) -> Result<T, DiagnosticLogError>,
) -> Result<T, ApiCommandError> {
    let state = app.state::<DiagnosticLogState>();
    let mut log = state
        .log
        .lock()
        .map_err(|_| ApiCommandError::DiagnosticLogUnavailable)?;
    if log.is_none() {
        let root = app
            .path()
            .app_log_dir()
            .map_err(|_| ApiCommandError::DiagnosticLogUnavailable)?
            .join("diagnostics");
        log.replace(
            DiagnosticLog::open(root).map_err(|_| ApiCommandError::DiagnosticLogUnavailable)?,
        );
    }
    operation(
        log.as_mut()
            .ok_or(ApiCommandError::DiagnosticLogUnavailable)?,
    )
    .map_err(|_| ApiCommandError::DiagnosticLogUnavailable)
}

fn record_diagnostic_event(app: &tauri::AppHandle, entry: DiagnosticEntry) {
    let _ = with_diagnostic_log(app, |log| log.record(entry));
}

fn diagnostic_connection_mode(mode: ConnectionMode) -> LogConnectionMode {
    match mode {
        ConnectionMode::Standard => LogConnectionMode::Standard,
        ConnectionMode::Ech => LogConnectionMode::Ech,
        ConnectionMode::Compatible => LogConnectionMode::Compatible,
    }
}

fn session_log_failure(error: &SessionCommandError) -> LogFailure {
    match error {
        SessionCommandError::TokenTransportUnavailable
        | SessionCommandError::TokenRequestFailed => LogFailure::NetworkUnavailable,
        SessionCommandError::TokenRejected { .. } => LogFailure::UpstreamRejected,
        SessionCommandError::SecureStorageUnavailable => LogFailure::SecureStorageUnavailable,
        SessionCommandError::SessionUnavailable => LogFailure::StateUnavailable,
        SessionCommandError::OAuthConfigurationUnavailable
        | SessionCommandError::TokenClientUnavailable
        | SessionCommandError::InvalidTokenResponse => LogFailure::InvalidResponse,
    }
}

fn login_log_failure(error: &LoginCompletionError) -> LogFailure {
    match error {
        LoginCompletionError::TokenTransportUnavailable
        | LoginCompletionError::TokenRequestFailed => LogFailure::NetworkUnavailable,
        LoginCompletionError::TokenRejected { .. } => LogFailure::UpstreamRejected,
        LoginCompletionError::SecureStorageUnavailable => LogFailure::SecureStorageUnavailable,
        LoginCompletionError::SessionUnavailable => LogFailure::StateUnavailable,
        LoginCompletionError::MobilePluginUnavailable => LogFailure::WebviewUnavailable,
        LoginCompletionError::OAuthConfigurationUnavailable
        | LoginCompletionError::TokenClientUnavailable
        | LoginCompletionError::InvalidTokenResponse => LogFailure::InvalidResponse,
        LoginCompletionError::AttemptUnavailable
        | LoginCompletionError::LaunchMismatch
        | LoginCompletionError::InvalidCallback
        | LoginCompletionError::CallbackStateMismatch
        | LoginCompletionError::AuthorizationDenied
        | LoginCompletionError::AttemptNotPending => LogFailure::InvalidInput,
    }
}

fn runtime_platform() -> RuntimePlatform {
    match std::env::consts::OS {
        "windows" => RuntimePlatform::Windows,
        "linux" => RuntimePlatform::Linux,
        "android" => RuntimePlatform::Android,
        _ => RuntimePlatform::Other,
    }
}

fn runtime_architecture() -> RuntimeArchitecture {
    match std::env::consts::ARCH {
        "x86_64" => RuntimeArchitecture::X86_64,
        "aarch64" => RuntimeArchitecture::Aarch64,
        "arm" => RuntimeArchitecture::Armv7,
        _ => RuntimeArchitecture::Other,
    }
}

fn unix_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[tauri::command]
fn get_diagnostic_log_summary(
    app: tauri::AppHandle,
) -> Result<DiagnosticLogSummary, ApiCommandError> {
    with_diagnostic_log(&app, |log| Ok(log.summary()))
}

#[tauri::command]
async fn export_diagnostic_logs(
    app: tauri::AppHandle,
) -> Result<DiagnosticLogExportResult, ApiCommandError> {
    let (contents, entry_count) = with_diagnostic_log(&app, |log| {
        let summary = log.summary();
        Ok((
            log.export_text(runtime_platform(), runtime_architecture())?,
            summary.entry_count,
        ))
    })?;
    let file_name = format!("pixnya-diagnostics-{}.txt", unix_seconds());
    let destination = export_diagnostic_log_file(&app, file_name, contents).await?;
    record_diagnostic_event(
        &app,
        DiagnosticEntry::now(
            LogLevel::Info,
            LogComponent::Privacy,
            LogEvent::DiagnosticLogExported,
        )
        .with_item_count(entry_count),
    );
    Ok(DiagnosticLogExportResult {
        destination,
        entry_count,
    })
}

#[tauri::command]
fn clear_diagnostic_logs(
    confirmed: bool,
    app: tauri::AppHandle,
) -> Result<DiagnosticLogSummary, ApiCommandError> {
    if !confirmed {
        return Err(ApiCommandError::InvalidInput);
    }
    with_diagnostic_log(&app, DiagnosticLog::clear)
}

#[cfg(target_os = "android")]
async fn export_diagnostic_log_file(
    app: &tauri::AppHandle,
    file_name: String,
    contents: String,
) -> Result<String, ApiCommandError> {
    app.state::<AndroidLoginWebViewPlugin>()
        .0
        .clone()
        .run_mobile_plugin_async::<AndroidDiagnosticLogExportResult>(
            "exportDiagnosticLog",
            AndroidDiagnosticLogExportPayload {
                file_name,
                contents,
            },
        )
        .await
        .map(|result| result.destination)
        .map_err(|_| ApiCommandError::ExportUnavailable)
}

#[cfg(not(target_os = "android"))]
async fn export_diagnostic_log_file(
    app: &tauri::AppHandle,
    file_name: String,
    contents: String,
) -> Result<String, ApiCommandError> {
    let directory = app
        .path()
        .download_dir()
        .map_err(|_| ApiCommandError::ExportUnavailable)?
        .join("PixNya");
    tauri::async_runtime::spawn_blocking(move || {
        use std::io::Write;

        std::fs::create_dir_all(&directory).map_err(|_| ApiCommandError::ExportUnavailable)?;
        for sequence in 0..100_u32 {
            let candidate = if sequence == 0 {
                directory.join(&file_name)
            } else {
                directory.join(file_name.replace(".txt", &format!("-{sequence}.txt")))
            };
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&candidate)
            {
                Ok(mut file) => {
                    file.write_all(contents.as_bytes())
                        .map_err(|_| ApiCommandError::ExportUnavailable)?;
                    file.sync_all()
                        .map_err(|_| ApiCommandError::ExportUnavailable)?;
                    return Ok(candidate.to_string_lossy().into_owned());
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(_) => return Err(ApiCommandError::ExportUnavailable),
            }
        }
        Err(ApiCommandError::ExportUnavailable)
    })
    .await
    .map_err(|_| ApiCommandError::ExportUnavailable)?
}

#[tauri::command]
fn get_app_status() -> AppStatus {
    AppStatus {
        phase: "phase_2_authenticated_data",
        platform: std::env::consts::OS,
        architecture: std::env::consts::ARCH,
        version: env!("CARGO_PKG_VERSION"),
        capabilities: detected_capabilities(),
    }
}

#[tauri::command]
fn mark_frontend_ready(app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(not(target_os = "android"))]
    {
        let window = app
            .get_webview_window("main")
            .ok_or_else(|| "main window unavailable".to_owned())?;
        window
            .set_title(MAIN_WINDOW_READY_TITLE)
            .map_err(|_| "main window title unavailable".to_owned())?;
    }
    #[cfg(target_os = "android")]
    let _ = app;
    Ok(())
}

#[tauri::command]
fn evaluate_connection(
    mode: ConnectionMode,
    traffic: TrafficClass,
    host: String,
) -> Result<RoutePlan, PolicyError> {
    ConnectionPolicy.evaluate(&RouteRequest {
        mode,
        traffic,
        host,
        capabilities: detected_capabilities(),
    })
}

#[tauri::command]
async fn probe_connection(
    mode: ConnectionMode,
    traffic: TrafficClass,
    host: String,
    unsafe_acknowledged: bool,
) -> Result<ConnectionProbe, ProbeError> {
    let failure_host = host.clone();
    let request = ProbeRequest {
        mode,
        traffic,
        host,
        unsafe_acknowledged,
    };

    tauri::async_runtime::spawn_blocking(move || NetworkGateway::default().probe(&request))
        .await
        .map_err(|_| ProbeError::ConnectionFailed { host: failure_host })?
}

#[tauri::command]
async fn run_connection_diagnostics(
    mode: ConnectionMode,
    unsafe_acknowledged: bool,
    app: tauri::AppHandle,
    state: tauri::State<'_, InteractiveLoginState>,
) -> Result<ConnectionDiagnosticReport, ProbeError> {
    let started = std::time::Instant::now();
    let webview_proxy_active = state
        .proxy
        .lock()
        .map_err(|_| ProbeError::ConnectionFailed {
            host: LOGIN_HOST.to_owned(),
        })?
        .is_some();

    let result = tauri::async_runtime::spawn_blocking(move || {
        NetworkGateway::default().diagnose(
            mode,
            unsafe_acknowledged,
            DiagnosticContext {
                application_version: env!("CARGO_PKG_VERSION"),
                platform: std::env::consts::OS,
                architecture: std::env::consts::ARCH,
                webview_proxy_active,
            },
        )
    })
    .await
    .map_err(|_| ProbeError::ConnectionFailed {
        host: LOGIN_HOST.to_owned(),
    });
    let mut entry = DiagnosticEntry::now(
        if result.is_ok() {
            LogLevel::Info
        } else {
            LogLevel::Warning
        },
        LogComponent::Network,
        if result.is_ok() {
            LogEvent::ConnectionDiagnosticsCompleted
        } else {
            LogEvent::ConnectionDiagnosticsFailed
        },
    )
    .with_connection_mode(diagnostic_connection_mode(mode))
    .with_latency_ms(started.elapsed().as_millis().try_into().unwrap_or(u64::MAX));
    if result.is_err() {
        entry = entry.with_failure(LogFailure::NetworkUnavailable);
    }
    record_diagnostic_event(&app, entry);
    result
}

#[tauri::command]
fn prepare_interactive_login(
    mode: ConnectionMode,
    unsafe_acknowledged: bool,
    state: tauri::State<'_, InteractiveLoginState>,
) -> Result<LoginPreparation, LoginPreparationError> {
    let (preparation, attempt) =
        create_login_attempt(mode, detected_capabilities(), unsafe_acknowledged)?;

    let mut pending = state
        .attempt
        .lock()
        .map_err(|_| LoginPreparationError::StateUnavailable)?;
    let replaced_existing_attempt = pending
        .replace(PendingLogin {
            attempt,
            active_launch_id: None,
            mode,
            unsafe_acknowledged,
            prepared_oauth_client: None,
        })
        .is_some();
    state
        .proxy
        .lock()
        .map_err(|_| LoginPreparationError::StateUnavailable)?
        .take();

    Ok(LoginPreparation {
        replaced_existing_attempt,
        ..preparation
    })
}

fn create_login_attempt(
    mode: ConnectionMode,
    capabilities: PlatformCapabilities,
    unsafe_acknowledged: bool,
) -> Result<(LoginPreparation, LoginAttempt), LoginPreparationError> {
    let route = evaluate_login_route(mode, capabilities, unsafe_acknowledged, LOGIN_HOST)
        .map_err(LoginPreparationError::from)?;

    let target = CallbackTarget::new(CALLBACK_SCHEME, CALLBACK_HOST, CALLBACK_PATH)
        .map_err(LoginPreparationError::from)?;
    let callback_target = target.display_value();
    let attempt = LoginAttempt::begin(target).map_err(LoginPreparationError::from)?;
    let pkce_method = attempt
        .authorization_parameters()
        .map_err(LoginPreparationError::from)?
        .code_challenge_method();

    Ok((
        LoginPreparation {
            route,
            pkce_method,
            callback_target,
            oauth_configuration_ready: oauth_configuration().is_ok(),
            replaced_existing_attempt: false,
        },
        attempt,
    ))
}

#[tauri::command]
async fn open_interactive_login(
    mode: ConnectionMode,
    unsafe_acknowledged: bool,
    app: tauri::AppHandle,
    state: tauri::State<'_, InteractiveLoginState>,
) -> Result<LoginLaunchResult, LoginLaunchError> {
    if oauth_configuration().is_err() {
        return Err(LoginLaunchError::OAuthConfigurationUnavailable);
    }
    let route = evaluate_login_route(
        mode,
        detected_capabilities(),
        unsafe_acknowledged,
        LOGIN_HOST,
    )
    .map_err(LoginLaunchError::from)?;
    let authorization_url = {
        let pending = state
            .attempt
            .lock()
            .map_err(|_| LoginLaunchError::StateUnavailable)?;
        let pending = pending
            .as_ref()
            .ok_or(LoginLaunchError::AttemptUnavailable)?;
        build_authorization_url(&pending.attempt)?
    };

    let ech_preflight = if mode == ConnectionMode::Ech {
        let request = ProbeRequest {
            mode,
            traffic: TrafficClass::Api,
            host: LOGIN_HOST.to_owned(),
            unsafe_acknowledged: false,
        };
        tauri::async_runtime::spawn_blocking(move || NetworkGateway::default().probe(&request))
            .await
            .map_err(|_| LoginLaunchError::ConnectionFailed {
                host: LOGIN_HOST.into(),
            })?
            .map_err(LoginLaunchError::from)?;
        "accepted_by_rust_preflight"
    } else {
        "not_applicable"
    };

    let launch_id = LOGIN_LAUNCH_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let prepared_oauth_client = begin_oauth_client_preparation(mode, unsafe_acknowledged);
    {
        let mut pending = state
            .attempt
            .lock()
            .map_err(|_| LoginLaunchError::StateUnavailable)?;
        let pending = pending
            .as_mut()
            .ok_or(LoginLaunchError::AttemptUnavailable)?;
        pending.active_launch_id = Some(launch_id);
        pending.prepared_oauth_client = Some(prepared_oauth_client);
    }
    let proxy = configure_login_proxy(&state, mode, launch_id)?;
    let proxy_active = proxy.port.is_some();
    let target = match open_login_surface(
        &app,
        LoginSurfaceRequest {
            launch_id,
            mode,
            authorization_url,
            proxy_url: proxy.url,
            proxy_port: proxy.port,
            bridge_cert_sha256: proxy.bridge_cert_sha256,
            ech_preflight,
        },
    )
    .await
    {
        Ok(target) => target,
        Err(error) => {
            cleanup_login_proxy(&state, launch_id)?;
            if let Ok(mut pending) = state.attempt.lock() {
                if pending
                    .as_ref()
                    .is_some_and(|pending| pending.active_launch_id == Some(launch_id))
                {
                    if let Some(pending) = pending.as_mut() {
                        pending.active_launch_id = None;
                    }
                }
            }
            return Err(error);
        }
    };

    record_diagnostic_event(
        &app,
        DiagnosticEntry::now(LogLevel::Info, LogComponent::Login, LogEvent::LoginOpened)
            .with_connection_mode(diagnostic_connection_mode(mode)),
    );
    Ok(LoginLaunchResult {
        launch_id,
        mode,
        route,
        target,
        ech_preflight,
        proxy_active,
    })
}

fn build_authorization_url(attempt: &LoginAttempt) -> Result<tauri::Url, LoginLaunchError> {
    let parameters = attempt
        .authorization_parameters()
        .map_err(LoginLaunchError::from)?;
    let mut url =
        tauri::Url::parse(LOGIN_URL).map_err(|_| LoginLaunchError::InvalidAuthorizationUrl)?;
    url.query_pairs_mut()
        .append_pair("code_challenge", parameters.code_challenge())
        .append_pair("code_challenge_method", parameters.code_challenge_method())
        .append_pair("client", "pixiv-android");
    Ok(url)
}

fn configure_login_proxy(
    state: &InteractiveLoginState,
    mode: ConnectionMode,
    launch_id: u64,
) -> Result<LoginProxyEndpoint, LoginLaunchError> {
    let mut active = state
        .proxy
        .lock()
        .map_err(|_| LoginLaunchError::StateUnavailable)?;
    active.take();

    let Some(proxy_mode) = login_proxy_mode(mode) else {
        return Ok(LoginProxyEndpoint::default());
    };

    let proxy = LoginProxy::start(proxy_mode).map_err(|_| LoginLaunchError::ProxyStartFailed)?;
    let port = proxy.port();
    let url = tauri::Url::parse(&proxy.url()).map_err(|_| LoginLaunchError::ProxyStartFailed)?;
    let bridge_cert_sha256 = proxy.certificate_sha256().map(str::to_owned);
    active.replace(ActiveLoginProxy {
        launch_id,
        _proxy: proxy,
    });
    Ok(LoginProxyEndpoint {
        url: Some(url),
        port: Some(port),
        bridge_cert_sha256,
    })
}

#[cfg(target_os = "android")]
fn login_proxy_mode(mode: ConnectionMode) -> Option<LoginProxyMode> {
    match mode {
        ConnectionMode::Standard => None,
        ConnectionMode::Ech | ConnectionMode::Compatible => Some(LoginProxyMode::InsecureTlsBridge),
    }
}

#[cfg(not(target_os = "android"))]
fn login_proxy_mode(mode: ConnectionMode) -> Option<LoginProxyMode> {
    match mode {
        ConnectionMode::Compatible => Some(LoginProxyMode::EndToEndFixedIp),
        ConnectionMode::Standard | ConnectionMode::Ech => None,
    }
}

fn cleanup_login_proxy(
    state: &InteractiveLoginState,
    launch_id: u64,
) -> Result<bool, LoginLaunchError> {
    let mut active = state
        .proxy
        .lock()
        .map_err(|_| LoginLaunchError::StateUnavailable)?;
    if active
        .as_ref()
        .is_some_and(|proxy| proxy.launch_id == launch_id)
    {
        active.take();
        Ok(true)
    } else {
        Ok(false)
    }
}

#[cfg(not(target_os = "android"))]
async fn open_login_surface(
    app: &tauri::AppHandle,
    request: LoginSurfaceRequest,
) -> Result<&'static str, LoginLaunchError> {
    let LoginSurfaceRequest {
        launch_id,
        mode,
        authorization_url,
        proxy_url,
        proxy_port: _proxy_port,
        bridge_cert_sha256: _bridge_cert_sha256,
        ech_preflight: _ech_preflight,
    } = request;
    for (label, window) in app.webview_windows() {
        if label.starts_with(LOGIN_WINDOW_PREFIX) {
            let _ = window.destroy();
        }
    }

    let label = format!("{LOGIN_WINDOW_PREFIX}{launch_id}");
    let data_directory = paths::app_cache_dir(app)
        .map_err(|_| LoginLaunchError::WindowCreationFailed)?
        .join("login-webview")
        .join(mode_code(mode));
    let navigation_app = app.clone();
    let navigation_label = label.clone();
    let mut builder = tauri::WebviewWindowBuilder::new(
        app,
        label.clone(),
        tauri::WebviewUrl::External(authorization_url),
    )
    .title("Pixiv 官方登录")
    .inner_size(520.0, 760.0)
    .min_inner_size(420.0, 620.0)
    .center()
    .incognito(true)
    .data_directory(data_directory)
    .on_navigation(move |url| {
        if is_expected_callback_url(url) {
            let callback_url = url.to_string();
            let callback_app = navigation_app.clone();
            let callback_label = navigation_label.clone();
            if let Some(window) = callback_app.get_webview_window(&callback_label) {
                let _ = window.destroy();
            }
            tauri::async_runtime::spawn(async move {
                let result = complete_captured_login(&callback_app, launch_id, callback_url).await;
                emit_login_result(&callback_app, result);
            });
            false
        } else {
            allow_login_navigation(url)
        }
    })
    .on_new_window(|url, _| {
        if allow_login_navigation(&url) {
            tauri::webview::NewWindowResponse::Allow
        } else {
            tauri::webview::NewWindowResponse::Deny
        }
    });

    if let Some(proxy_url) = proxy_url {
        builder = builder.proxy_url(proxy_url);
    }

    #[cfg(target_os = "windows")]
    if mode == ConnectionMode::Ech {
        builder = builder.additional_browser_args(
            "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection \
             --enable-features=EncryptedClientHello \
             --dns-over-https-mode=secure \
             --dns-over-https-templates=https://dns.alidns.com/dns-query{?dns}",
        );
    }

    let window = builder
        .build()
        .map_err(|_| LoginLaunchError::WindowCreationFailed)?;
    let cleanup_app = app.clone();
    window.on_window_event(move |event| {
        if matches!(event, tauri::WindowEvent::Destroyed) {
            let state = cleanup_app.state::<InteractiveLoginState>();
            let _ = cleanup_login_proxy(&state, launch_id);
        }
    });

    Ok("desktop_webview_window")
}

#[cfg(target_os = "android")]
async fn open_login_surface(
    app: &tauri::AppHandle,
    request: LoginSurfaceRequest,
) -> Result<&'static str, LoginLaunchError> {
    let LoginSurfaceRequest {
        launch_id,
        mode,
        authorization_url,
        proxy_url: _proxy_url,
        proxy_port,
        bridge_cert_sha256,
        ech_preflight,
    } = request;
    let plugin = app.state::<AndroidLoginWebViewPlugin>().0.clone();
    plugin
        .run_mobile_plugin_async::<()>(
            "openLogin",
            AndroidLoginPayload {
                launch_id,
                url: authorization_url.to_string(),
                mode,
                proxy_port,
                bridge_cert_sha256,
                ech_preflight,
            },
        )
        .await
        .map_err(|_| LoginLaunchError::MobilePluginUnavailable)?;
    Ok("android_login_activity")
}

#[cfg(not(target_os = "android"))]
fn allow_login_navigation(url: &tauri::Url) -> bool {
    matches!(url.scheme(), "https" | "about")
}

#[cfg(not(target_os = "android"))]
fn is_expected_callback_url(url: &tauri::Url) -> bool {
    url.scheme() == CALLBACK_SCHEME
        && url.host_str() == Some(CALLBACK_HOST)
        && url.path() == CALLBACK_PATH
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none()
}

fn oauth_configuration() -> Result<OAuthClientConfig, OAuthError> {
    OAuthClientConfig::new(
        option_env!("PIXIV_OAUTH_CLIENT_ID").unwrap_or_default(),
        option_env!("PIXIV_OAUTH_CLIENT_SECRET").unwrap_or_default(),
        option_env!("PIXIV_OAUTH_HASH_SALT").unwrap_or_default(),
    )
}

fn build_oauth_client(
    mode: ConnectionMode,
    unsafe_acknowledged: bool,
) -> Result<OAuthClient, LoginCompletionError> {
    let config = oauth_configuration().map_err(LoginCompletionError::from)?;
    let http = NetworkGateway::default()
        .build_client(&ProbeRequest {
            mode,
            traffic: TrafficClass::OAuth,
            host: OAUTH_HOST.into(),
            unsafe_acknowledged,
        })
        .map_err(|_| LoginCompletionError::TokenTransportUnavailable)?;
    let client = OAuthClient::with_http(config, http);
    client.warm_transport();
    Ok(client)
}

fn begin_oauth_client_preparation(
    mode: ConnectionMode,
    unsafe_acknowledged: bool,
) -> PreparedOAuthClient {
    tauri::async_runtime::spawn_blocking(move || build_oauth_client(mode, unsafe_acknowledged))
}

async fn finish_oauth_client_preparation(
    prepared: Option<PreparedOAuthClient>,
    mode: ConnectionMode,
    unsafe_acknowledged: bool,
) -> Result<OAuthClient, LoginCompletionError> {
    if let Some(prepared) = prepared {
        if let Ok(Ok(client)) = prepared.await {
            return Ok(client);
        }
    }

    begin_oauth_client_preparation(mode, unsafe_acknowledged)
        .await
        .map_err(|_| LoginCompletionError::TokenClientUnavailable)?
}

fn emit_login_progress(
    app: &tauri::AppHandle,
    launch_id: u64,
    stage: &'static str,
    started: std::time::Instant,
) {
    let _ = app.emit(
        "pixiv-login-progress",
        LoginCompletionProgress {
            launch_id,
            stage,
            elapsed_ms: started.elapsed().as_millis().try_into().unwrap_or(u64::MAX),
        },
    );
}

async fn complete_captured_login(
    app: &tauri::AppHandle,
    launch_id: u64,
    callback_url: String,
) -> Result<SessionSnapshot, LoginCompletionError> {
    let started = std::time::Instant::now();
    let callback_url = Zeroizing::new(callback_url);
    let login_state = app.state::<InteractiveLoginState>();
    let (grant, mode, unsafe_acknowledged, prepared_oauth_client) = {
        let mut pending_guard = login_state
            .attempt
            .lock()
            .map_err(|_| LoginCompletionError::SessionUnavailable)?;
        let pending = pending_guard
            .as_mut()
            .ok_or(LoginCompletionError::AttemptUnavailable)?;
        if pending.active_launch_id != Some(launch_id) {
            return Err(LoginCompletionError::LaunchMismatch);
        }

        let mode = pending.mode;
        let unsafe_acknowledged = pending.unsafe_acknowledged;
        let result = pending
            .attempt
            .accept_private_surface_callback(callback_url.as_str());
        let prepared_oauth_client = if result.is_ok() {
            pending.prepared_oauth_client.take()
        } else {
            None
        };
        if result.is_ok() || pending.attempt.status() != LoginStatus::Pending {
            pending_guard.take();
        }
        (
            result.map_err(LoginCompletionError::from)?,
            mode,
            unsafe_acknowledged,
            prepared_oauth_client,
        )
    };
    emit_login_progress(app, launch_id, "callback_verified", started);
    cleanup_login_proxy(&login_state, launch_id)
        .map_err(|_| LoginCompletionError::SessionUnavailable)?;

    let oauth_client =
        finish_oauth_client_preparation(prepared_oauth_client, mode, unsafe_acknowledged).await?;
    emit_login_progress(app, launch_id, "transport_ready", started);
    let tokens = tauri::async_runtime::spawn_blocking(move || {
        oauth_client
            .exchange_authorization_code(&grant)
            .map_err(LoginCompletionError::from)
    })
    .await
    .map_err(|_| LoginCompletionError::TokenRequestFailed)??;
    emit_login_progress(app, launch_id, "token_received", started);

    let session_state = app.state::<SessionState>();
    let _operation = session_state.operation_guard().await;
    let snapshot = install_login_tokens(app, &session_state, tokens, mode).await?;
    emit_login_progress(app, launch_id, "session_saved", started);
    Ok(snapshot)
}

async fn install_login_tokens(
    app: &tauri::AppHandle,
    session_state: &SessionState,
    tokens: TokenSet,
    mode: ConnectionMode,
) -> Result<SessionSnapshot, LoginCompletionError> {
    let (access_token, refresh_token, expires_in, user) = tokens.into_parts();
    save_refresh_token(app, refresh_token.as_str(), mode)
        .await
        .map_err(|_| LoginCompletionError::SecureStorageUnavailable)?;
    session_state
        .install(access_token, user, expires_in, mode)
        .map_err(LoginCompletionError::from)
}

fn emit_login_result(
    app: &tauri::AppHandle,
    result: Result<SessionSnapshot, LoginCompletionError>,
) {
    record_diagnostic_event(
        app,
        match &result {
            Ok(_) => DiagnosticEntry::now(
                LogLevel::Info,
                LogComponent::Login,
                LogEvent::LoginCompleted,
            ),
            Err(error) => DiagnosticEntry::now(
                LogLevel::Warning,
                LogComponent::Login,
                LogEvent::LoginFailed,
            )
            .with_failure(login_log_failure(error)),
        },
    );
    match result {
        Ok(snapshot) => {
            downloads::wake_download_worker(app);
            let _ = app.emit("pixiv-session-changed", snapshot.clone());
            let _ = app.emit("pixiv-login-completed", snapshot);
        }
        Err(error) => {
            let _ = app.emit("pixiv-login-failed", error);
        }
    }
}

#[tauri::command]
async fn complete_mobile_interactive_login(
    launch_id: u64,
    app: tauri::AppHandle,
) -> Result<LoginCompletionResult, LoginCompletionError> {
    #[cfg(target_os = "android")]
    {
        let plugin = app.state::<AndroidLoginWebViewPlugin>().0.clone();
        let result = plugin
            .run_mobile_plugin_async::<AndroidLoginResult>(
                "takeLoginResult",
                AndroidLoginResultPayload { launch_id },
            )
            .await
            .map_err(|_| LoginCompletionError::MobilePluginUnavailable)?;
        let login_state = app.state::<InteractiveLoginState>();
        cleanup_login_proxy(&login_state, launch_id)
            .map_err(|_| LoginCompletionError::SessionUnavailable)?;

        let Some(callback_url) = result.callback_url else {
            return Ok(LoginCompletionResult {
                status: "pending",
                session: None,
            });
        };
        let completion = complete_captured_login(&app, launch_id, callback_url).await;
        match completion {
            Ok(snapshot) => {
                emit_login_result(&app, Ok(snapshot.clone()));
                Ok(LoginCompletionResult {
                    status: "completed",
                    session: Some(snapshot),
                })
            }
            Err(error) => {
                emit_login_result(&app, Err(error.clone()));
                Err(error)
            }
        }
    }

    #[cfg(not(target_os = "android"))]
    {
        let _ = (launch_id, app);
        Err(LoginCompletionError::MobilePluginUnavailable)
    }
}

#[tauri::command]
fn get_session_status(
    state: tauri::State<'_, SessionState>,
) -> Result<SessionSnapshot, SessionCommandError> {
    state.snapshot().map_err(SessionCommandError::from)
}

#[tauri::command]
async fn restore_session(
    app: tauri::AppHandle,
    state: tauri::State<'_, SessionState>,
) -> Result<SessionSnapshot, SessionCommandError> {
    let _operation = state.operation_guard().await;
    if paths::isolated_test_root().is_some() {
        return state.clear().map_err(SessionCommandError::from);
    }
    let result = async {
        if state
            .authenticated_context(API_TOKEN_MINIMUM_TTL_SECONDS)
            .map_err(SessionCommandError::from)?
            .is_some()
        {
            return state.snapshot().map_err(SessionCommandError::from);
        }

        refresh_session_locked(&app, &state).await
    }
    .await;
    record_diagnostic_event(
        &app,
        match &result {
            Ok(_) => DiagnosticEntry::now(
                LogLevel::Info,
                LogComponent::Session,
                LogEvent::SessionRestoreCompleted,
            ),
            Err(error) => DiagnosticEntry::now(
                LogLevel::Warning,
                LogComponent::Session,
                LogEvent::SessionRestoreFailed,
            )
            .with_failure(session_log_failure(error)),
        },
    );
    if result.as_ref().is_ok_and(|snapshot| snapshot.logged_in) {
        downloads::wake_download_worker(&app);
    }
    result
}

async fn refresh_session_locked(
    app: &tauri::AppHandle,
    state: &SessionState,
) -> Result<SessionSnapshot, SessionCommandError> {
    let before = state.snapshot().map_err(SessionCommandError::from)?;

    let Some(credential) = load_refresh_token(app)
        .await
        .map_err(|_| SessionCommandError::SecureStorageUnavailable)?
    else {
        let snapshot = state.clear().map_err(SessionCommandError::from)?;
        emit_session_change_if_needed(app, &before, &snapshot);
        return Ok(snapshot);
    };
    let (refresh_token, connection_mode) = credential.into_parts();
    let outcome = tauri::async_runtime::spawn_blocking(move || {
        let config = oauth_configuration().map_err(SessionCommandError::from)?;
        let http = NetworkGateway::default()
            .build_client(&ProbeRequest {
                mode: connection_mode,
                traffic: TrafficClass::OAuth,
                host: OAUTH_HOST.into(),
                unsafe_acknowledged: connection_mode == ConnectionMode::Compatible,
            })
            .map_err(|_| SessionCommandError::TokenTransportUnavailable)?;
        OAuthClient::with_http(config, http)
            .refresh(refresh_token.as_str())
            .map_err(SessionCommandError::from)
    })
    .await
    .map_err(|_| SessionCommandError::TokenRequestFailed)?;

    let tokens = match outcome {
        Ok(tokens) => tokens,
        Err(SessionCommandError::TokenRejected {
            http_status: 400 | 401,
        }) => {
            delete_refresh_token(app)
                .await
                .map_err(|_| SessionCommandError::SecureStorageUnavailable)?;
            let snapshot = state.clear().map_err(SessionCommandError::from)?;
            emit_session_change_if_needed(app, &before, &snapshot);
            return Ok(snapshot);
        }
        Err(error) => return Err(error),
    };
    let (access_token, rotated_refresh_token, expires_in, user) = tokens.into_parts();
    save_refresh_token(app, rotated_refresh_token.as_str(), connection_mode)
        .await
        .map_err(|_| SessionCommandError::SecureStorageUnavailable)?;
    let snapshot = state
        .install(access_token, user, expires_in, connection_mode)
        .map_err(SessionCommandError::from)?;
    emit_session_change_if_needed(app, &before, &snapshot);
    Ok(snapshot)
}

fn emit_session_change_if_needed(
    app: &tauri::AppHandle,
    before: &SessionSnapshot,
    after: &SessionSnapshot,
) {
    if before != after {
        let _ = app.emit("pixiv-session-changed", after.clone());
    }
}

#[tauri::command]
async fn logout(
    app: tauri::AppHandle,
    state: tauri::State<'_, SessionState>,
) -> Result<SessionSnapshot, SessionCommandError> {
    let _operation = state.operation_guard().await;
    downloads::suspend_download_worker(&app).await;
    delete_refresh_token(&app)
        .await
        .map_err(|_| SessionCommandError::SecureStorageUnavailable)?;
    let snapshot = state.clear().map_err(SessionCommandError::from)?;
    let _ = app.emit("pixiv-session-changed", snapshot.clone());
    Ok(snapshot)
}

async fn ensure_authenticated_context(
    app: &tauri::AppHandle,
    state: &SessionState,
) -> Result<AuthenticatedContext, ApiCommandError> {
    if let Some(context) = state
        .authenticated_context(API_TOKEN_MINIMUM_TTL_SECONDS)
        .map_err(ApiCommandError::from)?
    {
        return Ok(context);
    }

    let _operation = state.operation_guard().await;
    if let Some(context) = state
        .authenticated_context(API_TOKEN_MINIMUM_TTL_SECONDS)
        .map_err(ApiCommandError::from)?
    {
        return Ok(context);
    }
    let snapshot = refresh_session_locked(app, state)
        .await
        .map_err(ApiCommandError::from)?;
    if !snapshot.logged_in {
        return Err(ApiCommandError::AuthenticationRequired);
    }
    state
        .authenticated_context(0)
        .map_err(ApiCommandError::from)?
        .ok_or(ApiCommandError::AuthenticationRequired)
}

async fn invalidate_session_generation(
    app: &tauri::AppHandle,
    state: &SessionState,
    generation: u64,
) -> Result<(), ApiCommandError> {
    let _operation = state.operation_guard().await;
    if state.generation().map_err(ApiCommandError::from)? != Some(generation) {
        return Ok(());
    }

    let before = state.snapshot().map_err(ApiCommandError::from)?;
    delete_refresh_token(app)
        .await
        .map_err(|_| ApiCommandError::SecureStorageUnavailable)?;
    let snapshot = state.clear().map_err(ApiCommandError::from)?;
    emit_session_change_if_needed(app, &before, &snapshot);
    Ok(())
}

async fn request_authenticated_data<T, F>(
    context: AuthenticatedContext,
    data_state: AuthenticatedDataState,
    request: F,
) -> Result<T, ApiCommandError>
where
    T: Send + 'static,
    F: Fn(&PixivApiClient, &str, &ClientRequestSignature, &str) -> Result<T, ApiError>
        + Send
        + 'static,
{
    tauri::async_runtime::spawn_blocking(move || {
        let http = data_state.client_for(context.connection_mode(), TrafficClass::Api, API_HOST)?;
        let signature = oauth_configuration()
            .and_then(|config| config.client_request_signature())
            .map_err(SessionCommandError::from)
            .map_err(ApiCommandError::from)?;
        request(
            &PixivApiClient::with_http(http),
            context.access_token(),
            &signature,
            context.user_id(),
        )
        .map_err(ApiCommandError::from)
    })
    .await
    .map_err(|_| ApiCommandError::RequestFailed)?
}

async fn execute_authenticated_data_request<T, F>(
    app: &tauri::AppHandle,
    session_state: &SessionState,
    data_state: AuthenticatedDataState,
    request: F,
) -> Result<T, ApiCommandError>
where
    T: Send + 'static,
    F: Fn(&PixivApiClient, &str, &ClientRequestSignature, &str) -> Result<T, ApiError>
        + Clone
        + Send
        + 'static,
{
    let context = ensure_authenticated_context(app, session_state).await?;
    let rejected_generation = context.generation();
    let first_attempt =
        request_authenticated_data(context, data_state.clone(), request.clone()).await;
    if !matches!(
        first_attempt,
        Err(ApiCommandError::AuthenticationRequired)
            | Err(ApiCommandError::UpstreamRejected { http_status: 401 })
    ) {
        return first_attempt;
    }

    let refreshed =
        refresh_context_after_rejection(app, session_state, rejected_generation).await?;
    let refreshed_generation = refreshed.generation();
    let second_attempt = request_authenticated_data(refreshed, data_state, request).await;
    if matches!(second_attempt, Err(ApiCommandError::AuthenticationRequired)) {
        invalidate_session_generation(app, session_state, refreshed_generation).await?;
    }
    second_attempt
}

async fn execute_authenticated_mutation<T, F>(
    app: &tauri::AppHandle,
    session_state: &SessionState,
    data_state: AuthenticatedDataState,
    request: F,
) -> Result<T, ApiCommandError>
where
    T: Send + 'static,
    F: Fn(&PixivApiClient, &str, &ClientRequestSignature, &str) -> Result<T, ApiError>
        + Clone
        + Send
        + 'static,
{
    let _permit = data_state
        .mutation_gate
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| ApiCommandError::StateUnavailable)?;
    execute_authenticated_data_request(app, session_state, data_state, request).await
}

async fn refresh_context_after_rejection(
    app: &tauri::AppHandle,
    state: &SessionState,
    rejected_generation: u64,
) -> Result<AuthenticatedContext, ApiCommandError> {
    let _operation = state.operation_guard().await;
    if state.generation().map_err(ApiCommandError::from)? != Some(rejected_generation) {
        if let Some(context) = state
            .authenticated_context(0)
            .map_err(ApiCommandError::from)?
        {
            return Ok(context);
        }
    }

    let snapshot = refresh_session_locked(app, state)
        .await
        .map_err(ApiCommandError::from)?;
    if !snapshot.logged_in {
        return Err(ApiCommandError::AuthenticationRequired);
    }
    state
        .authenticated_context(0)
        .map_err(ApiCommandError::from)?
        .ok_or(ApiCommandError::AuthenticationRequired)
}

#[tauri::command]
async fn get_recommended_illustrations(
    cursor: Option<String>,
    app: tauri::AppHandle,
    session_state: tauri::State<'_, SessionState>,
    data_state: tauri::State<'_, AuthenticatedDataState>,
) -> Result<IllustrationPage, ApiCommandError> {
    execute_authenticated_data_request(
        &app,
        &session_state,
        data_state.inner().clone(),
        move |api, token, signature, _user_id| {
            api.recommended_illustrations(token, cursor.as_deref(), signature)
        },
    )
    .await
}

#[tauri::command]
async fn get_recommended_manga(
    cursor: Option<String>,
    app: tauri::AppHandle,
    session_state: tauri::State<'_, SessionState>,
    data_state: tauri::State<'_, AuthenticatedDataState>,
) -> Result<IllustrationPage, ApiCommandError> {
    execute_authenticated_data_request(
        &app,
        &session_state,
        data_state.inner().clone(),
        move |api, token, signature, _user_id| {
            api.recommended_manga(token, cursor.as_deref(), signature)
        },
    )
    .await
}

#[tauri::command]
async fn get_recommended_novels(
    cursor: Option<String>,
    app: tauri::AppHandle,
    session_state: tauri::State<'_, SessionState>,
    data_state: tauri::State<'_, AuthenticatedDataState>,
) -> Result<NovelPage, ApiCommandError> {
    execute_authenticated_data_request(
        &app,
        &session_state,
        data_state.inner().clone(),
        move |api, token, signature, _user_id| {
            api.recommended_novels(token, cursor.as_deref(), signature)
        },
    )
    .await
}

#[tauri::command]
async fn get_novel_detail(
    novel_id: String,
    app: tauri::AppHandle,
    session_state: tauri::State<'_, SessionState>,
    data_state: tauri::State<'_, AuthenticatedDataState>,
) -> Result<NovelDetail, ApiCommandError> {
    execute_authenticated_data_request(
        &app,
        &session_state,
        data_state.inner().clone(),
        move |api, token, signature, _user_id| api.novel_detail(token, &novel_id, signature),
    )
    .await
}

#[tauri::command]
async fn get_novel_content(
    novel_id: String,
    app: tauri::AppHandle,
    session_state: tauri::State<'_, SessionState>,
    data_state: tauri::State<'_, AuthenticatedDataState>,
) -> Result<NovelContent, ApiCommandError> {
    execute_authenticated_data_request(
        &app,
        &session_state,
        data_state.inner().clone(),
        move |api, token, signature, _user_id| api.novel_content(token, &novel_id, signature),
    )
    .await
}

#[tauri::command]
async fn get_novel_series(
    series_id: String,
    cursor: Option<String>,
    app: tauri::AppHandle,
    session_state: tauri::State<'_, SessionState>,
    data_state: tauri::State<'_, AuthenticatedDataState>,
) -> Result<NovelSeriesPage, ApiCommandError> {
    execute_authenticated_data_request(
        &app,
        &session_state,
        data_state.inner().clone(),
        move |api, token, signature, _user_id| {
            api.novel_series(token, &series_id, cursor.as_deref(), signature)
        },
    )
    .await
}

#[tauri::command]
async fn search_novels(
    word: String,
    search_target: String,
    cursor: Option<String>,
    app: tauri::AppHandle,
    session_state: tauri::State<'_, SessionState>,
    data_state: tauri::State<'_, AuthenticatedDataState>,
) -> Result<NovelPage, ApiCommandError> {
    execute_authenticated_data_request(
        &app,
        &session_state,
        data_state.inner().clone(),
        move |api, token, signature, _user_id| {
            api.search_novels(token, &word, &search_target, cursor.as_deref(), signature)
        },
    )
    .await
}

#[tauri::command]
async fn get_user_novels(
    user_id: String,
    cursor: Option<String>,
    app: tauri::AppHandle,
    session_state: tauri::State<'_, SessionState>,
    data_state: tauri::State<'_, AuthenticatedDataState>,
) -> Result<NovelPage, ApiCommandError> {
    execute_authenticated_data_request(
        &app,
        &session_state,
        data_state.inner().clone(),
        move |api, token, signature, _authenticated_user_id| {
            api.user_novels(token, &user_id, cursor.as_deref(), signature)
        },
    )
    .await
}

#[tauri::command]
async fn get_followed_novels(
    cursor: Option<String>,
    app: tauri::AppHandle,
    session_state: tauri::State<'_, SessionState>,
    data_state: tauri::State<'_, AuthenticatedDataState>,
) -> Result<NovelPage, ApiCommandError> {
    execute_authenticated_data_request(
        &app,
        &session_state,
        data_state.inner().clone(),
        move |api, token, signature, _user_id| {
            api.followed_novels(token, cursor.as_deref(), signature)
        },
    )
    .await
}

#[tauri::command]
async fn get_bookmarked_novels(
    restrict: String,
    cursor: Option<String>,
    app: tauri::AppHandle,
    session_state: tauri::State<'_, SessionState>,
    data_state: tauri::State<'_, AuthenticatedDataState>,
) -> Result<NovelPage, ApiCommandError> {
    execute_authenticated_data_request(
        &app,
        &session_state,
        data_state.inner().clone(),
        move |api, token, signature, user_id| {
            api.bookmarked_novels(token, user_id, &restrict, cursor.as_deref(), signature)
        },
    )
    .await
}

#[tauri::command]
async fn get_ranking_novels(
    ranking_mode: String,
    cursor: Option<String>,
    app: tauri::AppHandle,
    session_state: tauri::State<'_, SessionState>,
    data_state: tauri::State<'_, AuthenticatedDataState>,
) -> Result<NovelPage, ApiCommandError> {
    execute_authenticated_data_request(
        &app,
        &session_state,
        data_state.inner().clone(),
        move |api, token, signature, _user_id| {
            api.ranking_novels(token, &ranking_mode, cursor.as_deref(), signature)
        },
    )
    .await
}

#[tauri::command]
async fn get_ugoira_metadata(
    illustration_id: String,
    app: tauri::AppHandle,
    session_state: tauri::State<'_, SessionState>,
    data_state: tauri::State<'_, AuthenticatedDataState>,
) -> Result<UgoiraMetadata, ApiCommandError> {
    execute_authenticated_data_request(
        &app,
        &session_state,
        data_state.inner().clone(),
        move |api, token, signature, _user_id| {
            api.ugoira_metadata(token, &illustration_id, signature)
        },
    )
    .await
}

#[tauri::command]
async fn get_illustration_detail(
    illustration_id: String,
    app: tauri::AppHandle,
    session_state: tauri::State<'_, SessionState>,
    data_state: tauri::State<'_, AuthenticatedDataState>,
) -> Result<IllustrationDetail, ApiCommandError> {
    execute_authenticated_data_request(
        &app,
        &session_state,
        data_state.inner().clone(),
        move |api, token, signature, _user_id| {
            api.illustration_detail(token, &illustration_id, signature)
        },
    )
    .await
}

#[tauri::command]
async fn get_illustration_series(
    series_id: String,
    cursor: Option<String>,
    app: tauri::AppHandle,
    session_state: tauri::State<'_, SessionState>,
    data_state: tauri::State<'_, AuthenticatedDataState>,
) -> Result<IllustrationSeriesPage, ApiCommandError> {
    execute_authenticated_data_request(
        &app,
        &session_state,
        data_state.inner().clone(),
        move |api, token, signature, _user_id| {
            api.illustration_series(token, &series_id, cursor.as_deref(), signature)
        },
    )
    .await
}

#[tauri::command]
async fn get_related_illustrations(
    illustration_id: String,
    cursor: Option<String>,
    app: tauri::AppHandle,
    session_state: tauri::State<'_, SessionState>,
    data_state: tauri::State<'_, AuthenticatedDataState>,
) -> Result<IllustrationPage, ApiCommandError> {
    execute_authenticated_data_request(
        &app,
        &session_state,
        data_state.inner().clone(),
        move |api, token, signature, _user_id| {
            api.related_illustrations(token, &illustration_id, cursor.as_deref(), signature)
        },
    )
    .await
}

#[tauri::command]
async fn get_user_detail(
    user_id: String,
    app: tauri::AppHandle,
    session_state: tauri::State<'_, SessionState>,
    data_state: tauri::State<'_, AuthenticatedDataState>,
) -> Result<UserDetail, ApiCommandError> {
    execute_authenticated_data_request(
        &app,
        &session_state,
        data_state.inner().clone(),
        move |api, token, signature, _authenticated_user_id| {
            api.user_detail(token, &user_id, signature)
        },
    )
    .await
}

#[tauri::command]
async fn get_user_illustrations(
    user_id: String,
    work_kind: String,
    cursor: Option<String>,
    app: tauri::AppHandle,
    session_state: tauri::State<'_, SessionState>,
    data_state: tauri::State<'_, AuthenticatedDataState>,
) -> Result<IllustrationPage, ApiCommandError> {
    execute_authenticated_data_request(
        &app,
        &session_state,
        data_state.inner().clone(),
        move |api, token, signature, _authenticated_user_id| {
            api.user_illustrations(token, &user_id, &work_kind, cursor.as_deref(), signature)
        },
    )
    .await
}

#[tauri::command]
async fn get_ranking_illustrations(
    ranking_mode: String,
    cursor: Option<String>,
    app: tauri::AppHandle,
    session_state: tauri::State<'_, SessionState>,
    data_state: tauri::State<'_, AuthenticatedDataState>,
) -> Result<IllustrationPage, ApiCommandError> {
    execute_authenticated_data_request(
        &app,
        &session_state,
        data_state.inner().clone(),
        move |api, token, signature, _user_id| {
            api.ranking_illustrations(token, &ranking_mode, cursor.as_deref(), signature)
        },
    )
    .await
}

#[tauri::command]
async fn get_trending_tags(
    app: tauri::AppHandle,
    session_state: tauri::State<'_, SessionState>,
    data_state: tauri::State<'_, AuthenticatedDataState>,
) -> Result<Vec<TrendingTag>, ApiCommandError> {
    execute_authenticated_data_request(
        &app,
        &session_state,
        data_state.inner().clone(),
        |api, token, signature, _user_id| api.trending_tags(token, signature),
    )
    .await
}

#[tauri::command]
async fn search_illustrations(
    word: String,
    search_target: String,
    cursor: Option<String>,
    app: tauri::AppHandle,
    session_state: tauri::State<'_, SessionState>,
    data_state: tauri::State<'_, AuthenticatedDataState>,
) -> Result<IllustrationPage, ApiCommandError> {
    execute_authenticated_data_request(
        &app,
        &session_state,
        data_state.inner().clone(),
        move |api, token, signature, _user_id| {
            api.search_illustrations(token, &word, &search_target, cursor.as_deref(), signature)
        },
    )
    .await
}

#[tauri::command]
async fn search_users(
    word: String,
    cursor: Option<String>,
    app: tauri::AppHandle,
    session_state: tauri::State<'_, SessionState>,
    data_state: tauri::State<'_, AuthenticatedDataState>,
) -> Result<UserPreviewPage, ApiCommandError> {
    execute_authenticated_data_request(
        &app,
        &session_state,
        data_state.inner().clone(),
        move |api, token, signature, _user_id| {
            api.search_users(token, &word, cursor.as_deref(), signature)
        },
    )
    .await
}

#[tauri::command]
async fn get_followed_users(
    restrict: String,
    cursor: Option<String>,
    app: tauri::AppHandle,
    session_state: tauri::State<'_, SessionState>,
    data_state: tauri::State<'_, AuthenticatedDataState>,
) -> Result<UserPreviewPage, ApiCommandError> {
    execute_authenticated_data_request(
        &app,
        &session_state,
        data_state.inner().clone(),
        move |api, token, signature, user_id| {
            api.followed_users(token, user_id, &restrict, cursor.as_deref(), signature)
        },
    )
    .await
}

#[tauri::command]
async fn get_followed_illustrations(
    cursor: Option<String>,
    app: tauri::AppHandle,
    session_state: tauri::State<'_, SessionState>,
    data_state: tauri::State<'_, AuthenticatedDataState>,
) -> Result<IllustrationPage, ApiCommandError> {
    execute_authenticated_data_request(
        &app,
        &session_state,
        data_state.inner().clone(),
        move |api, token, signature, _user_id| {
            api.followed_illustrations(token, cursor.as_deref(), signature)
        },
    )
    .await
}

#[tauri::command]
async fn get_bookmarked_illustrations(
    restrict: String,
    cursor: Option<String>,
    app: tauri::AppHandle,
    session_state: tauri::State<'_, SessionState>,
    data_state: tauri::State<'_, AuthenticatedDataState>,
) -> Result<IllustrationPage, ApiCommandError> {
    execute_authenticated_data_request(
        &app,
        &session_state,
        data_state.inner().clone(),
        move |api, token, signature, user_id| {
            api.bookmarked_illustrations(token, user_id, &restrict, cursor.as_deref(), signature)
        },
    )
    .await
}

#[tauri::command]
async fn set_illustration_bookmark(
    illustration_id: String,
    bookmarked: bool,
    restrict: String,
    app: tauri::AppHandle,
    session_state: tauri::State<'_, SessionState>,
    data_state: tauri::State<'_, AuthenticatedDataState>,
) -> Result<(), ApiCommandError> {
    execute_authenticated_mutation(
        &app,
        &session_state,
        data_state.inner().clone(),
        move |api, token, signature, _user_id| {
            if bookmarked {
                api.add_illustration_bookmark(token, &illustration_id, &restrict, signature)
            } else {
                api.delete_illustration_bookmark(token, &illustration_id, signature)
            }
        },
    )
    .await
}

#[tauri::command]
async fn set_novel_bookmark(
    novel_id: String,
    bookmarked: bool,
    restrict: String,
    app: tauri::AppHandle,
    session_state: tauri::State<'_, SessionState>,
    data_state: tauri::State<'_, AuthenticatedDataState>,
) -> Result<(), ApiCommandError> {
    execute_authenticated_mutation(
        &app,
        &session_state,
        data_state.inner().clone(),
        move |api, token, signature, _user_id| {
            if bookmarked {
                api.add_novel_bookmark(token, &novel_id, &restrict, signature)
            } else {
                api.delete_novel_bookmark(token, &novel_id, signature)
            }
        },
    )
    .await
}

#[tauri::command]
async fn set_user_follow(
    user_id: String,
    followed: bool,
    restrict: String,
    app: tauri::AppHandle,
    session_state: tauri::State<'_, SessionState>,
    data_state: tauri::State<'_, AuthenticatedDataState>,
) -> Result<(), ApiCommandError> {
    execute_authenticated_mutation(
        &app,
        &session_state,
        data_state.inner().clone(),
        move |api, token, signature, _authenticated_user_id| {
            if followed {
                api.follow_user(token, &user_id, &restrict, signature)
            } else {
                api.unfollow_user(token, &user_id, signature)
            }
        },
    )
    .await
}

#[tauri::command]
async fn get_illustration_comments(
    illustration_id: String,
    cursor: Option<String>,
    app: tauri::AppHandle,
    session_state: tauri::State<'_, SessionState>,
    data_state: tauri::State<'_, AuthenticatedDataState>,
) -> Result<CommentPage, ApiCommandError> {
    execute_authenticated_data_request(
        &app,
        &session_state,
        data_state.inner().clone(),
        move |api, token, signature, _user_id| {
            api.illustration_comments(token, &illustration_id, cursor.as_deref(), signature)
        },
    )
    .await
}

#[tauri::command]
async fn get_comment_replies(
    comment_id: String,
    cursor: Option<String>,
    app: tauri::AppHandle,
    session_state: tauri::State<'_, SessionState>,
    data_state: tauri::State<'_, AuthenticatedDataState>,
) -> Result<CommentPage, ApiCommandError> {
    execute_authenticated_data_request(
        &app,
        &session_state,
        data_state.inner().clone(),
        move |api, token, signature, _user_id| {
            api.comment_replies(token, &comment_id, cursor.as_deref(), signature)
        },
    )
    .await
}

#[tauri::command]
async fn add_illustration_comment(
    illustration_id: String,
    comment: String,
    parent_comment_id: Option<String>,
    app: tauri::AppHandle,
    session_state: tauri::State<'_, SessionState>,
    data_state: tauri::State<'_, AuthenticatedDataState>,
) -> Result<Comment, ApiCommandError> {
    execute_authenticated_mutation(
        &app,
        &session_state,
        data_state.inner().clone(),
        move |api, token, signature, _user_id| {
            api.add_illustration_comment(
                token,
                &illustration_id,
                &comment,
                parent_comment_id.as_deref(),
                signature,
            )
        },
    )
    .await
}

#[tauri::command]
async fn get_novel_comments(
    novel_id: String,
    cursor: Option<String>,
    app: tauri::AppHandle,
    session_state: tauri::State<'_, SessionState>,
    data_state: tauri::State<'_, AuthenticatedDataState>,
) -> Result<CommentPage, ApiCommandError> {
    execute_authenticated_data_request(
        &app,
        &session_state,
        data_state.inner().clone(),
        move |api, token, signature, _user_id| {
            api.novel_comments(token, &novel_id, cursor.as_deref(), signature)
        },
    )
    .await
}

#[tauri::command]
async fn get_novel_comment_replies(
    comment_id: String,
    cursor: Option<String>,
    app: tauri::AppHandle,
    session_state: tauri::State<'_, SessionState>,
    data_state: tauri::State<'_, AuthenticatedDataState>,
) -> Result<CommentPage, ApiCommandError> {
    execute_authenticated_data_request(
        &app,
        &session_state,
        data_state.inner().clone(),
        move |api, token, signature, _user_id| {
            api.novel_comment_replies(token, &comment_id, cursor.as_deref(), signature)
        },
    )
    .await
}

#[tauri::command]
async fn add_novel_comment(
    novel_id: String,
    comment: String,
    parent_comment_id: Option<String>,
    app: tauri::AppHandle,
    session_state: tauri::State<'_, SessionState>,
    data_state: tauri::State<'_, AuthenticatedDataState>,
) -> Result<Comment, ApiCommandError> {
    execute_authenticated_mutation(
        &app,
        &session_state,
        data_state.inner().clone(),
        move |api, token, signature, _user_id| {
            api.add_novel_comment(
                token,
                &novel_id,
                &comment,
                parent_comment_id.as_deref(),
                signature,
            )
        },
    )
    .await
}

#[derive(Clone, Copy)]
enum MediaExpectation {
    Image,
    Zip,
}

struct DownloadedMedia {
    bytes: Vec<u8>,
    content_type: &'static str,
}

fn offline_library(app: &tauri::AppHandle) -> Result<OfflineLibrary, ApiCommandError> {
    let root = paths::app_data_dir(app)
        .map_err(|_| ApiCommandError::OfflineUnavailable)?
        .join("offline-library");
    OfflineLibrary::open(root).map_err(ApiCommandError::from)
}

fn media_cache_root(app: &tauri::AppHandle) -> Result<std::path::PathBuf, ApiCommandError> {
    paths::app_cache_dir(app)
        .map(|root| root.join("media-v1"))
        .map_err(|_| ApiCommandError::CacheUnavailable)
}

fn storage_manager(app: &tauri::AppHandle) -> Result<Arc<StorageManager>, ApiCommandError> {
    let state = app.state::<StoragePolicyState>();
    let mut manager = state
        .manager
        .lock()
        .map_err(|_| ApiCommandError::StateUnavailable)?;
    if let Some(manager) = manager.as_ref() {
        return Ok(manager.clone());
    }
    let data_root = paths::app_data_dir(app).map_err(|_| ApiCommandError::StorageUnavailable)?;
    let cache_root = paths::app_cache_dir(app).map_err(|_| ApiCommandError::StorageUnavailable)?;
    let opened = Arc::new(StorageManager::open(data_root, cache_root)?);
    *manager = Some(opened.clone());
    Ok(opened)
}

fn media_cache_source_key(url: &str) -> Result<String, ApiCommandError> {
    let url = validated_media_url(url).map_err(ApiCommandError::from)?;
    let host = url.host_str().ok_or(ApiCommandError::InvalidMediaUrl)?;
    Ok(format!("{host}{}", url.path()))
}

fn media_cache_scope(mode: ConnectionMode) -> CacheScope {
    if mode == ConnectionMode::Compatible {
        CacheScope::Insecure
    } else {
        CacheScope::Verified
    }
}

fn download_media_blocking(
    data_state: &AuthenticatedDataState,
    mode: ConnectionMode,
    url: &str,
    max_bytes: usize,
    expectation: MediaExpectation,
) -> Result<DownloadedMedia, ApiCommandError> {
    let media_url = validated_media_url(url).map_err(ApiCommandError::from)?;
    let host = media_url
        .host_str()
        .ok_or(ApiCommandError::InvalidMediaUrl)?
        .to_owned();
    let client = data_state.client_for(mode, TrafficClass::Media, &host)?;
    let accept = match expectation {
        MediaExpectation::Image => "image/avif,image/webp,image/apng,image/*,*/*;q=0.8",
        MediaExpectation::Zip => "application/zip,application/octet-stream;q=0.9,*/*;q=0.1",
    };
    let response = client
        .get(media_url.clone())
        .header("Referer", MEDIA_REFERER)
        .header("User-Agent", MEDIA_USER_AGENT)
        .header("Accept", accept)
        .send()
        .map_err(|_| ApiCommandError::RequestFailed)?;
    let status = response.status();
    if !status.is_success() {
        return Err(ApiCommandError::UpstreamRejected {
            http_status: status.as_u16(),
        });
    }
    let header = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    let content_type = match expectation {
        MediaExpectation::Image => match header.as_str() {
            "image/jpeg" | "image/jpg" => "image/jpeg",
            "image/png" => "image/png",
            "image/gif" => "image/gif",
            "image/webp" => "image/webp",
            "image/avif" => "image/avif",
            _ => return Err(ApiCommandError::InvalidResponse),
        },
        MediaExpectation::Zip
            if matches!(
                header.as_str(),
                "application/zip" | "application/octet-stream" | "binary/octet-stream" | ""
            ) && media_url.path().to_ascii_lowercase().ends_with(".zip") =>
        {
            "application/zip"
        }
        MediaExpectation::Zip => return Err(ApiCommandError::InvalidResponse),
    };
    if response
        .content_length()
        .is_some_and(|length| length > max_bytes as u64)
    {
        return Err(ApiCommandError::MediaTooLarge);
    }
    let mut bytes = Vec::new();
    response
        .take((max_bytes + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| ApiCommandError::RequestFailed)?;
    if bytes.len() > max_bytes {
        return Err(ApiCommandError::MediaTooLarge);
    }
    Ok(DownloadedMedia {
        bytes,
        content_type,
    })
}

fn asset_extension(content_type: &str) -> Result<&'static str, ApiCommandError> {
    match content_type {
        "image/jpeg" => Ok("jpg"),
        "image/png" => Ok("png"),
        "image/gif" => Ok("gif"),
        "image/webp" => Ok("webp"),
        "image/avif" => Ok("avif"),
        _ => Err(ApiCommandError::InvalidResponse),
    }
}

#[tauri::command]
async fn download_artwork(
    illustration_id: String,
    app: tauri::AppHandle,
    session_state: tauri::State<'_, SessionState>,
    data_state: tauri::State<'_, AuthenticatedDataState>,
) -> Result<OfflineEntry, ApiCommandError> {
    perform_artwork_download(
        illustration_id,
        &app,
        &session_state,
        data_state.inner().clone(),
        None,
    )
    .await
}

async fn perform_artwork_download(
    illustration_id: String,
    app: &tauri::AppHandle,
    session_state: &SessionState,
    data_state: AuthenticatedDataState,
    progress: Option<DownloadProgress>,
) -> Result<OfflineEntry, ApiCommandError> {
    if let Some(progress) = &progress {
        progress.checkpoint()?;
    }
    let storage = storage_manager(app)?;
    storage.ensure_offline_write(1)?;
    let detail = execute_authenticated_data_request(app, session_state, data_state.clone(), {
        let illustration_id = illustration_id.clone();
        move |api, token, signature, _user_id| {
            api.illustration_detail(token, &illustration_id, signature)
        }
    })
    .await?;
    if detail.pages.is_empty() {
        return Err(ApiCommandError::InvalidResponse);
    }
    let total_items =
        u32::try_from(detail.pages.len()).map_err(|_| ApiCommandError::InvalidResponse)?;
    if let Some(progress) = &progress {
        progress.update_metadata(
            Some(detail.illustration.title.clone()),
            Some(detail.illustration.author.name.clone()),
        )?;
        progress.update(0, total_items, 0)?;
    }
    let (session_mode, generation) = session_state
        .connection_context()
        .map_err(ApiCommandError::from)?
        .ok_or(ApiCommandError::AuthenticationRequired)?;
    let mode = data_state.media_mode_for(session_mode, generation)?;
    let permit = data_state
        .library_gate
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| ApiCommandError::StateUnavailable)?;
    let library = offline_library(app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        let metadata =
            serde_json::to_vec_pretty(&detail).map_err(|_| ApiCommandError::InvalidResponse)?;
        storage.ensure_offline_write(metadata.len() as u64)?;
        let draft = EntryDraft {
            kind: OfflineKind::Artwork,
            resource_id: illustration_id,
            title: detail.illustration.title.clone(),
            author: detail.illustration.author.name.clone(),
            cover_url: detail.illustration.thumbnail_url.clone(),
        };
        library
            .store_entry(draft, |writer| {
                writer.write_asset("detail.json", "application/json", &metadata)?;
                let mut downloaded_bytes = 0_u64;
                for (index, page) in detail.pages.iter().enumerate() {
                    if let Some(progress) = &progress {
                        progress.checkpoint().map_err(|_| LibraryError::Io)?;
                    }
                    let url = page
                        .original_url
                        .as_deref()
                        .or(page.display_url.as_deref())
                        .ok_or(LibraryError::InvalidManifest)?;
                    let media = download_media_blocking(
                        &data_state,
                        mode,
                        url,
                        MAX_ARTWORK_ASSET_BYTES,
                        MediaExpectation::Image,
                    )
                    .map_err(|_| LibraryError::Io)?;
                    storage
                        .ensure_offline_write(media.bytes.len() as u64)
                        .map_err(|_| LibraryError::Io)?;
                    let extension = asset_extension(media.content_type)
                        .map_err(|_| LibraryError::InvalidContentType)?;
                    let name = format!("page-{:04}.{extension}", page.page_index + 1);
                    writer.write_asset(&name, media.content_type, &media.bytes)?;
                    downloaded_bytes = downloaded_bytes.saturating_add(media.bytes.len() as u64);
                    if let Some(progress) = &progress {
                        progress
                            .update(
                                u32::try_from(index + 1).map_err(|_| LibraryError::Io)?,
                                total_items,
                                downloaded_bytes,
                            )
                            .map_err(|_| LibraryError::Io)?;
                    }
                }
                Ok(())
            })
            .map_err(ApiCommandError::from)
    })
    .await
    .map_err(|_| ApiCommandError::RequestFailed)?
}

#[tauri::command]
async fn download_novel(
    novel_id: String,
    app: tauri::AppHandle,
    session_state: tauri::State<'_, SessionState>,
    data_state: tauri::State<'_, AuthenticatedDataState>,
) -> Result<OfflineEntry, ApiCommandError> {
    perform_novel_download(
        novel_id,
        &app,
        &session_state,
        data_state.inner().clone(),
        None,
    )
    .await
}

async fn perform_novel_download(
    novel_id: String,
    app: &tauri::AppHandle,
    session_state: &SessionState,
    data_state: AuthenticatedDataState,
    progress: Option<DownloadProgress>,
) -> Result<OfflineEntry, ApiCommandError> {
    if let Some(progress) = &progress {
        progress.checkpoint()?;
    }
    let storage = storage_manager(app)?;
    storage.ensure_offline_write(1)?;
    let (detail, content) =
        execute_authenticated_data_request(app, session_state, data_state.clone(), {
            let novel_id = novel_id.clone();
            move |api, token, signature, _user_id| {
                Ok((
                    api.novel_detail(token, &novel_id, signature)?,
                    api.novel_content(token, &novel_id, signature)?,
                ))
            }
        })
        .await?;
    if let Some(progress) = &progress {
        progress.update_metadata(
            Some(detail.novel.title.clone()),
            Some(detail.novel.author.name.clone()),
        )?;
        progress.update(0, 1, 0)?;
    }
    let (session_mode, generation) = session_state
        .connection_context()
        .map_err(ApiCommandError::from)?
        .ok_or(ApiCommandError::AuthenticationRequired)?;
    let mode = data_state.media_mode_for(session_mode, generation)?;
    let permit = data_state
        .library_gate
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| ApiCommandError::StateUnavailable)?;
    let library = offline_library(app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        if let Some(progress) = &progress {
            progress
                .checkpoint()
                .map_err(|_| ApiCommandError::DownloadInterrupted)?;
        }
        let detail_json =
            serde_json::to_vec_pretty(&detail).map_err(|_| ApiCommandError::InvalidResponse)?;
        let content_json =
            serde_json::to_vec_pretty(&content).map_err(|_| ApiCommandError::InvalidResponse)?;
        let cover_url = detail.novel.cover_url.clone().or(content.cover_url.clone());
        let cover = cover_url
            .as_deref()
            .map(|url| {
                download_media_blocking(
                    &data_state,
                    mode,
                    url,
                    MAX_THUMBNAIL_BYTES,
                    MediaExpectation::Image,
                )
            })
            .transpose()?;
        let required_bytes = (detail_json.len() as u64)
            .saturating_add(content_json.len() as u64)
            .saturating_add(cover.as_ref().map_or(0, |media| media.bytes.len() as u64));
        storage.ensure_offline_write(required_bytes.max(1))?;
        let draft = EntryDraft {
            kind: OfflineKind::Novel,
            resource_id: novel_id,
            title: detail.novel.title.clone(),
            author: detail.novel.author.name.clone(),
            cover_url,
        };
        let entry = library
            .store_entry(draft, |writer| {
                writer.write_asset("detail.json", "application/json", &detail_json)?;
                writer.write_asset("content.json", "application/json", &content_json)?;
                if let Some(cover) = &cover {
                    let extension = asset_extension(cover.content_type)
                        .map_err(|_| LibraryError::InvalidContentType)?;
                    writer.write_asset(
                        &format!("cover.{extension}"),
                        cover.content_type,
                        &cover.bytes,
                    )?;
                }
                Ok(())
            })
            .map_err(ApiCommandError::from)?;
        if let Some(progress) = &progress {
            progress.update(1, 1, entry.size_bytes)?;
        }
        Ok(entry)
    })
    .await
    .map_err(|_| ApiCommandError::RequestFailed)?
}

#[tauri::command]
async fn prepare_ugoira(
    illustration_id: String,
    app: tauri::AppHandle,
    session_state: tauri::State<'_, SessionState>,
    data_state: tauri::State<'_, AuthenticatedDataState>,
) -> Result<PreparedUgoira, ApiCommandError> {
    perform_ugoira_download(
        illustration_id,
        &app,
        &session_state,
        data_state.inner().clone(),
        None,
    )
    .await
}

async fn perform_ugoira_download(
    illustration_id: String,
    app: &tauri::AppHandle,
    session_state: &SessionState,
    data_state: AuthenticatedDataState,
    progress: Option<DownloadProgress>,
) -> Result<PreparedUgoira, ApiCommandError> {
    if let Some(progress) = &progress {
        progress.checkpoint()?;
    }
    let storage = storage_manager(app)?;
    storage.ensure_offline_write(1)?;
    let (detail, metadata) =
        execute_authenticated_data_request(app, session_state, data_state.clone(), {
            let illustration_id = illustration_id.clone();
            move |api, token, signature, _user_id| {
                Ok((
                    api.illustration_detail(token, &illustration_id, signature)?,
                    api.ugoira_metadata(token, &illustration_id, signature)?,
                ))
            }
        })
        .await?;
    if detail.illustration.kind != "ugoira" {
        return Err(ApiCommandError::InvalidInput);
    }
    let total_items =
        u32::try_from(metadata.frames.len()).map_err(|_| ApiCommandError::InvalidResponse)?;
    if total_items == 0 {
        return Err(ApiCommandError::InvalidResponse);
    }
    if let Some(progress) = &progress {
        progress.update_metadata(
            Some(detail.illustration.title.clone()),
            Some(detail.illustration.author.name.clone()),
        )?;
        progress.update(0, total_items, 0)?;
    }
    let (session_mode, generation) = session_state
        .connection_context()
        .map_err(ApiCommandError::from)?
        .ok_or(ApiCommandError::AuthenticationRequired)?;
    let mode = data_state.media_mode_for(session_mode, generation)?;
    let permit = data_state
        .library_gate
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| ApiCommandError::StateUnavailable)?;
    let library = offline_library(app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        if let Some(progress) = &progress {
            progress.checkpoint()?;
        }
        let archive = download_media_blocking(
            &data_state,
            mode,
            &metadata.zip_url,
            MAX_UGOIRA_ARCHIVE_BYTES,
            MediaExpectation::Zip,
        )?;
        let metadata_json =
            serde_json::to_vec_pretty(&metadata).map_err(|_| ApiCommandError::InvalidResponse)?;
        let prepared_frames: Vec<_> = metadata
            .frames
            .iter()
            .enumerate()
            .map(|(index, frame)| PreparedUgoiraFrame {
                asset_name: format!(
                    "frame-{index:06}.{}",
                    frame
                        .file_name
                        .rsplit('.')
                        .next()
                        .unwrap_or("jpg")
                        .to_ascii_lowercase()
                ),
                delay_ms: frame.delay_ms,
            })
            .collect();
        let draft = EntryDraft {
            kind: OfflineKind::Ugoira,
            resource_id: illustration_id,
            title: detail.illustration.title.clone(),
            author: detail.illustration.author.name.clone(),
            cover_url: detail.illustration.thumbnail_url.clone(),
        };
        let mut zip = zip::ZipArchive::new(Cursor::new(archive.bytes))
            .map_err(|_| ApiCommandError::InvalidResponse)?;
        let mut expected_bytes = metadata_json.len() as u64;
        for frame in &metadata.frames {
            let file = zip
                .by_name(&frame.file_name)
                .map_err(|_| ApiCommandError::InvalidResponse)?;
            if !file.is_file() || file.size() > MAX_ARTWORK_ASSET_BYTES as u64 {
                return Err(ApiCommandError::MediaTooLarge);
            }
            expected_bytes = expected_bytes.saturating_add(file.size());
            if expected_bytes > MAX_UGOIRA_ARCHIVE_BYTES as u64 {
                return Err(ApiCommandError::MediaTooLarge);
            }
        }
        storage.ensure_offline_write(expected_bytes.max(1))?;
        let entry = library.store_entry(draft, |writer| {
            writer.write_asset("metadata.json", "application/json", &metadata_json)?;
            let mut total_uncompressed = 0_u64;
            for (index, (frame, prepared)) in
                metadata.frames.iter().zip(&prepared_frames).enumerate()
            {
                if let Some(progress) = &progress {
                    progress.checkpoint().map_err(|_| LibraryError::Io)?;
                }
                let mut file = zip
                    .by_name(&frame.file_name)
                    .map_err(|_| LibraryError::InvalidManifest)?;
                if !file.is_file() || file.size() > MAX_ARTWORK_ASSET_BYTES as u64 {
                    return Err(LibraryError::AssetTooLarge);
                }
                total_uncompressed = total_uncompressed.saturating_add(file.size());
                if total_uncompressed > MAX_UGOIRA_ARCHIVE_BYTES as u64 {
                    return Err(LibraryError::AssetTooLarge);
                }
                let mut bytes = Vec::new();
                file.by_ref()
                    .take((MAX_ARTWORK_ASSET_BYTES + 1) as u64)
                    .read_to_end(&mut bytes)
                    .map_err(|_| LibraryError::Io)?;
                if bytes.len() > MAX_ARTWORK_ASSET_BYTES {
                    return Err(LibraryError::AssetTooLarge);
                }
                let content_type = match frame
                    .file_name
                    .rsplit('.')
                    .next()
                    .unwrap_or_default()
                    .to_ascii_lowercase()
                    .as_str()
                {
                    "jpg" | "jpeg" => "image/jpeg",
                    "png" => "image/png",
                    "gif" => "image/gif",
                    "webp" => "image/webp",
                    _ => return Err(LibraryError::InvalidContentType),
                };
                writer.write_asset(&prepared.asset_name, content_type, &bytes)?;
                if let Some(progress) = &progress {
                    progress
                        .update(
                            u32::try_from(index + 1).map_err(|_| LibraryError::Io)?,
                            total_items,
                            total_uncompressed,
                        )
                        .map_err(|_| LibraryError::Io)?;
                }
            }
            Ok(())
        })?;
        Ok(PreparedUgoira {
            entry,
            frames: prepared_frames,
        })
    })
    .await
    .map_err(|_| ApiCommandError::RequestFailed)?
}

#[tauri::command]
async fn list_offline_entries(app: tauri::AppHandle) -> Result<Vec<OfflineEntry>, ApiCommandError> {
    let library = offline_library(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        library.list_entries().map_err(ApiCommandError::from)
    })
    .await
    .map_err(|_| ApiCommandError::OfflineUnavailable)?
}

#[tauri::command]
async fn get_offline_stats(app: tauri::AppHandle) -> Result<LibraryStats, ApiCommandError> {
    let library = offline_library(&app)?;
    tauri::async_runtime::spawn_blocking(move || library.stats().map_err(ApiCommandError::from))
        .await
        .map_err(|_| ApiCommandError::OfflineUnavailable)?
}

#[tauri::command]
async fn read_offline_asset(
    key: String,
    asset_name: String,
    app: tauri::AppHandle,
) -> Result<tauri::ipc::Response, ApiCommandError> {
    let library = offline_library(&app)?;
    let asset = tauri::async_runtime::spawn_blocking(move || library.read_asset(&key, &asset_name))
        .await
        .map_err(|_| ApiCommandError::OfflineUnavailable)?
        .map_err(ApiCommandError::from)?;
    if !asset.content_type.starts_with("image/") {
        return Err(ApiCommandError::InvalidResponse);
    }
    Ok(tauri::ipc::Response::new(asset.bytes))
}

#[tauri::command]
async fn read_offline_text(
    key: String,
    asset_name: String,
    app: tauri::AppHandle,
) -> Result<String, ApiCommandError> {
    let library = offline_library(&app)?;
    let asset = tauri::async_runtime::spawn_blocking(move || library.read_asset(&key, &asset_name))
        .await
        .map_err(|_| ApiCommandError::OfflineUnavailable)?
        .map_err(ApiCommandError::from)?;
    if !matches!(
        asset.content_type.as_str(),
        "application/json" | "text/plain; charset=utf-8"
    ) || asset.bytes.len() > 32 * 1024 * 1024
    {
        return Err(ApiCommandError::InvalidResponse);
    }
    String::from_utf8(asset.bytes).map_err(|_| ApiCommandError::InvalidResponse)
}

#[tauri::command]
async fn remove_offline_entry(
    key: String,
    app: tauri::AppHandle,
    data_state: tauri::State<'_, AuthenticatedDataState>,
) -> Result<bool, ApiCommandError> {
    let permit = data_state
        .library_gate
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| ApiCommandError::StateUnavailable)?;
    let library = offline_library(&app)?;
    let catalog = catalog::open_catalog(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        let removed = library.remove_entry(&key)?;
        if removed {
            catalog.remove_entry(&key)?;
        }
        Ok(removed)
    })
    .await
    .map_err(|_| ApiCommandError::OfflineUnavailable)?
}

#[tauri::command]
async fn fetch_pixiv_thumbnail(
    url: String,
    cache_kind: Option<CacheKind>,
    app: tauri::AppHandle,
    session_state: tauri::State<'_, SessionState>,
    data_state: tauri::State<'_, AuthenticatedDataState>,
    cache_state: tauri::State<'_, MediaCacheState>,
) -> Result<tauri::ipc::Response, ApiCommandError> {
    let (session_mode, generation) = session_state
        .connection_context()
        .map_err(ApiCommandError::from)?
        .ok_or(ApiCommandError::AuthenticationRequired)?;
    let mode = data_state.media_mode_for(session_mode, generation)?;
    let cache_kind = cache_kind.unwrap_or(CacheKind::Thumbnail);
    let cache_scope = media_cache_scope(mode);
    let cache_source_key = media_cache_source_key(&url)?;
    let cache_root = media_cache_root(&app)?;
    let storage = storage_manager(&app)?;
    let cache_limit_bytes = storage.cache_limit_bytes()?;
    let read_gate = cache_state.gate.clone();
    let read_root = cache_root.clone();
    let read_key = cache_source_key.clone();
    let cached = tauri::async_runtime::spawn_blocking(move || {
        let _guard = read_gate.lock().ok()?;
        let mut cache = MediaCache::open(read_root, cache_limit_bytes).ok()?;
        cache
            .get(
                cache_scope,
                cache_kind,
                &read_key,
                MAX_THUMBNAIL_BYTES as u64,
            )
            .ok()?
    })
    .await
    .unwrap_or(None);
    if let Some(bytes) = cached {
        return Ok(tauri::ipc::Response::new(bytes));
    }

    let permit = data_state
        .media_gate
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| ApiCommandError::StateUnavailable)?;
    let data_state = data_state.inner().clone();

    let media = tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        download_media_blocking(
            &data_state,
            mode,
            &url,
            MAX_THUMBNAIL_BYTES,
            MediaExpectation::Image,
        )
    })
    .await
    .map_err(|_| ApiCommandError::RequestFailed)??;

    let store_gate = cache_state.gate.clone();
    let store_storage = storage.clone();
    let bytes = tauri::async_runtime::spawn_blocking(move || {
        if let Ok(_guard) = store_gate.lock() {
            if store_storage
                .allows_cache_write(media.bytes.len() as u64)
                .unwrap_or(false)
            {
                if let Ok(limit) = store_storage.cache_limit_bytes() {
                    if let Ok(mut cache) = MediaCache::open(cache_root, limit) {
                        let _ = cache.put(cache_scope, cache_kind, &cache_source_key, &media.bytes);
                    }
                }
            }
        }
        media.bytes
    })
    .await
    .map_err(|_| ApiCommandError::CacheUnavailable)?;

    Ok(tauri::ipc::Response::new(bytes))
}

#[tauri::command]
async fn get_media_cache_stats(
    app: tauri::AppHandle,
    cache_state: tauri::State<'_, MediaCacheState>,
) -> Result<CacheStats, ApiCommandError> {
    let root = media_cache_root(&app)?;
    let limit = storage_manager(&app)?.cache_limit_bytes()?;
    let gate = cache_state.gate.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = gate.lock().map_err(|_| ApiCommandError::StateUnavailable)?;
        Ok(MediaCache::open(root, limit)?.stats())
    })
    .await
    .map_err(|_| ApiCommandError::CacheUnavailable)?
}

#[tauri::command]
async fn clear_media_cache(
    confirmed: bool,
    app: tauri::AppHandle,
    cache_state: tauri::State<'_, MediaCacheState>,
) -> Result<CacheStats, ApiCommandError> {
    if !confirmed {
        return Err(ApiCommandError::InvalidInput);
    }
    let root = media_cache_root(&app)?;
    let limit = storage_manager(&app)?.cache_limit_bytes()?;
    let gate = cache_state.gate.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let _guard = gate.lock().map_err(|_| ApiCommandError::StateUnavailable)?;
        MediaCache::open(root, limit)?
            .clear()
            .map_err(ApiCommandError::from)
    })
    .await
    .map_err(|_| ApiCommandError::CacheUnavailable)?;
    if let Ok(stats) = &result {
        record_diagnostic_event(
            &app,
            DiagnosticEntry::now(
                LogLevel::Info,
                LogComponent::Storage,
                LogEvent::MediaCacheCleared,
            )
            .with_item_count(stats.entry_count),
        );
    }
    result
}

#[tauri::command]
async fn get_storage_status(app: tauri::AppHandle) -> Result<StorageStatus, ApiCommandError> {
    let manager = storage_manager(&app)?;
    tauri::async_runtime::spawn_blocking(move || manager.status().map_err(ApiCommandError::from))
        .await
        .map_err(|_| ApiCommandError::StorageUnavailable)?
}

#[tauri::command]
async fn set_media_cache_limit(
    cache_limit_bytes: u64,
    app: tauri::AppHandle,
    cache_state: tauri::State<'_, MediaCacheState>,
) -> Result<StorageStatus, ApiCommandError> {
    let root = media_cache_root(&app)?;
    let manager = storage_manager(&app)?;
    let gate = cache_state.gate.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = gate.lock().map_err(|_| ApiCommandError::StateUnavailable)?;
        manager.set_cache_limit(cache_limit_bytes)?;
        MediaCache::open(root, manager.cache_limit_bytes()?)?;
        manager.status().map_err(ApiCommandError::from)
    })
    .await
    .map_err(|_| ApiCommandError::StorageUnavailable)?
}

#[tauri::command]
async fn clear_local_data(
    request: LocalDataClearRequest,
    app: tauri::AppHandle,
    login_state: tauri::State<'_, InteractiveLoginState>,
    session_state: tauri::State<'_, SessionState>,
    data_state: tauri::State<'_, AuthenticatedDataState>,
    cache_state: tauri::State<'_, MediaCacheState>,
    update_manager: tauri::State<'_, UpdateManagerState>,
) -> Result<LocalDataClearReport, ApiCommandError> {
    if request.confirmation != "清除" {
        return Err(ApiCommandError::InvalidInput);
    }

    let _operation = session_state.operation_guard().await;
    let mut failed_steps = Vec::new();

    let mut login_state_cleared = (|| -> Result<(), ()> {
        login_state.attempt.lock().map_err(|_| ())?.take();
        login_state.proxy.lock().map_err(|_| ())?.take();
        Ok(())
    })()
    .is_ok();
    for (label, window) in app.webview_windows() {
        if label.starts_with(LOGIN_WINDOW_PREFIX) && window.destroy().is_err() {
            login_state_cleared = false;
        }
    }
    if !login_state_cleared {
        failed_steps.push(LocalDataClearFailure::LoginState);
    }

    let credentials_cleared = delete_refresh_token(&app).await.is_ok();
    if !credentials_cleared {
        failed_steps.push(LocalDataClearFailure::SecureStorage);
    }

    let session_cleared = match session_state.clear() {
        Ok(snapshot) => {
            let _ = app.emit("pixiv-session-changed", snapshot);
            true
        }
        Err(_) => false,
    };
    if !session_cleared {
        failed_steps.push(LocalDataClearFailure::Session);
    }

    let transport_state_cleared = data_state.clear_transport_state().is_ok();
    if !transport_state_cleared {
        failed_steps.push(LocalDataClearFailure::TransportState);
    }

    let download_tasks_removed = match downloads::clear_download_queue(&app).await {
        Ok(stats) => stats.task_count,
        Err(_) => {
            failed_steps.push(LocalDataClearFailure::DownloadQueue);
            0
        }
    };

    let offline_result = match data_state.library_gate.clone().acquire_owned().await {
        Ok(permit) => match offline_library(&app) {
            Ok(library) => tauri::async_runtime::spawn_blocking(move || {
                let _permit = permit;
                library.clear().map_err(ApiCommandError::from)
            })
            .await
            .map_err(|_| ApiCommandError::OfflineUnavailable)
            .and_then(|result| result),
            Err(error) => Err(error),
        },
        Err(_) => Err(ApiCommandError::StateUnavailable),
    };
    let (offline_entries_removed, offline_bytes_removed) = match offline_result {
        Ok(stats) => (stats.entry_count, stats.size_bytes),
        Err(_) => {
            failed_steps.push(LocalDataClearFailure::OfflineLibrary);
            (0, 0)
        }
    };

    let catalog_result = catalog::clear_local_catalog(&app).await;
    let (local_collections_removed, local_organized_entries_removed, local_tags_removed) =
        match catalog_result {
            Ok(stats) => (
                stats.collection_count,
                stats.organized_entry_count,
                stats.tag_count,
            ),
            Err(_) => {
                failed_steps.push(LocalDataClearFailure::LocalCatalog);
                (0, 0, 0)
            }
        };

    let browsing_history_entries_removed = match history::clear_all_history(&app).await {
        Ok(stats) => stats.entries_removed,
        Err(_) => {
            failed_steps.push(LocalDataClearFailure::BrowsingHistory);
            0
        }
    };

    let storage = storage_manager(&app).ok();
    let cache_limit = storage
        .as_ref()
        .and_then(|manager| manager.cache_limit_bytes().ok())
        .unwrap_or(DEFAULT_CACHE_LIMIT_BYTES);
    let cache_root = media_cache_root(&app);
    let cache_gate = cache_state.gate.clone();
    let cache_result = match cache_root {
        Ok(root) => tauri::async_runtime::spawn_blocking(move || {
            let _guard = cache_gate
                .lock()
                .map_err(|_| ApiCommandError::StateUnavailable)?;
            MediaCache::open(root, cache_limit)?
                .clear()
                .map_err(ApiCommandError::from)
        })
        .await
        .map_err(|_| ApiCommandError::CacheUnavailable)
        .and_then(|result| result),
        Err(error) => Err(error),
    };
    let (cache_entries_removed, cache_bytes_removed) = match cache_result {
        Ok(stats) => (stats.entry_count, stats.size_bytes),
        Err(_) => {
            failed_steps.push(LocalDataClearFailure::MediaCache);
            (0, 0)
        }
    };

    let storage_settings_reset = storage
        .as_ref()
        .is_some_and(|manager| manager.reset_settings().is_ok());
    if !storage_settings_reset {
        failed_steps.push(LocalDataClearFailure::StorageSettings);
    }

    let export_settings_reset = exports::clear_all_export_settings(&app).await.is_ok();
    if !export_settings_reset {
        failed_steps.push(LocalDataClearFailure::ExportSettings);
    }

    let update_settings_reset = updates::clear_update_state(&app, update_manager.inner()).is_ok();
    if !update_settings_reset {
        failed_steps.push(LocalDataClearFailure::UpdateSettings);
    }

    let login_webview_data_cleared = clear_login_webview_data(&app).await.is_ok();
    if !login_webview_data_cleared {
        failed_steps.push(LocalDataClearFailure::LoginWebView);
    }

    let diagnostic_log_result = with_diagnostic_log(&app, DiagnosticLog::clear);
    let diagnostic_log_entries_removed = match diagnostic_log_result {
        Ok(summary) => summary.entry_count,
        Err(_) => {
            failed_steps.push(LocalDataClearFailure::DiagnosticLog);
            0
        }
    };

    Ok(LocalDataClearReport {
        complete: failed_steps.is_empty(),
        credentials_cleared,
        session_cleared,
        transport_state_cleared,
        offline_entries_removed,
        offline_bytes_removed,
        cache_entries_removed,
        cache_bytes_removed,
        login_webview_data_cleared,
        diagnostic_log_entries_removed,
        download_tasks_removed,
        storage_settings_reset,
        export_settings_reset,
        update_settings_reset,
        local_collections_removed,
        local_organized_entries_removed,
        local_tags_removed,
        browsing_history_entries_removed,
        failed_steps,
    })
}

#[cfg(target_os = "android")]
async fn clear_login_webview_data(app: &tauri::AppHandle) -> Result<(), ()> {
    app.state::<AndroidLoginWebViewPlugin>()
        .0
        .clone()
        .run_mobile_plugin_async::<()>("clearLocalWebData", EmptyMobilePayload {})
        .await
        .map_err(|_| ())
}

#[cfg(not(target_os = "android"))]
async fn clear_login_webview_data(app: &tauri::AppHandle) -> Result<(), ()> {
    let root = paths::app_cache_dir(app)
        .map_err(|_| ())?
        .join("login-webview");
    tauri::async_runtime::spawn_blocking(move || {
        if !root.exists() {
            return Ok(());
        }
        for attempt in 0..3 {
            match std::fs::remove_dir_all(&root) {
                Ok(()) => return Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
                Err(_) if attempt < 2 => {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                Err(_) => return Err(()),
            }
        }
        Err(())
    })
    .await
    .map_err(|_| ())?
}

#[tauri::command]
fn acknowledge_insecure_media_fallback(
    session_state: tauri::State<'_, SessionState>,
    data_state: tauri::State<'_, AuthenticatedDataState>,
) -> Result<(), ApiCommandError> {
    let (mode, generation) = session_state
        .connection_context()
        .map_err(ApiCommandError::from)?
        .ok_or(ApiCommandError::AuthenticationRequired)?;
    if mode != ConnectionMode::Ech {
        return Err(ApiCommandError::StateUnavailable);
    }
    data_state.acknowledge_insecure_media_fallback(generation);
    Ok(())
}

#[cfg(not(target_os = "android"))]
fn mode_code(mode: ConnectionMode) -> &'static str {
    match mode {
        ConnectionMode::Standard => "standard",
        ConnectionMode::Ech => "ech",
        ConnectionMode::Compatible => "compatible",
    }
}

#[tauri::command]
fn finish_interactive_login_view(
    launch_id: u64,
    state: tauri::State<'_, InteractiveLoginState>,
) -> Result<bool, LoginLaunchError> {
    cleanup_login_proxy(&state, launch_id)
}

#[tauri::command]
fn cancel_interactive_login(
    app: tauri::AppHandle,
    state: tauri::State<'_, InteractiveLoginState>,
) -> Result<bool, LoginPreparationError> {
    let mut pending = state
        .attempt
        .lock()
        .map_err(|_| LoginPreparationError::StateUnavailable)?;
    let cancelled = if let Some(mut pending) = pending.take() {
        pending
            .attempt
            .cancel()
            .map_err(LoginPreparationError::from)?;
        true
    } else {
        false
    };
    drop(pending);
    state
        .proxy
        .lock()
        .map_err(|_| LoginPreparationError::StateUnavailable)?
        .take();
    for (label, window) in app.webview_windows() {
        if label.starts_with(LOGIN_WINDOW_PREFIX) {
            let _ = window.destroy();
        }
    }
    Ok(cancelled)
}

impl From<PolicyError> for LoginPreparationError {
    fn from(error: PolicyError) -> Self {
        match error {
            PolicyError::InvalidHost { host } => Self::InvalidHost { host },
            PolicyError::EchUnavailable { host } => Self::EchUnavailable { host },
            PolicyError::CompatibleDirectUnavailable { host } => {
                Self::CompatibleDirectUnavailable { host }
            }
            PolicyError::InsecureTransportForbidden { host } => {
                Self::InsecureTransportForbidden { host }
            }
            PolicyError::WebViewProxyUnavailable { host } => Self::WebViewProxyUnavailable { host },
        }
    }
}

impl From<LoginRouteError> for LoginPreparationError {
    fn from(error: LoginRouteError) -> Self {
        match error {
            LoginRouteError::Policy(error) => error.into(),
            LoginRouteError::UnsafeAcknowledgementRequired { host } => {
                Self::UnsafeAcknowledgementRequired { host }
            }
        }
    }
}

impl From<PolicyError> for LoginLaunchError {
    fn from(error: PolicyError) -> Self {
        match error {
            PolicyError::InvalidHost { host } => Self::InvalidHost { host },
            PolicyError::EchUnavailable { host } => Self::EchUnavailable { host },
            PolicyError::CompatibleDirectUnavailable { host } => {
                Self::CompatibleDirectUnavailable { host }
            }
            PolicyError::InsecureTransportForbidden { host } => {
                Self::InsecureTransportForbidden { host }
            }
            PolicyError::WebViewProxyUnavailable { host } => Self::WebViewProxyUnavailable { host },
        }
    }
}

impl From<LoginRouteError> for LoginLaunchError {
    fn from(error: LoginRouteError) -> Self {
        match error {
            LoginRouteError::Policy(error) => error.into(),
            LoginRouteError::UnsafeAcknowledgementRequired { host } => {
                Self::UnsafeAcknowledgementRequired { host }
            }
        }
    }
}

impl From<ProbeError> for LoginLaunchError {
    fn from(error: ProbeError) -> Self {
        match error {
            ProbeError::InvalidHost { host } => Self::InvalidHost { host },
            ProbeError::UnsafeAcknowledgementRequired { host } => {
                Self::UnsafeAcknowledgementRequired { host }
            }
            ProbeError::EchUnavailable { host } => Self::EchUnavailable { host },
            ProbeError::CompatibleDirectUnavailable { host } => {
                Self::CompatibleDirectUnavailable { host }
            }
            ProbeError::InsecureTransportForbidden { host } => {
                Self::InsecureTransportForbidden { host }
            }
            ProbeError::WebViewProxyUnavailable { host } => Self::WebViewProxyUnavailable { host },
            ProbeError::WebViewTransportUnavailable { host }
            | ProbeError::ConnectionFailed { host } => Self::ConnectionFailed { host },
            ProbeError::DnsQueryFailed { host } => Self::DnsQueryFailed { host },
            ProbeError::EchConfigUnavailable { host } => Self::EchConfigUnavailable { host },
            ProbeError::EchNotAccepted { host } => Self::EchNotAccepted { host },
            ProbeError::HttpProtocolError { host } => Self::HttpProtocolError { host },
        }
    }
}

impl From<LoginError> for LoginPreparationError {
    fn from(error: LoginError) -> Self {
        match error {
            LoginError::InvalidCallbackTarget => Self::InvalidCallbackConfiguration,
            LoginError::SecureRandomUnavailable => Self::SecureRandomUnavailable,
            _ => Self::StateUnavailable,
        }
    }
}

impl From<LoginError> for LoginLaunchError {
    fn from(error: LoginError) -> Self {
        match error {
            LoginError::AttemptNotPending => Self::AttemptNotPending,
            _ => Self::StateUnavailable,
        }
    }
}

impl From<LoginError> for LoginCompletionError {
    fn from(error: LoginError) -> Self {
        match error {
            LoginError::AttemptNotPending => Self::AttemptNotPending,
            LoginError::MissingState | LoginError::DuplicateState | LoginError::StateMismatch => {
                Self::CallbackStateMismatch
            }
            LoginError::AuthorizationDenied => Self::AuthorizationDenied,
            LoginError::InvalidCallbackTarget
            | LoginError::SecureRandomUnavailable
            | LoginError::InvalidCallbackUrl
            | LoginError::UnexpectedCallbackTarget
            | LoginError::MissingAuthorizationResult
            | LoginError::DuplicateAuthorizationResult
            | LoginError::ConflictingAuthorizationResult
            | LoginError::EmptyAuthorizationCode => Self::InvalidCallback,
        }
    }
}

impl From<OAuthError> for LoginCompletionError {
    fn from(error: OAuthError) -> Self {
        match error {
            OAuthError::ConfigurationUnavailable => Self::OAuthConfigurationUnavailable,
            OAuthError::ClientUnavailable | OAuthError::ClockUnavailable => {
                Self::TokenClientUnavailable
            }
            OAuthError::RequestFailed => Self::TokenRequestFailed,
            OAuthError::Rejected { http_status } => Self::TokenRejected { http_status },
            OAuthError::InvalidResponse => Self::InvalidTokenResponse,
        }
    }
}

impl From<SessionStateError> for LoginCompletionError {
    fn from(_: SessionStateError) -> Self {
        Self::SessionUnavailable
    }
}

impl From<OAuthError> for SessionCommandError {
    fn from(error: OAuthError) -> Self {
        match error {
            OAuthError::ConfigurationUnavailable => Self::OAuthConfigurationUnavailable,
            OAuthError::ClientUnavailable | OAuthError::ClockUnavailable => {
                Self::TokenClientUnavailable
            }
            OAuthError::RequestFailed => Self::TokenRequestFailed,
            OAuthError::Rejected { http_status } => Self::TokenRejected { http_status },
            OAuthError::InvalidResponse => Self::InvalidTokenResponse,
        }
    }
}

impl From<SessionStateError> for SessionCommandError {
    fn from(_: SessionStateError) -> Self {
        Self::SessionUnavailable
    }
}

impl From<SessionStateError> for ApiCommandError {
    fn from(_: SessionStateError) -> Self {
        Self::StateUnavailable
    }
}

impl From<SessionCommandError> for ApiCommandError {
    fn from(error: SessionCommandError) -> Self {
        match error {
            SessionCommandError::OAuthConfigurationUnavailable => {
                Self::OAuthConfigurationUnavailable
            }
            SessionCommandError::SecureStorageUnavailable => Self::SecureStorageUnavailable,
            SessionCommandError::SessionUnavailable => Self::StateUnavailable,
            SessionCommandError::TokenRejected {
                http_status: 400 | 401,
            } => Self::AuthenticationRequired,
            SessionCommandError::TokenRejected { .. }
            | SessionCommandError::TokenClientUnavailable
            | SessionCommandError::TokenTransportUnavailable
            | SessionCommandError::TokenRequestFailed
            | SessionCommandError::InvalidTokenResponse => Self::TokenRefreshFailed,
        }
    }
}

impl From<ApiError> for ApiCommandError {
    fn from(error: ApiError) -> Self {
        match error {
            ApiError::AuthenticationRequired => Self::AuthenticationRequired,
            ApiError::InvalidCursor => Self::InvalidCursor,
            ApiError::InvalidIdentifier => Self::InvalidIdentifier,
            ApiError::InvalidInput => Self::InvalidInput,
            ApiError::InvalidMediaUrl => Self::InvalidMediaUrl,
            ApiError::RequestFailed => Self::RequestFailed,
            ApiError::Rejected { http_status } => Self::UpstreamRejected { http_status },
            ApiError::InvalidResponse => Self::InvalidResponse,
        }
    }
}

impl From<LibraryError> for ApiCommandError {
    fn from(error: LibraryError) -> Self {
        match error {
            LibraryError::EntryNotFound | LibraryError::AssetNotFound => Self::OfflineNotFound,
            LibraryError::InvalidIdentifier
            | LibraryError::InvalidAssetName
            | LibraryError::InvalidContentType
            | LibraryError::InvalidManifest => Self::InvalidInput,
            LibraryError::AssetTooLarge => Self::MediaTooLarge,
            LibraryError::ExportConflict => Self::ExportDestinationUnavailable,
            LibraryError::ExportUnavailable => Self::ExportUnavailable,
            LibraryError::Io => Self::OfflineUnavailable,
        }
    }
}

impl From<CatalogError> for ApiCommandError {
    fn from(error: CatalogError) -> Self {
        match error {
            CatalogError::InvalidInput => Self::InvalidInput,
            CatalogError::CollectionNotFound => Self::LocalCollectionNotFound,
            CatalogError::Conflict => Self::LocalCollectionConflict,
            CatalogError::InvalidDatabase | CatalogError::Database | CatalogError::Io => {
                Self::LocalCatalogUnavailable
            }
        }
    }
}

impl From<HistoryError> for ApiCommandError {
    fn from(error: HistoryError) -> Self {
        match error {
            HistoryError::InvalidInput => Self::InvalidInput,
            HistoryError::InvalidDatabase | HistoryError::Database | HistoryError::Io => {
                Self::BrowsingHistoryUnavailable
            }
        }
    }
}

impl From<CacheError> for ApiCommandError {
    fn from(error: CacheError) -> Self {
        match error {
            CacheError::InvalidSourceKey => Self::InvalidMediaUrl,
            CacheError::AssetTooLarge => Self::MediaTooLarge,
            CacheError::Io => Self::CacheUnavailable,
        }
    }
}

impl From<StorageError> for ApiCommandError {
    fn from(error: StorageError) -> Self {
        match error {
            StorageError::InvalidCacheLimit | StorageError::InvalidWriteSize => Self::InvalidInput,
            StorageError::InsufficientSpace {
                available_bytes,
                required_bytes,
                reserve_bytes,
            } => Self::StorageCapacityExceeded {
                available_bytes,
                required_bytes,
                reserve_bytes,
            },
            StorageError::Io => Self::StorageUnavailable,
            StorageError::StateUnavailable => Self::StateUnavailable,
        }
    }
}

fn detected_capabilities() -> PlatformCapabilities {
    NetworkGateway::capabilities()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(target_os = "android")]
    let builder = builder.plugin(android_login_webview_plugin());
    #[cfg(target_os = "android")]
    let builder = builder.plugin(exports::android_export_plugin());
    #[cfg(target_os = "android")]
    let builder = builder.plugin(updates::android_update_installer_plugin());
    #[cfg(not(target_os = "android"))]
    let builder = builder.plugin(tauri_plugin_dialog::init());
    #[cfg(not(target_os = "android"))]
    let builder = builder.plugin(tauri_plugin_updater::Builder::new().build());

    builder
        .manage(InteractiveLoginState::default())
        .manage(SessionState::default())
        .manage(AuthenticatedDataState::default())
        .manage(MediaCacheState::default())
        .manage(DiagnosticLogState::default())
        .manage(DownloadWorkerState::default())
        .manage(StoragePolicyState::default())
        .manage(ExportState::default())
        .manage(CatalogState::default())
        .manage(HistoryState::default())
        .manage(UpdateManagerState::default())
        .setup(|app| {
            record_diagnostic_event(
                app.handle(),
                DiagnosticEntry::now(
                    LogLevel::Info,
                    LogComponent::Application,
                    LogEvent::ApplicationStarted,
                ),
            );
            downloads::start_download_worker(app.handle().clone());
            updates::start_startup_check(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_app_status,
            updates::get_update_snapshot,
            updates::set_update_preferences,
            updates::check_for_updates,
            updates::download_update,
            updates::install_update,
            updates::cancel_update,
            mark_frontend_ready,
            evaluate_connection,
            probe_connection,
            run_connection_diagnostics,
            prepare_interactive_login,
            open_interactive_login,
            complete_mobile_interactive_login,
            finish_interactive_login_view,
            cancel_interactive_login,
            get_session_status,
            restore_session,
            logout,
            get_recommended_illustrations,
            get_recommended_manga,
            get_recommended_novels,
            get_novel_detail,
            get_novel_content,
            get_novel_series,
            search_novels,
            get_user_novels,
            get_followed_novels,
            get_bookmarked_novels,
            get_ranking_novels,
            get_ugoira_metadata,
            get_illustration_detail,
            get_illustration_series,
            get_related_illustrations,
            get_user_detail,
            get_user_illustrations,
            get_ranking_illustrations,
            get_trending_tags,
            search_illustrations,
            search_users,
            get_followed_users,
            get_followed_illustrations,
            get_bookmarked_illustrations,
            set_illustration_bookmark,
            set_novel_bookmark,
            set_user_follow,
            get_illustration_comments,
            get_comment_replies,
            add_illustration_comment,
            get_novel_comments,
            get_novel_comment_replies,
            add_novel_comment,
            enqueue_download,
            list_download_tasks,
            get_download_queue_stats,
            pause_download_task,
            resume_download_task,
            remove_download_task,
            download_artwork,
            download_novel,
            prepare_ugoira,
            list_offline_entries,
            get_offline_stats,
            get_media_cache_stats,
            clear_media_cache,
            get_storage_status,
            set_media_cache_limit,
            get_export_destination_status,
            select_export_destination,
            clear_export_destination,
            set_auto_export_downloads,
            export_offline_entry,
            get_local_catalog_snapshot,
            create_local_collection,
            rename_local_collection,
            delete_local_collection,
            organize_offline_entry,
            get_browsing_history,
            set_browsing_history_enabled,
            record_browsing_history,
            remove_browsing_history_entry,
            clear_browsing_history,
            get_diagnostic_log_summary,
            export_diagnostic_logs,
            clear_diagnostic_logs,
            clear_local_data,
            read_offline_asset,
            read_offline_text,
            remove_offline_entry,
            fetch_pixiv_thumbnail,
            acknowledge_insecure_media_fallback
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::{
        build_authorization_url, create_login_attempt, oauth_configuration, ApiCommandError,
        AuthenticatedDataState, LoginPreparationError,
    };
    use pixiv_client_auth::LoginStatus;
    use pixiv_client_domain::{
        ConnectionMode, EchRequirement, PlatformCapabilities, TransportRoute,
    };

    #[test]
    fn standard_login_prepares_pkce_for_the_system_webview() {
        let (preparation, attempt) = create_login_attempt(
            ConnectionMode::Standard,
            PlatformCapabilities::default(),
            false,
        )
        .unwrap();

        assert_eq!(preparation.route.transport, TransportRoute::WebViewSystem);
        assert_eq!(
            preparation.route.ech_requirement,
            EchRequirement::NotApplicable
        );
        assert_eq!(preparation.pkce_method, "S256");
        assert_eq!(
            preparation.oauth_configuration_ready,
            oauth_configuration().is_ok()
        );
        assert_eq!(attempt.status(), LoginStatus::Pending);
    }

    #[test]
    fn official_login_url_contains_only_public_pkce_parameters() {
        let (_, attempt) = create_login_attempt(
            ConnectionMode::Standard,
            PlatformCapabilities::default(),
            false,
        )
        .unwrap();

        let url = build_authorization_url(&attempt).unwrap();
        let query = url
            .query_pairs()
            .collect::<std::collections::HashMap<_, _>>();

        assert_eq!(url.host_str(), Some("app-api.pixiv.net"));
        assert_eq!(url.path(), "/web/v1/login");
        assert_eq!(
            query.get("code_challenge_method").map(|v| v.as_ref()),
            Some("S256")
        );
        assert_eq!(
            query.get("client").map(|v| v.as_ref()),
            Some("pixiv-android")
        );
        assert_eq!(query.get("code_challenge").map(|v| v.len()), Some(43));
        assert!(!url.as_str().contains("client_secret"));
    }

    #[test]
    fn ech_login_reports_that_tls_is_managed_by_the_platform_webview() {
        let (preparation, _) =
            create_login_attempt(ConnectionMode::Ech, PlatformCapabilities::default(), false)
                .unwrap();

        assert_eq!(preparation.route.transport, TransportRoute::WebViewSystem);
        assert_eq!(
            preparation.route.ech_requirement,
            EchRequirement::PlatformManaged
        );
    }

    #[test]
    fn compatible_login_requires_acknowledgement_before_checking_the_webview_proxy() {
        let result = create_login_attempt(
            ConnectionMode::Compatible,
            PlatformCapabilities::default(),
            false,
        );

        match result {
            Err(LoginPreparationError::UnsafeAcknowledgementRequired { host }) => {
                assert_eq!(host, "app-api.pixiv.net")
            }
            Err(other) => panic!("unexpected login preparation error: {other:?}"),
            Ok(_) => panic!("compatible login unexpectedly started"),
        }
    }

    #[test]
    fn acknowledged_compatible_login_refuses_to_start_without_a_webview_proxy() {
        let result = create_login_attempt(
            ConnectionMode::Compatible,
            PlatformCapabilities::default(),
            true,
        );

        match result {
            Err(LoginPreparationError::WebViewProxyUnavailable { host }) => {
                assert_eq!(host, "app-api.pixiv.net")
            }
            Err(other) => panic!("unexpected login preparation error: {other:?}"),
            Ok(_) => panic!("compatible login unexpectedly started"),
        }
    }

    #[test]
    fn compatible_login_prepares_a_certificate_verified_webview_proxy() {
        let (preparation, _) = create_login_attempt(
            ConnectionMode::Compatible,
            PlatformCapabilities {
                webview_proxy: true,
                ..PlatformCapabilities::default()
            },
            true,
        )
        .unwrap();

        assert_eq!(preparation.route.transport, TransportRoute::WebViewProxy);
        assert_eq!(
            preparation.oauth_configuration_ready,
            oauth_configuration().is_ok()
        );
    }

    #[test]
    fn ech_media_fallback_acknowledgement_is_scoped_to_one_session_generation() {
        let state = AuthenticatedDataState::default();

        assert!(matches!(
            state.media_mode_for(ConnectionMode::Ech, 7),
            Err(ApiCommandError::UnsafeMediaAcknowledgementRequired)
        ));
        state.acknowledge_insecure_media_fallback(7);
        assert_eq!(
            state.media_mode_for(ConnectionMode::Ech, 7).unwrap(),
            ConnectionMode::Compatible
        );
        assert!(matches!(
            state.media_mode_for(ConnectionMode::Ech, 8),
            Err(ApiCommandError::UnsafeMediaAcknowledgementRequired)
        ));
        assert_eq!(
            state.media_mode_for(ConnectionMode::Standard, 8).unwrap(),
            ConnectionMode::Standard
        );
        assert_eq!(
            state.media_mode_for(ConnectionMode::Compatible, 8).unwrap(),
            ConnectionMode::Compatible
        );
    }
}
