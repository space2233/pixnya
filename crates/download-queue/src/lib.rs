use rusqlite::{params, Connection, OptionalExtension, Row, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const SCHEMA_VERSION: u32 = 1;
const MAX_TITLE_BYTES: usize = 512;
const MAX_AUTHOR_BYTES: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadKind {
    Artwork,
    Novel,
    Ugoira,
}

impl DownloadKind {
    fn code(self) -> &'static str {
        match self {
            Self::Artwork => "artwork",
            Self::Novel => "novel",
            Self::Ugoira => "ugoira",
        }
    }

    fn parse(value: &str) -> Result<Self, QueueError> {
        match value {
            "artwork" => Ok(Self::Artwork),
            "novel" => Ok(Self::Novel),
            "ugoira" => Ok(Self::Ugoira),
            _ => Err(QueueError::InvalidDatabase),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadState {
    Queued,
    Running,
    Paused,
    Failed,
    Completed,
}

impl DownloadState {
    fn parse(value: &str) -> Result<Self, QueueError> {
        match value {
            "queued" => Ok(Self::Queued),
            "running" => Ok(Self::Running),
            "paused" => Ok(Self::Paused),
            "failed" => Ok(Self::Failed),
            "completed" => Ok(Self::Completed),
            _ => Err(QueueError::InvalidDatabase),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadFailure {
    Authentication,
    Network,
    InvalidResponse,
    Storage,
    Interrupted,
}

impl DownloadFailure {
    fn code(self) -> &'static str {
        match self {
            Self::Authentication => "authentication",
            Self::Network => "network",
            Self::InvalidResponse => "invalid_response",
            Self::Storage => "storage",
            Self::Interrupted => "interrupted",
        }
    }

    fn parse(value: &str) -> Result<Self, QueueError> {
        match value {
            "authentication" => Ok(Self::Authentication),
            "network" => Ok(Self::Network),
            "invalid_response" => Ok(Self::InvalidResponse),
            "storage" => Ok(Self::Storage),
            "interrupted" => Ok(Self::Interrupted),
            _ => Err(QueueError::InvalidDatabase),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewDownloadTask {
    pub kind: DownloadKind,
    pub resource_id: String,
    pub title: Option<String>,
    pub author: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadTask {
    pub id: i64,
    pub kind: DownloadKind,
    pub resource_id: String,
    pub title: Option<String>,
    pub author: Option<String>,
    pub state: DownloadState,
    pub completed_items: u32,
    pub total_items: u32,
    pub downloaded_bytes: u64,
    pub attempt_count: u32,
    pub failure: Option<DownloadFailure>,
    pub created_at_unix_seconds: u64,
    pub updated_at_unix_seconds: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadQueueStats {
    pub task_count: u32,
    pub active_count: u32,
    pub failed_count: u32,
    pub completed_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueError {
    InvalidInput,
    TaskNotFound,
    InvalidTransition,
    InvalidDatabase,
    Io,
    Database,
}

impl fmt::Display for QueueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidInput => "invalid download queue input",
            Self::TaskNotFound => "download task was not found",
            Self::InvalidTransition => "invalid download task transition",
            Self::InvalidDatabase => "download queue database is invalid",
            Self::Io => "download queue filesystem operation failed",
            Self::Database => "download queue database operation failed",
        })
    }
}

impl std::error::Error for QueueError {}

#[derive(Clone)]
pub struct DownloadQueue {
    path: PathBuf,
}

impl DownloadQueue {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, QueueError> {
        let path = path.into();
        let parent = path.parent().ok_or(QueueError::InvalidInput)?;
        fs::create_dir_all(parent).map_err(|_| QueueError::Io)?;
        let queue = Self { path };
        let mut connection = queue.connection()?;
        migrate(&mut connection)?;
        Ok(queue)
    }

    pub fn schema_version(&self) -> Result<u32, QueueError> {
        let connection = self.connection()?;
        connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(|_| QueueError::Database)
    }

    pub fn enqueue(&self, task: NewDownloadTask) -> Result<DownloadTask, QueueError> {
        let resource_id = normalized_resource_id(&task.resource_id)?;
        let title = normalized_optional_text(task.title, MAX_TITLE_BYTES)?;
        let author = normalized_optional_text(task.author, MAX_AUTHOR_BYTES)?;
        let now = unix_seconds();
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| QueueError::Database)?;
        let existing_id = transaction
            .query_row(
                "SELECT id FROM download_tasks WHERE kind = ?1 AND resource_id = ?2",
                params![task.kind.code(), resource_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(|_| QueueError::Database)?;
        let id = if let Some(id) = existing_id {
            let state = transaction
                .query_row(
                    "SELECT state FROM download_tasks WHERE id = ?1",
                    [id],
                    |row| row.get::<_, String>(0),
                )
                .map_err(|_| QueueError::Database)?;
            let state = DownloadState::parse(&state)?;
            if matches!(state, DownloadState::Failed | DownloadState::Completed) {
                transaction
                    .execute(
                        "UPDATE download_tasks
                         SET title = ?2, author = ?3, state = 'queued', completed_items = 0,
                             total_items = 0, downloaded_bytes = 0, attempt_count = 0,
                             failure = NULL, updated_at = ?4
                         WHERE id = ?1",
                        params![id, title, author, to_sql_u64(now)],
                    )
                    .map_err(|_| QueueError::Database)?;
            } else {
                transaction
                    .execute(
                        "UPDATE download_tasks
                         SET title = COALESCE(?2, title), author = COALESCE(?3, author), updated_at = ?4
                         WHERE id = ?1",
                        params![id, title, author, to_sql_u64(now)],
                    )
                    .map_err(|_| QueueError::Database)?;
            }
            id
        } else {
            transaction
                .execute(
                    "INSERT INTO download_tasks
                     (kind, resource_id, title, author, state, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, 'queued', ?5, ?5)",
                    params![
                        task.kind.code(),
                        resource_id,
                        title,
                        author,
                        to_sql_u64(now)
                    ],
                )
                .map_err(|_| QueueError::Database)?;
            transaction.last_insert_rowid()
        };
        let task = select_task(&transaction, id)?;
        transaction.commit().map_err(|_| QueueError::Database)?;
        Ok(task)
    }

    pub fn list(&self) -> Result<Vec<DownloadTask>, QueueError> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                "SELECT id, kind, resource_id, title, author, state, completed_items,
                        total_items, downloaded_bytes, attempt_count, failure, created_at, updated_at
                 FROM download_tasks
                 ORDER BY
                    CASE state
                      WHEN 'running' THEN 0 WHEN 'queued' THEN 1 WHEN 'paused' THEN 2
                      WHEN 'failed' THEN 3 ELSE 4 END,
                    created_at ASC, id ASC",
            )
            .map_err(|_| QueueError::Database)?;
        let rows = statement
            .query_map([], task_from_row)
            .map_err(|_| QueueError::Database)?;
        rows.map(|row| row.map_err(|_| QueueError::InvalidDatabase))
            .collect()
    }

    pub fn get(&self, id: i64) -> Result<DownloadTask, QueueError> {
        if id <= 0 {
            return Err(QueueError::InvalidInput);
        }
        let connection = self.connection()?;
        select_task(&connection, id)
    }

    pub fn stats(&self) -> Result<DownloadQueueStats, QueueError> {
        let connection = self.connection()?;
        connection
            .query_row(
                "SELECT COUNT(*),
                        COALESCE(SUM(CASE WHEN state IN ('queued', 'running', 'paused') THEN 1 ELSE 0 END), 0),
                        COALESCE(SUM(CASE WHEN state = 'failed' THEN 1 ELSE 0 END), 0),
                        COALESCE(SUM(CASE WHEN state = 'completed' THEN 1 ELSE 0 END), 0)
                 FROM download_tasks",
                [],
                |row| {
                    Ok(DownloadQueueStats {
                        task_count: sql_u32(row.get::<_, i64>(0)?)?,
                        active_count: sql_u32(row.get::<_, i64>(1)?)?,
                        failed_count: sql_u32(row.get::<_, i64>(2)?)?,
                        completed_count: sql_u32(row.get::<_, i64>(3)?)?,
                    })
                },
            )
            .map_err(|_| QueueError::Database)
    }

    pub fn recover_interrupted(&self) -> Result<u32, QueueError> {
        let connection = self.connection()?;
        let count = connection
            .execute(
                "UPDATE download_tasks
                 SET state = 'queued', failure = 'interrupted', updated_at = ?1
                 WHERE state = 'running'",
                [to_sql_u64(unix_seconds())],
            )
            .map_err(|_| QueueError::Database)?;
        u32::try_from(count).map_err(|_| QueueError::InvalidDatabase)
    }

    pub fn claim_next(&self) -> Result<Option<DownloadTask>, QueueError> {
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| QueueError::Database)?;
        let id = transaction
            .query_row(
                "SELECT id FROM download_tasks WHERE state = 'queued' ORDER BY created_at, id LIMIT 1",
                [],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(|_| QueueError::Database)?;
        let Some(id) = id else {
            transaction.commit().map_err(|_| QueueError::Database)?;
            return Ok(None);
        };
        let changed = transaction
            .execute(
                "UPDATE download_tasks
                 SET state = 'running', attempt_count = attempt_count + 1,
                     failure = NULL, updated_at = ?2
                 WHERE id = ?1 AND state = 'queued'",
                params![id, to_sql_u64(unix_seconds())],
            )
            .map_err(|_| QueueError::Database)?;
        if changed != 1 {
            return Err(QueueError::InvalidTransition);
        }
        let task = select_task(&transaction, id)?;
        transaction.commit().map_err(|_| QueueError::Database)?;
        Ok(Some(task))
    }

    pub fn update_metadata(
        &self,
        id: i64,
        title: Option<String>,
        author: Option<String>,
    ) -> Result<DownloadTask, QueueError> {
        let title = normalized_optional_text(title, MAX_TITLE_BYTES)?;
        let author = normalized_optional_text(author, MAX_AUTHOR_BYTES)?;
        self.update_task(
            id,
            "UPDATE download_tasks
             SET title = COALESCE(?2, title), author = COALESCE(?3, author), updated_at = ?4
             WHERE id = ?1 AND state IN ('running', 'paused')",
            params![id, title, author, to_sql_u64(unix_seconds())],
        )
    }

    pub fn update_progress(
        &self,
        id: i64,
        completed_items: u32,
        total_items: u32,
        downloaded_bytes: u64,
    ) -> Result<DownloadTask, QueueError> {
        if total_items == 0 || completed_items > total_items {
            return Err(QueueError::InvalidInput);
        }
        self.update_task(
            id,
            "UPDATE download_tasks
             SET completed_items = ?2, total_items = ?3, downloaded_bytes = ?4, updated_at = ?5
             WHERE id = ?1 AND state = 'running'",
            params![
                id,
                i64::from(completed_items),
                i64::from(total_items),
                to_sql_u64(downloaded_bytes),
                to_sql_u64(unix_seconds())
            ],
        )
    }

    pub fn pause(&self, id: i64) -> Result<DownloadTask, QueueError> {
        let current = self.get(id)?;
        if current.state == DownloadState::Paused {
            return Ok(current);
        }
        self.update_task(
            id,
            "UPDATE download_tasks SET state = 'paused', updated_at = ?2
             WHERE id = ?1 AND state IN ('queued', 'running')",
            params![id, to_sql_u64(unix_seconds())],
        )
    }

    pub fn resume(&self, id: i64) -> Result<DownloadTask, QueueError> {
        self.update_task(
            id,
            "UPDATE download_tasks
             SET state = 'queued', completed_items = 0, total_items = 0,
                 downloaded_bytes = 0, failure = NULL, updated_at = ?2
             WHERE id = ?1 AND state IN ('paused', 'failed')",
            params![id, to_sql_u64(unix_seconds())],
        )
    }

    pub fn mark_failed(
        &self,
        id: i64,
        failure: DownloadFailure,
    ) -> Result<DownloadTask, QueueError> {
        let current = self.get(id)?;
        if current.state == DownloadState::Paused {
            return Ok(current);
        }
        self.update_task(
            id,
            "UPDATE download_tasks SET state = 'failed', failure = ?2, updated_at = ?3
             WHERE id = ?1 AND state = 'running'",
            params![id, failure.code(), to_sql_u64(unix_seconds())],
        )
    }

    pub fn mark_completed(
        &self,
        id: i64,
        completed_items: u32,
        downloaded_bytes: u64,
    ) -> Result<DownloadTask, QueueError> {
        if completed_items == 0 {
            return Err(QueueError::InvalidInput);
        }
        self.update_task(
            id,
            "UPDATE download_tasks
             SET state = 'completed', completed_items = ?2, total_items = ?2,
                 downloaded_bytes = ?3, failure = NULL, updated_at = ?4
             WHERE id = ?1 AND state IN ('running', 'paused')",
            params![
                id,
                i64::from(completed_items),
                to_sql_u64(downloaded_bytes),
                to_sql_u64(unix_seconds())
            ],
        )
    }

    pub fn remove(&self, id: i64) -> Result<bool, QueueError> {
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| QueueError::Database)?;
        let state = transaction
            .query_row(
                "SELECT state FROM download_tasks WHERE id = ?1",
                [id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|_| QueueError::Database)?;
        let Some(state) = state else {
            transaction.commit().map_err(|_| QueueError::Database)?;
            return Ok(false);
        };
        if DownloadState::parse(&state)? == DownloadState::Running {
            return Err(QueueError::InvalidTransition);
        }
        let removed = transaction
            .execute("DELETE FROM download_tasks WHERE id = ?1", [id])
            .map_err(|_| QueueError::Database)?;
        transaction.commit().map_err(|_| QueueError::Database)?;
        Ok(removed == 1)
    }

    pub fn clear(&self) -> Result<DownloadQueueStats, QueueError> {
        let removed = self.stats()?;
        let connection = self.connection()?;
        connection
            .execute("DELETE FROM download_tasks", [])
            .map_err(|_| QueueError::Database)?;
        Ok(removed)
    }

    fn update_task<P: rusqlite::Params>(
        &self,
        id: i64,
        sql: &str,
        parameters: P,
    ) -> Result<DownloadTask, QueueError> {
        if id <= 0 {
            return Err(QueueError::InvalidInput);
        }
        let connection = self.connection()?;
        let changed = connection
            .execute(sql, parameters)
            .map_err(|_| QueueError::Database)?;
        if changed != 1 {
            return if self.get(id).is_ok() {
                Err(QueueError::InvalidTransition)
            } else {
                Err(QueueError::TaskNotFound)
            };
        }
        self.get(id)
    }

    fn connection(&self) -> Result<Connection, QueueError> {
        let connection = Connection::open(&self.path).map_err(|_| QueueError::Database)?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(|_| QueueError::Database)?;
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .map_err(|_| QueueError::Database)?;
        Ok(connection)
    }
}

fn migrate(connection: &mut Connection) -> Result<(), QueueError> {
    let version: u32 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|_| QueueError::Database)?;
    if version > SCHEMA_VERSION {
        return Err(QueueError::InvalidDatabase);
    }
    if version == 0 {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| QueueError::Database)?;
        transaction
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS schema_migrations (
                    version INTEGER PRIMARY KEY NOT NULL,
                    applied_at INTEGER NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS download_tasks (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    kind TEXT NOT NULL CHECK (kind IN ('artwork', 'novel', 'ugoira')),
                    resource_id TEXT NOT NULL,
                    title TEXT,
                    author TEXT,
                    state TEXT NOT NULL CHECK (state IN ('queued', 'running', 'paused', 'failed', 'completed')),
                    completed_items INTEGER NOT NULL DEFAULT 0 CHECK (completed_items >= 0),
                    total_items INTEGER NOT NULL DEFAULT 0 CHECK (total_items >= 0),
                    downloaded_bytes INTEGER NOT NULL DEFAULT 0 CHECK (downloaded_bytes >= 0),
                    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
                    failure TEXT CHECK (failure IS NULL OR failure IN
                        ('authentication', 'network', 'invalid_response', 'storage', 'interrupted')),
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL,
                    UNIQUE(kind, resource_id)
                 );
                 CREATE INDEX IF NOT EXISTS download_tasks_state_order
                 ON download_tasks(state, created_at, id);",
            )
            .map_err(|_| QueueError::Database)?;
        transaction
            .execute(
                "INSERT OR IGNORE INTO schema_migrations(version, applied_at) VALUES (?1, ?2)",
                params![i64::from(SCHEMA_VERSION), to_sql_u64(unix_seconds())],
            )
            .map_err(|_| QueueError::Database)?;
        transaction
            .pragma_update(None, "user_version", SCHEMA_VERSION)
            .map_err(|_| QueueError::Database)?;
        transaction.commit().map_err(|_| QueueError::Database)?;
    }
    connection
        .pragma_update(None, "journal_mode", "WAL")
        .map_err(|_| QueueError::Database)?;
    connection
        .pragma_update(None, "synchronous", "NORMAL")
        .map_err(|_| QueueError::Database)?;
    Ok(())
}

fn select_task(connection: &Connection, id: i64) -> Result<DownloadTask, QueueError> {
    connection
        .query_row(
            "SELECT id, kind, resource_id, title, author, state, completed_items,
                    total_items, downloaded_bytes, attempt_count, failure, created_at, updated_at
             FROM download_tasks WHERE id = ?1",
            [id],
            task_from_row,
        )
        .optional()
        .map_err(|_| QueueError::Database)?
        .ok_or(QueueError::TaskNotFound)
}

fn task_from_row(row: &Row<'_>) -> rusqlite::Result<DownloadTask> {
    let invalid = || {
        rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Text,
            Box::new(QueueError::InvalidDatabase),
        )
    };
    let kind = DownloadKind::parse(&row.get::<_, String>(1)?).map_err(|_| invalid())?;
    let state = DownloadState::parse(&row.get::<_, String>(5)?).map_err(|_| invalid())?;
    let failure = row
        .get::<_, Option<String>>(10)?
        .map(|value| DownloadFailure::parse(&value).map_err(|_| invalid()))
        .transpose()?;
    Ok(DownloadTask {
        id: row.get(0)?,
        kind,
        resource_id: row.get(2)?,
        title: row.get(3)?,
        author: row.get(4)?,
        state,
        completed_items: sql_u32(row.get(6)?)?,
        total_items: sql_u32(row.get(7)?)?,
        downloaded_bytes: sql_u64(row.get(8)?)?,
        attempt_count: sql_u32(row.get(9)?)?,
        failure,
        created_at_unix_seconds: sql_u64(row.get(11)?)?,
        updated_at_unix_seconds: sql_u64(row.get(12)?)?,
    })
}

fn normalized_resource_id(candidate: &str) -> Result<String, QueueError> {
    let value = candidate
        .parse::<u64>()
        .map_err(|_| QueueError::InvalidInput)?;
    if value == 0 {
        return Err(QueueError::InvalidInput);
    }
    Ok(value.to_string())
}

fn normalized_optional_text(
    candidate: Option<String>,
    max_bytes: usize,
) -> Result<Option<String>, QueueError> {
    let Some(candidate) = candidate else {
        return Ok(None);
    };
    let value = candidate.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.len() > max_bytes || value.chars().any(char::is_control) {
        return Err(QueueError::InvalidInput);
    }
    Ok(Some(value.to_owned()))
}

fn sql_u32(value: i64) -> rusqlite::Result<u32> {
    u32::try_from(value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Integer,
            Box::new(error),
        )
    })
}

fn sql_u64(value: i64) -> rusqlite::Result<u64> {
    u64::try_from(value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Integer,
            Box::new(error),
        )
    })
}

