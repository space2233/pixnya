use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const SCHEMA_VERSION: u32 = 1;
const HISTORY_LIMIT: usize = 500;
const MAX_TITLE_BYTES: usize = 512;
const MAX_SUBTITLE_BYTES: usize = 256;
const MAX_THUMBNAIL_BYTES: usize = 2048;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoryKind {
    Artwork,
    Novel,
    User,
}

impl HistoryKind {
    fn code(self) -> &'static str {
        match self {
            Self::Artwork => "artwork",
            Self::Novel => "novel",
            Self::User => "user",
        }
    }

    fn parse(value: &str) -> Result<Self, HistoryError> {
        match value {
            "artwork" => Ok(Self::Artwork),
            "novel" => Ok(Self::Novel),
            "user" => Ok(Self::User),
            _ => Err(HistoryError::InvalidDatabase),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryRecord {
    pub kind: HistoryKind,
    pub resource_id: String,
    pub title: String,
    pub subtitle: String,
    pub thumbnail_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub kind: HistoryKind,
    pub resource_id: String,
    pub title: String,
    pub subtitle: String,
    pub thumbnail_url: Option<String>,
    pub viewed_at_unix_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistorySnapshot {
    pub enabled: bool,
    pub limit: u32,
    pub entries: Vec<HistoryEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryClearStats {
    pub entries_removed: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryError {
    InvalidInput,
    InvalidDatabase,
    Database,
    Io,
}

impl fmt::Display for HistoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidInput => "invalid browsing history input",
            Self::InvalidDatabase => "browsing history database is invalid or too new",
            Self::Database => "browsing history database operation failed",
            Self::Io => "browsing history filesystem operation failed",
        })
    }
}

impl std::error::Error for HistoryError {}

#[derive(Clone)]
pub struct LocalHistory {
    path: PathBuf,
}

impl LocalHistory {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, HistoryError> {
        let path = path.into();
        let parent = path.parent().ok_or(HistoryError::InvalidInput)?;
        fs::create_dir_all(parent).map_err(|_| HistoryError::Io)?;
        let history = Self { path };
        let mut connection = history.connection()?;
        migrate(&mut connection)?;
        Ok(history)
    }

    pub fn snapshot(&self) -> Result<HistorySnapshot, HistoryError> {
        let connection = self.connection()?;
        let enabled = read_enabled(&connection)?;
        let mut statement = connection
            .prepare(
                "SELECT kind, resource_id, title, subtitle, thumbnail_url, viewed_at
                 FROM history_entries ORDER BY view_order DESC LIMIT ?1",
            )
            .map_err(|_| HistoryError::Database)?;
        let rows = statement
            .query_map([HISTORY_LIMIT as u32], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            })
            .map_err(|_| HistoryError::Database)?;
        let mut entries = Vec::new();
        for row in rows {
            let (kind, resource_id, title, subtitle, thumbnail_url, viewed_at) =
                row.map_err(|_| HistoryError::Database)?;
            let viewed_at_unix_seconds =
                u64::try_from(viewed_at).map_err(|_| HistoryError::InvalidDatabase)?;
            entries.push(HistoryEntry {
                kind: HistoryKind::parse(&kind)?,
                resource_id,
                title,
                subtitle,
                thumbnail_url,
                viewed_at_unix_seconds,
            });
        }
        Ok(HistorySnapshot {
            enabled,
            limit: HISTORY_LIMIT as u32,
            entries,
        })
    }

    pub fn set_enabled(&self, enabled: bool) -> Result<HistorySnapshot, HistoryError> {
        let connection = self.connection()?;
        connection
            .execute(
                "UPDATE history_settings SET enabled = ?1 WHERE id = 1",
                [i64::from(enabled)],
            )
            .map_err(|_| HistoryError::Database)?;
        self.snapshot()
    }

    pub fn record(&self, record: HistoryRecord) -> Result<bool, HistoryError> {
        let record = normalize_record(record)?;
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| HistoryError::Database)?;
        if !read_enabled(&transaction)? {
            transaction.commit().map_err(|_| HistoryError::Database)?;
            return Ok(false);
        }
        let next_order: i64 = transaction
            .query_row(
                "SELECT COALESCE(MAX(view_order), 0) + 1 FROM history_entries",
                [],
                |row| row.get(0),
            )
            .map_err(|_| HistoryError::Database)?;
        transaction
            .execute(
                "INSERT INTO history_entries (
                    kind, resource_id, title, subtitle, thumbnail_url, viewed_at, view_order
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(kind, resource_id) DO UPDATE SET
                    title = excluded.title,
                    subtitle = excluded.subtitle,
                    thumbnail_url = excluded.thumbnail_url,
                    viewed_at = excluded.viewed_at,
                    view_order = excluded.view_order",
                params![
                    record.kind.code(),
                    record.resource_id,
                    record.title,
                    record.subtitle,
                    record.thumbnail_url,
                    i64::try_from(unix_seconds()).unwrap_or(i64::MAX),
                    next_order,
                ],
            )
            .map_err(|_| HistoryError::Database)?;
        transaction
            .execute(
                "DELETE FROM history_entries WHERE rowid IN (
                    SELECT rowid FROM history_entries
                    ORDER BY view_order DESC LIMIT -1 OFFSET ?1
                 )",
                [HISTORY_LIMIT as u32],
            )
            .map_err(|_| HistoryError::Database)?;
        transaction.commit().map_err(|_| HistoryError::Database)?;
        Ok(true)
    }

    pub fn remove(&self, kind: HistoryKind, resource_id: &str) -> Result<bool, HistoryError> {
        let resource_id = normalized_resource_id(resource_id)?;
        self.connection()?
            .execute(
                "DELETE FROM history_entries WHERE kind = ?1 AND resource_id = ?2",
                params![kind.code(), resource_id],
            )
            .map(|count| count > 0)
            .map_err(|_| HistoryError::Database)
    }

    pub fn clear(&self) -> Result<HistoryClearStats, HistoryError> {
        let connection = self.connection()?;
        let count: u32 = connection
            .query_row("SELECT COUNT(*) FROM history_entries", [], |row| row.get(0))
            .map_err(|_| HistoryError::Database)?;
        connection
            .execute("DELETE FROM history_entries", [])
            .map_err(|_| HistoryError::Database)?;
        Ok(HistoryClearStats {
            entries_removed: count,
        })
    }

    fn connection(&self) -> Result<Connection, HistoryError> {
        let connection = Connection::open(&self.path).map_err(|_| HistoryError::Database)?;
        connection
            .busy_timeout(Duration::from_secs(2))
            .map_err(|_| HistoryError::Database)?;
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .map_err(|_| HistoryError::Database)?;
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .map_err(|_| HistoryError::Database)?;
        Ok(connection)
    }
}

