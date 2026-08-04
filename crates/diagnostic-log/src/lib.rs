use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub const DEFAULT_MAX_BYTES: u64 = 256 * 1024;
pub const DEFAULT_RETENTION_SECONDS: u64 = 7 * 24 * 60 * 60;
const LOG_FILE: &str = "diagnostic-log.jsonl";
const MAX_READ_MULTIPLIER: u64 = 4;
const MAX_FUTURE_SKEW_SECONDS: u64 = 5 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogComponent {
    Application,
    Session,
    Login,
    Network,
    Storage,
    Download,
    Privacy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogEvent {
    ApplicationStarted,
    SessionRestoreCompleted,
    SessionRestoreFailed,
    LoginOpened,
    LoginCompleted,
    LoginFailed,
    ConnectionDiagnosticsCompleted,
    ConnectionDiagnosticsFailed,
    MediaCacheCleared,
    DownloadQueued,
    DownloadStarted,
    DownloadPaused,
    DownloadResumed,
    DownloadCompleted,
    DownloadFailed,
    DownloadRecovered,
    DownloadRemoved,
    DownloadQueueCleared,
    LocalDataCleared,
    LocalDataClearIncomplete,
    DiagnosticLogExported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogConnectionMode {
    Standard,
    Ech,
    Compatible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogFailure {
    AuthenticationRequired,
    InvalidInput,
    NetworkUnavailable,
    UpstreamRejected,
    InvalidResponse,
    SecureStorageUnavailable,
    StorageUnavailable,
    EchUnavailable,
    WebviewUnavailable,
    StateUnavailable,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimePlatform {
    Windows,
    Linux,
    Android,
    Other,
}

impl RuntimePlatform {
    fn code(self) -> &'static str {
        match self {
            Self::Windows => "windows",
            Self::Linux => "linux",
            Self::Android => "android",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeArchitecture {
    X86_64,
    Aarch64,
    Armv7,
    Other,
}

impl RuntimeArchitecture {
    fn code(self) -> &'static str {
        match self {
            Self::X86_64 => "x86_64",
            Self::Aarch64 => "aarch64",
            Self::Armv7 => "armv7",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticEntry {
    pub timestamp_unix_seconds: u64,
    pub level: LogLevel,
    pub component: LogComponent,
    pub event: LogEvent,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection_mode: Option<LogConnectionMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<LogFailure>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_count: Option<u32>,
}

impl DiagnosticEntry {
    pub fn now(level: LogLevel, component: LogComponent, event: LogEvent) -> Self {
        Self::at(unix_seconds(), level, component, event)
    }

    pub fn at(
        timestamp_unix_seconds: u64,
        level: LogLevel,
        component: LogComponent,
        event: LogEvent,
    ) -> Self {
        Self {
            timestamp_unix_seconds,
            level,
            component,
            event,
            connection_mode: None,
            failure: None,
            latency_ms: None,
            item_count: None,
        }
    }

    pub fn with_connection_mode(mut self, mode: LogConnectionMode) -> Self {
        self.connection_mode = Some(mode);
        self
    }

    pub fn with_failure(mut self, failure: LogFailure) -> Self {
        self.failure = Some(failure);
        self
    }

    pub fn with_latency_ms(mut self, latency_ms: u64) -> Self {
        self.latency_ms = Some(u32::try_from(latency_ms).unwrap_or(u32::MAX));
        self
    }

    pub fn with_item_count(mut self, item_count: u32) -> Self {
        self.item_count = Some(item_count);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticLogSummary {
    pub entry_count: u32,
    pub retained_bytes: u64,
    pub max_bytes: u64,
    pub retention_days: u32,
    pub oldest_timestamp_unix_seconds: Option<u64>,
    pub newest_timestamp_unix_seconds: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticLogError {
    Io,
    Serialization,
}

impl fmt::Display for DiagnosticLogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Io => "diagnostic log I/O failed",
            Self::Serialization => "diagnostic log serialization failed",
        })
    }
}

impl std::error::Error for DiagnosticLogError {}

pub struct DiagnosticLog {
    root: PathBuf,
    entries: VecDeque<DiagnosticEntry>,
    max_bytes: u64,
    retention_seconds: u64,
}

impl DiagnosticLog {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, DiagnosticLogError> {
        Self::open_with_limits(
            root,
            DEFAULT_MAX_BYTES,
            DEFAULT_RETENTION_SECONDS,
            unix_seconds(),
        )
    }

    fn open_with_limits(
        root: impl Into<PathBuf>,
        max_bytes: u64,
        retention_seconds: u64,
        now: u64,
    ) -> Result<Self, DiagnosticLogError> {
        let root = root.into();
        fs::create_dir_all(&root).map_err(|_| DiagnosticLogError::Io)?;
        let path = root.join(LOG_FILE);
        let mut entries = VecDeque::new();
        if path.exists() {
            let metadata = path.metadata().map_err(|_| DiagnosticLogError::Io)?;
            if metadata.len() <= max_bytes.saturating_mul(MAX_READ_MULTIPLIER) {
                let contents = fs::read_to_string(&path).map_err(|_| DiagnosticLogError::Io)?;
                for line in contents.lines() {
                    if let Ok(entry) = serde_json::from_str::<DiagnosticEntry>(line) {
                        entries.push_back(entry);
                    }
                }
            }
        }
        let mut log = Self {
            root,
            entries,
            max_bytes,
            retention_seconds,
        };
        log.prune(now)?;
        log.persist()?;
        Ok(log)
    }

    pub fn record(&mut self, entry: DiagnosticEntry) -> Result<(), DiagnosticLogError> {
        self.record_at(entry, unix_seconds())
    }

    pub fn summary(&self) -> DiagnosticLogSummary {
        DiagnosticLogSummary {
            entry_count: u32::try_from(self.entries.len()).unwrap_or(u32::MAX),
            retained_bytes: self.serialized_bytes(),
            max_bytes: self.max_bytes,
            retention_days: u32::try_from(self.retention_seconds / (24 * 60 * 60))
                .unwrap_or(u32::MAX),
            oldest_timestamp_unix_seconds: self
                .entries
                .front()
                .map(|entry| entry.timestamp_unix_seconds),
            newest_timestamp_unix_seconds: self
                .entries
                .back()
                .map(|entry| entry.timestamp_unix_seconds),
        }
    }

    pub fn export_text(
        &self,
        platform: RuntimePlatform,
        architecture: RuntimeArchitecture,
    ) -> Result<String, DiagnosticLogError> {
        let mut output = format!(
            "PixNya diagnostics (redacted)\n\
             schema_version=1\n\
             application_version={}\n\
             platform={}\n\
             architecture={}\n\
             generated_at_unix_seconds={}\n\
             retention_days={}\n\
             max_bytes={}\n\
             entry_count={}\n\
             privacy=no tokens, cookies, URLs, account IDs, work IDs, search terms, or response bodies\n---\n",
            env!("CARGO_PKG_VERSION"),
            platform.code(),
            architecture.code(),
            unix_seconds(),
            self.summary().retention_days,
            self.max_bytes,
            self.entries.len(),
        );
        for entry in &self.entries {
            let line =
                serde_json::to_string(entry).map_err(|_| DiagnosticLogError::Serialization)?;
            output.push_str(&line);
            output.push('\n');
        }
        Ok(output)
    }

    pub fn clear(&mut self) -> Result<DiagnosticLogSummary, DiagnosticLogError> {
        let removed = self.summary();
        self.entries.clear();
        self.persist()?;
        Ok(removed)
    }

    fn record_at(&mut self, entry: DiagnosticEntry, now: u64) -> Result<(), DiagnosticLogError> {
        self.entries.push_back(entry);
        self.prune(now)?;
        self.persist()
    }

    fn prune(&mut self, now: u64) -> Result<(), DiagnosticLogError> {
        let cutoff = now.saturating_sub(self.retention_seconds);
        let future_limit = now.saturating_add(MAX_FUTURE_SKEW_SECONDS);
        self.entries.retain(|entry| {
            entry.timestamp_unix_seconds >= cutoff && entry.timestamp_unix_seconds <= future_limit
        });
        while self.serialized_bytes() > self.max_bytes {
            if self.entries.pop_front().is_none() {
                break;
            }
        }
        Ok(())
    }

    fn serialized_bytes(&self) -> u64 {
        self.entries
            .iter()
            .map(|entry| {
                serde_json::to_vec(entry)
                    .map(|bytes| u64::try_from(bytes.len()).unwrap_or(u64::MAX) + 1)
                    .unwrap_or(u64::MAX)
            })
            .sum()
    }

    fn persist(&self) -> Result<(), DiagnosticLogError> {
        fs::create_dir_all(&self.root).map_err(|_| DiagnosticLogError::Io)?;
        let capacity = usize::try_from(self.serialized_bytes())
            .unwrap_or(1024 * 1024)
            .min(1024 * 1024);
        let mut bytes = Vec::with_capacity(capacity);
        for entry in &self.entries {
            serde_json::to_writer(&mut bytes, entry)
                .map_err(|_| DiagnosticLogError::Serialization)?;
            bytes.push(b'\n');
        }
        fs::write(self.root.join(LOG_FILE), bytes).map_err(|_| DiagnosticLogError::Io)
    }
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQUENCE: AtomicU64 = AtomicU64::new(1);

    #[test]
    fn export_contains_only_the_typed_redacted_contract() {
        let root = test_root("redacted");
        let mut log = DiagnosticLog::open_with_limits(&root, 4096, 1000, 1000).unwrap();
        log.record_at(
            DiagnosticEntry::at(
                1000,
                LogLevel::Error,
                LogComponent::Network,
                LogEvent::ConnectionDiagnosticsFailed,
            )
            .with_connection_mode(LogConnectionMode::Ech)
            .with_failure(LogFailure::EchUnavailable)
            .with_latency_ms(u64::MAX),
            1000,
        )
        .unwrap();

        let export = log
            .export_text(RuntimePlatform::Android, RuntimeArchitecture::Aarch64)
            .unwrap();
        assert!(export.contains("connection_diagnostics_failed"));
        assert!(export.contains("ech_unavailable"));
        assert!(export.contains("\"latencyMs\":4294967295"));
        for forbidden in [
            "access_token",
            "refresh_token",
            "Cookie:",
            "https://",
            "search_word",
        ] {
            assert!(!export.contains(forbidden), "leaked {forbidden}");
        }
        cleanup(&root);
    }

    #[test]
    fn retention_and_capacity_remove_oldest_entries() {
        let root = test_root("limits");
        let mut log = DiagnosticLog::open_with_limits(&root, 430, 100, 1000).unwrap();
        log.record_at(
            DiagnosticEntry::at(
                899,
                LogLevel::Info,
                LogComponent::Application,
                LogEvent::ApplicationStarted,
            ),
            1000,
        )
        .unwrap();
        assert_eq!(log.summary().entry_count, 0);

        for timestamp in 1000..1010 {
            log.record_at(
                DiagnosticEntry::at(
                    timestamp,
                    LogLevel::Info,
                    LogComponent::Storage,
                    LogEvent::MediaCacheCleared,
                ),
                timestamp,
            )
            .unwrap();
        }
        let summary = log.summary();
        assert!(summary.entry_count < 10);
        assert!(summary.retained_bytes <= 430);
        assert!(summary.oldest_timestamp_unix_seconds.unwrap() > 1000);
        cleanup(&root);
    }

    #[test]
    fn download_events_remain_bounded_and_identifier_free() {
        let root = test_root("download-event");
        let mut log = DiagnosticLog::open_with_limits(&root, 4096, 1000, 1000).unwrap();
        log.record_at(
            DiagnosticEntry::at(
                1000,
                LogLevel::Error,
                LogComponent::Download,
                LogEvent::DownloadFailed,
            )
            .with_failure(LogFailure::NetworkUnavailable)
            .with_item_count(1),
            1000,
        )
        .unwrap();

        let export = log
            .export_text(RuntimePlatform::Windows, RuntimeArchitecture::X86_64)
            .unwrap();
        assert!(export.contains("download_failed"));
        assert!(export.contains("network_unavailable"));
        assert!(export.contains("\"itemCount\":1"));
        assert!(!export.contains("resourceId"));
        assert!(!export.contains("taskId"));
        cleanup(&root);
    }

    #[test]
    fn clear_never_touches_adjacent_application_data() {
        let parent = test_root("clear-boundary");
        let log_root = parent.join("diagnostics");
        let adjacent = parent.join("offline-library");
        fs::create_dir_all(&adjacent).unwrap();
        fs::write(adjacent.join("keep.bin"), b"user download").unwrap();
        let mut log = DiagnosticLog::open_with_limits(&log_root, 4096, 1000, 1000).unwrap();
        log.record_at(
            DiagnosticEntry::at(
                1000,
                LogLevel::Info,
                LogComponent::Application,
                LogEvent::ApplicationStarted,
            ),
            1000,
        )
        .unwrap();

        let removed = log.clear().unwrap();
        assert_eq!(removed.entry_count, 1);
        assert_eq!(log.summary().entry_count, 0);
        assert_eq!(
            fs::read(adjacent.join("keep.bin")).unwrap(),
            b"user download"
        );
        cleanup(&parent);
    }

    fn test_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "pixiv-client-diagnostic-log-{label}-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn cleanup(path: &std::path::Path) {
        let _ = fs::remove_dir_all(path);
    }
}