fn to_sql_u64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::{
        DownloadFailure, DownloadKind, DownloadQueue, DownloadState, NewDownloadTask, QueueError,
    };
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_root(name: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("pixiv-client-download-queue-{name}-{nonce}"))
    }

    fn task(kind: DownloadKind, resource_id: &str) -> NewDownloadTask {
        NewDownloadTask {
            kind,
            resource_id: resource_id.to_owned(),
            title: Some("  Sample title  ".to_owned()),
            author: Some("Author".to_owned()),
        }
    }

    #[test]
    fn migrates_and_persists_unique_tasks_across_reopen() {
        let root = test_root("persistence");
        let path = root.join("queue.sqlite3");
        let queue = DownloadQueue::open(&path).unwrap();
        assert_eq!(queue.schema_version().unwrap(), 1);
        assert_eq!(
            queue.stats().unwrap(),
            super::DownloadQueueStats {
                task_count: 0,
                active_count: 0,
                failed_count: 0,
                completed_count: 0,
            }
        );
        let first = queue.enqueue(task(DownloadKind::Artwork, "0042")).unwrap();
        assert_eq!(first.resource_id, "42");
        assert_eq!(first.title.as_deref(), Some("Sample title"));
        let duplicate = queue.enqueue(task(DownloadKind::Artwork, "42")).unwrap();
        assert_eq!(duplicate.id, first.id);
        drop(queue);

        let reopened = DownloadQueue::open(&path).unwrap();
        assert_eq!(reopened.list().unwrap(), [duplicate]);
        assert!(path.is_file());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn enforces_the_persistent_pause_retry_and_completion_state_machine() {
        let root = test_root("states");
        let queue = DownloadQueue::open(root.join("queue.sqlite3")).unwrap();
        let first = queue.enqueue(task(DownloadKind::Novel, "7")).unwrap();
        let running = queue.claim_next().unwrap().unwrap();
        assert_eq!(running.id, first.id);
        assert_eq!(running.state, DownloadState::Running);
        assert_eq!(running.attempt_count, 1);
        assert_eq!(
            queue
                .update_progress(first.id, 1, 3, 128)
                .unwrap()
                .completed_items,
            1
        );
        assert_eq!(queue.pause(first.id).unwrap().state, DownloadState::Paused);
        assert_eq!(
            queue
                .mark_failed(first.id, DownloadFailure::Network)
                .unwrap()
                .state,
            DownloadState::Paused
        );
        assert_eq!(queue.resume(first.id).unwrap().state, DownloadState::Queued);
        let running = queue.claim_next().unwrap().unwrap();
        assert_eq!(running.attempt_count, 2);
        let failed = queue
            .mark_failed(first.id, DownloadFailure::Network)
            .unwrap();
        assert_eq!(failed.state, DownloadState::Failed);
        assert_eq!(failed.failure, Some(DownloadFailure::Network));
        assert_eq!(queue.resume(first.id).unwrap().state, DownloadState::Queued);
        queue.claim_next().unwrap().unwrap();
        let completed = queue.mark_completed(first.id, 3, 512).unwrap();
        assert_eq!(completed.state, DownloadState::Completed);
        assert_eq!(completed.completed_items, 3);
        assert_eq!(completed.downloaded_bytes, 512);
        assert_eq!(queue.stats().unwrap().completed_count, 1);
        assert!(queue.remove(first.id).unwrap());
        assert!(queue.list().unwrap().is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn recovers_running_tasks_after_an_interrupted_process() {
        let root = test_root("recovery");
        let path = root.join("queue.sqlite3");
        let queue = DownloadQueue::open(&path).unwrap();
        let task = queue.enqueue(task(DownloadKind::Ugoira, "99")).unwrap();
        queue.claim_next().unwrap().unwrap();
        drop(queue);

        let reopened = DownloadQueue::open(path).unwrap();
        assert_eq!(reopened.recover_interrupted().unwrap(), 1);
        let recovered = reopened.get(task.id).unwrap();
        assert_eq!(recovered.state, DownloadState::Queued);
        assert_eq!(recovered.failure, Some(DownloadFailure::Interrupted));
        assert_eq!(reopened.claim_next().unwrap().unwrap().attempt_count, 2);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_invalid_identifiers_text_progress_and_transitions() {
        let root = test_root("validation");
        let queue = DownloadQueue::open(root.join("queue.sqlite3")).unwrap();
        assert_eq!(
            queue.enqueue(task(DownloadKind::Artwork, "../1")),
            Err(QueueError::InvalidInput)
        );
        let mut bad = task(DownloadKind::Artwork, "1");
        bad.title = Some("bad\ntext".to_owned());
        assert_eq!(queue.enqueue(bad), Err(QueueError::InvalidInput));
        let task = queue.enqueue(task(DownloadKind::Artwork, "1")).unwrap();
        assert_eq!(
            queue.update_progress(task.id, 2, 1, 0),
            Err(QueueError::InvalidInput)
        );
        assert_eq!(queue.resume(task.id), Err(QueueError::InvalidTransition));
        queue.claim_next().unwrap().unwrap();
        assert_eq!(queue.remove(task.id), Err(QueueError::InvalidTransition));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn clearing_the_queue_never_touches_adjacent_application_data() {
        let root = test_root("clear-boundary");
        let queue_root = root.join("queue");
        let adjacent = root.join("offline-library").join("keep.txt");
        fs::create_dir_all(adjacent.parent().unwrap()).unwrap();
        fs::write(&adjacent, b"keep").unwrap();
        let queue = DownloadQueue::open(queue_root.join("queue.sqlite3")).unwrap();
        queue.enqueue(task(DownloadKind::Novel, "8")).unwrap();

        let removed = queue.clear().unwrap();

        assert_eq!(removed.task_count, 1);
        assert!(queue.list().unwrap().is_empty());
        assert_eq!(fs::read(&adjacent).unwrap(), b"keep");
        fs::remove_dir_all(root).unwrap();
    }
}