fn migrate(connection: &mut Connection) -> Result<(), HistoryError> {
    let version: u32 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|_| HistoryError::Database)?;
    if version > SCHEMA_VERSION {
        return Err(HistoryError::InvalidDatabase);
    }
    if version == 0 {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| HistoryError::Database)?;
        transaction
            .execute_batch(
                "CREATE TABLE history_settings (
                    id INTEGER PRIMARY KEY CHECK (id = 1),
                    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1))
                 );
                 INSERT INTO history_settings (id, enabled) VALUES (1, 1);
                 CREATE TABLE history_entries (
                    kind TEXT NOT NULL CHECK (kind IN ('artwork', 'novel', 'user')),
                    resource_id TEXT NOT NULL,
                    title TEXT NOT NULL,
                    subtitle TEXT NOT NULL,
                    thumbnail_url TEXT,
                    viewed_at INTEGER NOT NULL,
                    view_order INTEGER NOT NULL UNIQUE,
                    PRIMARY KEY (kind, resource_id)
                 );
                 CREATE INDEX history_entries_order_idx ON history_entries(view_order DESC);",
            )
            .map_err(|_| HistoryError::Database)?;
        transaction
            .pragma_update(None, "user_version", SCHEMA_VERSION)
            .map_err(|_| HistoryError::Database)?;
        transaction.commit().map_err(|_| HistoryError::Database)?;
    }
    let enabled: Option<i64> = connection
        .query_row(
            "SELECT enabled FROM history_settings WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| HistoryError::Database)?;
    if !matches!(enabled, Some(0 | 1)) {
        return Err(HistoryError::InvalidDatabase);
    }
    Ok(())
}

fn read_enabled(connection: &Connection) -> Result<bool, HistoryError> {
    connection
        .query_row(
            "SELECT enabled FROM history_settings WHERE id = 1",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map(|enabled| enabled == 1)
        .map_err(|_| HistoryError::Database)
}

fn normalize_record(record: HistoryRecord) -> Result<HistoryRecord, HistoryError> {
    Ok(HistoryRecord {
        kind: record.kind,
        resource_id: normalized_resource_id(&record.resource_id)?,
        title: normalized_text(&record.title, MAX_TITLE_BYTES)?,
        subtitle: normalized_text(&record.subtitle, MAX_SUBTITLE_BYTES)?,
        thumbnail_url: normalized_thumbnail(record.thumbnail_url)?,
    })
}

fn normalized_resource_id(candidate: &str) -> Result<String, HistoryError> {
    let value = candidate
        .parse::<u64>()
        .map_err(|_| HistoryError::InvalidInput)?;
    (value > 0)
        .then(|| value.to_string())
        .ok_or(HistoryError::InvalidInput)
}

fn normalized_text(candidate: &str, maximum: usize) -> Result<String, HistoryError> {
    let value = candidate.trim();
    if value.is_empty()
        || value.len() > maximum
        || value.chars().any(|character| character.is_control())
    {
        return Err(HistoryError::InvalidInput);
    }
    Ok(value.to_owned())
}

fn normalized_thumbnail(candidate: Option<String>) -> Result<Option<String>, HistoryError> {
    let Some(value) = candidate else {
        return Ok(None);
    };
    let value = value.trim();
    if value.len() > MAX_THUMBNAIL_BYTES
        || !(value.starts_with("https://i.pximg.net/") || value.starts_with("https://s.pximg.net/"))
    {
        return Err(HistoryError::InvalidInput);
    }
    Ok(Some(value.to_owned()))
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

#[cfg(test)]
mod tests {
    use super::{HistoryError, HistoryKind, HistoryRecord, LocalHistory, HISTORY_LIMIT};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_history(name: &str) -> (std::path::PathBuf, LocalHistory) {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("pixiv-client-history-{name}-{nonce}"));
        let history = LocalHistory::open(root.join("history.sqlite3")).unwrap();
        (root, history)
    }

    fn record(kind: HistoryKind, id: u64) -> HistoryRecord {
        HistoryRecord {
            kind,
            resource_id: id.to_string(),
            title: format!("作品 {id}"),
            subtitle: "作者".to_owned(),
            thumbnail_url: Some("https://i.pximg.net/test.jpg".to_owned()),
        }
    }

    #[test]
    fn records_updates_orders_and_removes_entries() {
        let (root, history) = test_history("record");
        history.record(record(HistoryKind::Artwork, 1)).unwrap();
        history.record(record(HistoryKind::Novel, 2)).unwrap();
        let mut updated = record(HistoryKind::Artwork, 1);
        updated.title = "更新标题".to_owned();
        history.record(updated).unwrap();
        let snapshot = history.snapshot().unwrap();
        assert_eq!(snapshot.entries.len(), 2);
        assert_eq!(snapshot.entries[0].resource_id, "1");
        assert_eq!(snapshot.entries[0].title, "更新标题");
        assert!(history.remove(HistoryKind::Novel, "2").unwrap());
        assert!(!history.remove(HistoryKind::Novel, "2").unwrap());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn disabling_history_preserves_existing_rows_and_rejects_new_records() {
        let (root, history) = test_history("disabled");
        history.record(record(HistoryKind::Artwork, 1)).unwrap();
        assert!(!history.set_enabled(false).unwrap().enabled);
        assert!(!history.record(record(HistoryKind::Artwork, 2)).unwrap());
        let snapshot = history.snapshot().unwrap();
        assert_eq!(snapshot.entries.len(), 1);
        assert_eq!(snapshot.entries[0].resource_id, "1");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn enforces_the_bounded_most_recent_history_limit() {
        let (root, history) = test_history("limit");
        for id in 1..=(HISTORY_LIMIT as u64 + 7) {
            history.record(record(HistoryKind::Artwork, id)).unwrap();
        }
        let snapshot = history.snapshot().unwrap();
        assert_eq!(snapshot.entries.len(), HISTORY_LIMIT);
        assert_eq!(
            snapshot.entries[0].resource_id,
            (HISTORY_LIMIT + 7).to_string()
        );
        assert_eq!(snapshot.entries.last().unwrap().resource_id, "8");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_invalid_ids_text_and_non_pixiv_thumbnails() {
        let (root, history) = test_history("validation");
        let mut invalid = record(HistoryKind::User, 1);
        invalid.resource_id = "../1".to_owned();
        assert_eq!(history.record(invalid), Err(HistoryError::InvalidInput));
        let mut invalid = record(HistoryKind::User, 1);
        invalid.title = "bad\ntext".to_owned();
        assert_eq!(history.record(invalid), Err(HistoryError::InvalidInput));
        let mut invalid = record(HistoryKind::User, 1);
        invalid.thumbnail_url = Some("https://example.com/tracker".to_owned());
        assert_eq!(history.record(invalid), Err(HistoryError::InvalidInput));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn clear_reports_rows_but_keeps_the_enabled_preference() {
        let (root, history) = test_history("clear");
        history.record(record(HistoryKind::Artwork, 1)).unwrap();
        history.record(record(HistoryKind::Novel, 2)).unwrap();
        history.set_enabled(false).unwrap();
        assert_eq!(history.clear().unwrap().entries_removed, 2);
        let snapshot = history.snapshot().unwrap();
        assert!(!snapshot.enabled);
        assert!(snapshot.entries.is_empty());
        fs::remove_dir_all(root).unwrap();
    }
}
