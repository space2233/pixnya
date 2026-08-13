use rusqlite::{params, Connection, ErrorCode, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const SCHEMA_VERSION: u32 = 2;
const MAX_COLLECTION_NAME_BYTES: usize = 128;
const MAX_TAG_NAME_BYTES: usize = 96;
const MAX_TAGS_PER_ENTRY: usize = 16;
const MAX_VISIBLE_ENTRIES: usize = 100_000;
const MAX_BATCH_ENTRIES: usize = 1_000;
const MAX_FILTER_QUERY_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogCollection {
    pub id: i64,
    pub name: String,
    pub entry_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryOrganization {
    pub entry_key: String,
    pub collection_id: Option<i64>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSnapshot {
    pub collections: Vec<CatalogCollection>,
    pub entries: Vec<EntryOrganization>,
    pub saved_filters: Vec<SavedCatalogFilter>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogFilterDraft {
    pub name: String,
    pub query: String,
    pub kind: Option<String>,
    pub collection_id: Option<i64>,
    pub tag: Option<String>,
    pub sort_order: String,
    pub stored_after: Option<u64>,
    pub stored_before: Option<u64>,
    pub min_size_bytes: Option<u64>,
    pub max_size_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedCatalogFilter {
    pub id: i64,
    #[serde(flatten)]
    pub filter: CatalogFilterDraft,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchOrganizationChange {
    pub entry_keys: Vec<String>,
    #[serde(default)]
    pub update_collection: bool,
    pub collection_id: Option<i64>,
    pub add_tags: Vec<String>,
    pub remove_tags: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogClearStats {
    pub collection_count: u32,
    pub organized_entry_count: u32,
    pub tag_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogError {
    InvalidInput,
    CollectionNotFound,
    Conflict,
    InvalidDatabase,
    Database,
    Io,
}

impl fmt::Display for CatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidInput => "invalid local catalog input",
            Self::CollectionNotFound => "local collection was not found",
            Self::Conflict => "local collection name already exists",
            Self::InvalidDatabase => "local catalog database is invalid or too new",
            Self::Database => "local catalog database operation failed",
            Self::Io => "local catalog filesystem operation failed",
        })
    }
}

impl std::error::Error for CatalogError {}

#[derive(Clone)]
pub struct LocalCatalog {
    path: PathBuf,
}

impl LocalCatalog {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, CatalogError> {
        let path = path.into();
        let parent = path.parent().ok_or(CatalogError::InvalidInput)?;
        fs::create_dir_all(parent).map_err(|_| CatalogError::Io)?;
        let catalog = Self { path };
        let mut connection = catalog.connection()?;
        migrate(&mut connection)?;
        Ok(catalog)
    }

    pub fn schema_version(&self) -> Result<u32, CatalogError> {
        self.connection()?
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(|_| CatalogError::Database)
    }

    pub fn snapshot(&self, visible_entry_keys: &[String]) -> Result<CatalogSnapshot, CatalogError> {
        if visible_entry_keys.len() > MAX_VISIBLE_ENTRIES {
            return Err(CatalogError::InvalidInput);
        }
        let mut visible = HashSet::with_capacity(visible_entry_keys.len());
        for key in visible_entry_keys {
            visible.insert(normalized_entry_key(key)?);
        }

        let connection = self.connection()?;
        let mut collections = read_collections(&connection)?;
        let mut collection_indexes = HashMap::with_capacity(collections.len());
        for (index, collection) in collections.iter().enumerate() {
            collection_indexes.insert(collection.id, index);
        }

        let mut organizations = BTreeMap::<String, EntryOrganization>::new();
        let mut organization_statement = connection
            .prepare(
                "SELECT entry_key, collection_id
                 FROM entry_organization
                 ORDER BY entry_key",
            )
            .map_err(|_| CatalogError::Database)?;
        let organization_rows = organization_statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Option<i64>>(1)?))
            })
            .map_err(|_| CatalogError::Database)?;
        for row in organization_rows {
            let (entry_key, collection_id) = row.map_err(|_| CatalogError::Database)?;
            let normalized =
                normalized_entry_key(&entry_key).map_err(|_| CatalogError::InvalidDatabase)?;
            if normalized != entry_key {
                return Err(CatalogError::InvalidDatabase);
            }
            if let Some(id) = collection_id {
                let Some(index) = collection_indexes.get(&id).copied() else {
                    return Err(CatalogError::InvalidDatabase);
                };
                if visible.contains(&entry_key) {
                    collections[index].entry_count = collections[index]
                        .entry_count
                        .checked_add(1)
                        .ok_or(CatalogError::InvalidDatabase)?;
                }
            }
            if visible.contains(&entry_key) {
                organizations.insert(
                    entry_key.clone(),
                    EntryOrganization {
                        entry_key,
                        collection_id,
                        tags: Vec::new(),
                    },
                );
            }
        }
        drop(organization_statement);

        let mut tag_statement = connection
            .prepare(
                "SELECT et.entry_key, t.name
                 FROM entry_tags et
                 INNER JOIN catalog_tags t ON t.id = et.tag_id
                 ORDER BY et.entry_key, t.name_key, t.id",
            )
            .map_err(|_| CatalogError::Database)?;
        let tag_rows = tag_statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|_| CatalogError::Database)?;
        for row in tag_rows {
            let (entry_key, tag) = row.map_err(|_| CatalogError::Database)?;
            let normalized_tag = normalized_name(&tag, MAX_TAG_NAME_BYTES)
                .map_err(|_| CatalogError::InvalidDatabase)?;
            if normalized_tag != tag {
                return Err(CatalogError::InvalidDatabase);
            }
            if let Some(organization) = organizations.get_mut(&entry_key) {
                if organization.tags.len() >= MAX_TAGS_PER_ENTRY {
                    return Err(CatalogError::InvalidDatabase);
                }
                organization.tags.push(tag);
            }
        }

        Ok(CatalogSnapshot {
            collections,
            entries: organizations.into_values().collect(),
            saved_filters: read_saved_filters(&connection)?,
        })
    }

    pub fn portable_snapshot(&self) -> Result<CatalogSnapshot, CatalogError> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare("SELECT entry_key FROM entry_organization ORDER BY entry_key")
            .map_err(|_| CatalogError::Database)?;
        let keys = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|_| CatalogError::Database)?
            .map(|row| row.map_err(|_| CatalogError::InvalidDatabase))
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        drop(connection);
        self.snapshot(&keys)
    }

    pub fn restore_snapshot(
        &self,
        snapshot: &CatalogSnapshot,
        replace: bool,
    ) -> Result<CatalogSnapshot, CatalogError> {
        if snapshot.collections.len() > MAX_VISIBLE_ENTRIES
            || snapshot.entries.len() > MAX_VISIBLE_ENTRIES
            || snapshot.saved_filters.len() > MAX_VISIBLE_ENTRIES
        {
            return Err(CatalogError::InvalidInput);
        }
        let mut source_collection_names = BTreeMap::new();
        for collection in &snapshot.collections {
            if collection.id <= 0 {
                return Err(CatalogError::InvalidInput);
            }
            let name = normalized_name(&collection.name, MAX_COLLECTION_NAME_BYTES)?;
            if source_collection_names
                .insert(collection.id, name)
                .is_some()
            {
                return Err(CatalogError::InvalidInput);
            }
        }
        let mut normalized_entries = Vec::with_capacity(snapshot.entries.len());
        for entry in &snapshot.entries {
            let entry_key = normalized_entry_key(&entry.entry_key)?;
            if entry
                .collection_id
                .is_some_and(|id| !source_collection_names.contains_key(&id))
            {
                return Err(CatalogError::InvalidInput);
            }
            normalized_entries.push((
                entry_key,
                entry.collection_id,
                normalized_tag_map(&entry.tags)?,
            ));
        }
        let mut normalized_filters = Vec::with_capacity(snapshot.saved_filters.len());
        for saved in &snapshot.saved_filters {
            if saved.id <= 0
                || saved
                    .filter
                    .collection_id
                    .is_some_and(|id| !source_collection_names.contains_key(&id))
            {
                return Err(CatalogError::InvalidInput);
            }
            normalized_filters.push(normalized_filter(&saved.filter)?);
        }

        let now = to_sql_u64(unix_seconds());
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| CatalogError::Database)?;
        if replace {
            transaction
                .execute_batch(
                    "DELETE FROM catalog_saved_filters;
                     DELETE FROM entry_tags;
                     DELETE FROM entry_organization;
                     DELETE FROM catalog_tags;
                     DELETE FROM catalog_collections;",
                )
                .map_err(|_| CatalogError::Database)?;
        }

        let mut collection_id_map = HashMap::new();
        for (source_id, name) in source_collection_names {
            let name_key = name.to_lowercase();
            let existing = transaction
                .query_row(
                    "SELECT id, name FROM catalog_collections WHERE name_key = ?1",
                    [&name_key],
                    |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
                )
                .optional()
                .map_err(|_| CatalogError::Database)?;
            let target_id = if let Some((id, existing_name)) = existing {
                if existing_name != name {
                    return Err(CatalogError::Conflict);
                }
                id
            } else {
                transaction
                    .execute(
                        "INSERT INTO catalog_collections (name, name_key, created_at, updated_at)
                         VALUES (?1, ?2, ?3, ?3)",
                        params![name, name_key, now],
                    )
                    .map_err(map_insert_error)?;
                transaction.last_insert_rowid()
            };
            collection_id_map.insert(source_id, target_id);
        }

        for (entry_key, source_collection_id, mut source_tags) in normalized_entries {
            let mapped_collection_id = source_collection_id
                .map(|id| {
                    collection_id_map
                        .get(&id)
                        .copied()
                        .ok_or(CatalogError::InvalidInput)
                })
                .transpose()?;
            let (collection_id, tags) = if replace {
                (mapped_collection_id, source_tags)
            } else {
                let existing_collection = transaction
                    .query_row(
                        "SELECT collection_id FROM entry_organization WHERE entry_key = ?1",
                        [&entry_key],
                        |row| row.get::<_, Option<i64>>(0),
                    )
                    .optional()
                    .map_err(|_| CatalogError::Database)?
                    .flatten();
                let existing_tags = read_entry_tag_map(&transaction, &entry_key)?;
                source_tags.extend(existing_tags);
                if source_tags.len() > MAX_TAGS_PER_ENTRY {
                    return Err(CatalogError::Conflict);
                }
                (existing_collection.or(mapped_collection_id), source_tags)
            };
            write_entry_organization(&transaction, &entry_key, collection_id, &tags)?;
        }

        for mut filter in normalized_filters {
            filter.collection_id = filter
                .collection_id
                .map(|id| {
                    collection_id_map
                        .get(&id)
                        .copied()
                        .ok_or(CatalogError::InvalidInput)
                })
                .transpose()?;
            let name_key = filter.name.to_lowercase();
            let existing = transaction
                .query_row(
                    "SELECT id FROM catalog_saved_filters WHERE name_key = ?1",
                    [&name_key],
                    |row| row.get::<_, i64>(0),
                )
                .optional()
                .map_err(|_| CatalogError::Database)?;
            if existing.is_some() {
                if replace {
                    return Err(CatalogError::InvalidDatabase);
                }
                continue;
            }
            transaction
                .execute(
                    "INSERT INTO catalog_saved_filters (
                       name, name_key, query, kind, collection_id, tag, sort_order,
                       stored_after, stored_before, min_size_bytes, max_size_bytes, created_at, updated_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)",
                    params![
                        filter.name,
                        name_key,
                        filter.query,
                        filter.kind,
                        filter.collection_id,
                        filter.tag,
                        filter.sort_order,
                        filter.stored_after.map(to_sql_u64),
                        filter.stored_before.map(to_sql_u64),
                        filter.min_size_bytes.map(to_sql_u64),
                        filter.max_size_bytes.map(to_sql_u64),
                        now,
                    ],
                )
                .map_err(map_insert_error)?;
        }
        remove_empty_organizations(&transaction)?;
        remove_unused_tags(&transaction)?;
        transaction.commit().map_err(|_| CatalogError::Database)?;
        self.portable_snapshot()
    }

    pub fn create_collection(&self, name: &str) -> Result<CatalogCollection, CatalogError> {
        let name = normalized_name(name, MAX_COLLECTION_NAME_BYTES)?;
        let name_key = name.to_lowercase();
        let now = to_sql_u64(unix_seconds());
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| CatalogError::Database)?;
        ensure_collection_name_available(&transaction, &name_key, None)?;
        transaction
            .execute(
                "INSERT INTO catalog_collections (name, name_key, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?3)",
                params![name, name_key, now],
            )
            .map_err(map_insert_error)?;
        let id = transaction.last_insert_rowid();
        transaction.commit().map_err(|_| CatalogError::Database)?;
        Ok(CatalogCollection {
            id,
            name,
            entry_count: 0,
        })
    }

    pub fn rename_collection(
        &self,
        collection_id: i64,
        name: &str,
    ) -> Result<CatalogCollection, CatalogError> {
        if collection_id <= 0 {
            return Err(CatalogError::InvalidInput);
        }
        let name = normalized_name(name, MAX_COLLECTION_NAME_BYTES)?;
        let name_key = name.to_lowercase();
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| CatalogError::Database)?;
        ensure_collection_name_available(&transaction, &name_key, Some(collection_id))?;
        let changed = transaction
            .execute(
                "UPDATE catalog_collections
                 SET name = ?1, name_key = ?2, updated_at = ?3
                 WHERE id = ?4",
                params![name, name_key, to_sql_u64(unix_seconds()), collection_id],
            )
            .map_err(map_insert_error)?;
        if changed != 1 {
            return Err(CatalogError::CollectionNotFound);
        }
        let entry_count = transaction
            .query_row(
                "SELECT COUNT(*) FROM entry_organization WHERE collection_id = ?1",
                [collection_id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|_| CatalogError::Database)?;
        transaction.commit().map_err(|_| CatalogError::Database)?;
        Ok(CatalogCollection {
            id: collection_id,
            name,
            entry_count: sql_u32(entry_count)?,
        })
    }

    pub fn delete_collection(&self, collection_id: i64) -> Result<(), CatalogError> {
        if collection_id <= 0 {
            return Err(CatalogError::InvalidInput);
        }
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| CatalogError::Database)?;
        let changed = transaction
            .execute(
                "DELETE FROM catalog_collections WHERE id = ?1",
                [collection_id],
            )
            .map_err(|_| CatalogError::Database)?;
        if changed != 1 {
            return Err(CatalogError::CollectionNotFound);
        }
        remove_empty_organizations(&transaction)?;
        transaction.commit().map_err(|_| CatalogError::Database)
    }

    pub fn organize_entry(
        &self,
        entry_key: &str,
        collection_id: Option<i64>,
        tags: &[String],
    ) -> Result<EntryOrganization, CatalogError> {
        let entry_key = normalized_entry_key(entry_key)?;
        if tags.len() > MAX_TAGS_PER_ENTRY {
            return Err(CatalogError::InvalidInput);
        }
        let mut unique_tags = BTreeMap::<String, String>::new();
        for tag in tags {
            let tag = normalized_name(tag, MAX_TAG_NAME_BYTES)?;
            unique_tags.insert(tag.to_lowercase(), tag);
        }
        if unique_tags.len() > MAX_TAGS_PER_ENTRY {
            return Err(CatalogError::InvalidInput);
        }
        if collection_id.is_some_and(|id| id <= 0) {
            return Err(CatalogError::InvalidInput);
        }

        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| CatalogError::Database)?;
        if let Some(id) = collection_id {
            let exists = transaction
                .query_row(
                    "SELECT 1 FROM catalog_collections WHERE id = ?1",
                    [id],
                    |_| Ok(()),
                )
                .optional()
                .map_err(|_| CatalogError::Database)?
                .is_some();
            if !exists {
                return Err(CatalogError::CollectionNotFound);
            }
        }

        transaction
            .execute("DELETE FROM entry_tags WHERE entry_key = ?1", [&entry_key])
            .map_err(|_| CatalogError::Database)?;
        if collection_id.is_none() && unique_tags.is_empty() {
            transaction
                .execute(
                    "DELETE FROM entry_organization WHERE entry_key = ?1",
                    [&entry_key],
                )
                .map_err(|_| CatalogError::Database)?;
            remove_unused_tags(&transaction)?;
            transaction.commit().map_err(|_| CatalogError::Database)?;
            return Ok(EntryOrganization {
                entry_key,
                collection_id: None,
                tags: Vec::new(),
            });
        }

        transaction
            .execute(
                "INSERT INTO entry_organization (entry_key, collection_id, updated_at)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(entry_key) DO UPDATE SET
                   collection_id = excluded.collection_id,
                   updated_at = excluded.updated_at",
                params![entry_key, collection_id, to_sql_u64(unix_seconds())],
            )
            .map_err(|_| CatalogError::Database)?;
        let mut result_tags = Vec::with_capacity(unique_tags.len());
        for (name_key, name) in unique_tags {
            transaction
                .execute(
                    "INSERT INTO catalog_tags (name, name_key)
                     VALUES (?1, ?2)
                     ON CONFLICT(name_key) DO UPDATE SET name = excluded.name",
                    params![name, name_key],
                )
                .map_err(map_insert_error)?;
            let tag_id = transaction
                .query_row(
                    "SELECT id FROM catalog_tags WHERE name_key = ?1",
                    [&name_key],
                    |row| row.get::<_, i64>(0),
                )
                .map_err(|_| CatalogError::Database)?;
            transaction
                .execute(
                    "INSERT INTO entry_tags (entry_key, tag_id) VALUES (?1, ?2)",
                    params![entry_key, tag_id],
                )
                .map_err(|_| CatalogError::Database)?;
            result_tags.push(name);
        }
        remove_unused_tags(&transaction)?;
        transaction.commit().map_err(|_| CatalogError::Database)?;
        Ok(EntryOrganization {
            entry_key,
            collection_id,
            tags: result_tags,
        })
    }

    pub fn batch_organize_entries(
        &self,
        change: &BatchOrganizationChange,
    ) -> Result<Vec<EntryOrganization>, CatalogError> {
        if change.entry_keys.is_empty() || change.entry_keys.len() > MAX_BATCH_ENTRIES {
            return Err(CatalogError::InvalidInput);
        }
        let mut entry_keys = Vec::with_capacity(change.entry_keys.len());
        let mut seen_entries = HashSet::with_capacity(change.entry_keys.len());
        for entry_key in &change.entry_keys {
            let entry_key = normalized_entry_key(entry_key)?;
            if !seen_entries.insert(entry_key.clone()) {
                return Err(CatalogError::InvalidInput);
            }
            entry_keys.push(entry_key);
        }
        if change.update_collection && change.collection_id.is_some_and(|id| id <= 0) {
            return Err(CatalogError::InvalidInput);
        }
        let add_tags = normalized_tag_map(&change.add_tags)?;
        let remove_tags = normalized_tag_map(&change.remove_tags)?;
        if add_tags.keys().any(|key| remove_tags.contains_key(key)) {
            return Err(CatalogError::InvalidInput);
        }

        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| CatalogError::Database)?;
        if let Some(collection_id) = change.collection_id.filter(|_| change.update_collection) {
            let exists = transaction
                .query_row(
                    "SELECT 1 FROM catalog_collections WHERE id = ?1",
                    [collection_id],
                    |_| Ok(()),
                )
                .optional()
                .map_err(|_| CatalogError::Database)?
                .is_some();
            if !exists {
                return Err(CatalogError::CollectionNotFound);
            }
        }

        let mut changed = Vec::with_capacity(entry_keys.len());
        for entry_key in entry_keys {
            let collection_id = if change.update_collection {
                change.collection_id
            } else {
                transaction
                    .query_row(
                        "SELECT collection_id FROM entry_organization WHERE entry_key = ?1",
                        [&entry_key],
                        |row| row.get::<_, Option<i64>>(0),
                    )
                    .optional()
                    .map_err(|_| CatalogError::Database)?
                    .flatten()
            };
            let mut tags = read_entry_tag_map(&transaction, &entry_key)?;
            for key in remove_tags.keys() {
                tags.remove(key);
            }
            for (key, name) in &add_tags {
                tags.insert(key.clone(), name.clone());
            }
            if tags.len() > MAX_TAGS_PER_ENTRY {
                return Err(CatalogError::InvalidInput);
            }
            write_entry_organization(&transaction, &entry_key, collection_id, &tags)?;
            changed.push(EntryOrganization {
                entry_key,
                collection_id,
                tags: tags.into_values().collect(),
            });
        }
        remove_unused_tags(&transaction)?;
        transaction.commit().map_err(|_| CatalogError::Database)?;
        Ok(changed)
    }

    pub fn save_filter(
        &self,
        draft: &CatalogFilterDraft,
    ) -> Result<SavedCatalogFilter, CatalogError> {
        let filter = normalized_filter(draft)?;
        let name_key = filter.name.to_lowercase();
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| CatalogError::Database)?;
        if let Some(collection_id) = filter.collection_id {
            let exists = transaction
                .query_row(
                    "SELECT 1 FROM catalog_collections WHERE id = ?1",
                    [collection_id],
                    |_| Ok(()),
                )
                .optional()
                .map_err(|_| CatalogError::Database)?
                .is_some();
            if !exists {
                return Err(CatalogError::CollectionNotFound);
            }
        }
        transaction
            .execute(
                "INSERT INTO catalog_saved_filters (
                   name, name_key, query, kind, collection_id, tag, sort_order,
                   stored_after, stored_before, min_size_bytes, max_size_bytes, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)",
                params![
                    filter.name,
                    name_key,
                    filter.query,
                    filter.kind,
                    filter.collection_id,
                    filter.tag,
                    filter.sort_order,
                    filter.stored_after.map(to_sql_u64),
                    filter.stored_before.map(to_sql_u64),
                    filter.min_size_bytes.map(to_sql_u64),
                    filter.max_size_bytes.map(to_sql_u64),
                    to_sql_u64(unix_seconds()),
                ],
            )
            .map_err(map_insert_error)?;
        let id = transaction.last_insert_rowid();
        transaction.commit().map_err(|_| CatalogError::Database)?;
        Ok(SavedCatalogFilter { id, filter })
    }

    pub fn delete_filter(&self, filter_id: i64) -> Result<bool, CatalogError> {
        if filter_id <= 0 {
            return Err(CatalogError::InvalidInput);
        }
        Ok(self
            .connection()?
            .execute(
                "DELETE FROM catalog_saved_filters WHERE id = ?1",
                [filter_id],
            )
            .map_err(|_| CatalogError::Database)?
            == 1)
    }

    pub fn remove_entries(&self, entry_keys: &[String]) -> Result<u32, CatalogError> {
        if entry_keys.is_empty() || entry_keys.len() > MAX_BATCH_ENTRIES {
            return Err(CatalogError::InvalidInput);
        }
        let mut keys = Vec::with_capacity(entry_keys.len());
        let mut seen = HashSet::with_capacity(entry_keys.len());
        for key in entry_keys {
            let key = normalized_entry_key(key)?;
            if !seen.insert(key.clone()) {
                return Err(CatalogError::InvalidInput);
            }
            keys.push(key);
        }
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| CatalogError::Database)?;
        let mut removed = 0_u32;
        for key in keys {
            removed = removed
                .checked_add(
                    u32::try_from(
                        transaction
                            .execute(
                                "DELETE FROM entry_organization WHERE entry_key = ?1",
                                [&key],
                            )
                            .map_err(|_| CatalogError::Database)?,
                    )
                    .map_err(|_| CatalogError::Database)?,
                )
                .ok_or(CatalogError::Database)?;
        }
        remove_unused_tags(&transaction)?;
        transaction.commit().map_err(|_| CatalogError::Database)?;
        Ok(removed)
    }

    pub fn restore_entries(&self, entries: &[EntryOrganization]) -> Result<(), CatalogError> {
        if entries.len() > MAX_BATCH_ENTRIES {
            return Err(CatalogError::InvalidInput);
        }
        let mut normalized = Vec::with_capacity(entries.len());
        let mut seen = HashSet::with_capacity(entries.len());
        for entry in entries {
            let entry_key = normalized_entry_key(&entry.entry_key)?;
            if !seen.insert(entry_key.clone())
                || entry
                    .collection_id
                    .is_some_and(|collection_id| collection_id <= 0)
            {
                return Err(CatalogError::InvalidInput);
            }
            normalized.push((
                entry_key,
                entry.collection_id,
                normalized_tag_map(&entry.tags)?,
            ));
        }

        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| CatalogError::Database)?;
        for (_, collection_id, _) in &normalized {
            if let Some(collection_id) = collection_id {
                let exists = transaction
                    .query_row(
                        "SELECT 1 FROM catalog_collections WHERE id = ?1",
                        [collection_id],
                        |_| Ok(()),
                    )
                    .optional()
                    .map_err(|_| CatalogError::Database)?
                    .is_some();
                if !exists {
                    return Err(CatalogError::CollectionNotFound);
                }
            }
        }
        for (entry_key, collection_id, tags) in normalized {
            write_entry_organization(&transaction, &entry_key, collection_id, &tags)?;
        }
        remove_unused_tags(&transaction)?;
        transaction.commit().map_err(|_| CatalogError::Database)
    }

    pub fn remove_entry(&self, entry_key: &str) -> Result<bool, CatalogError> {
        let entry_key = normalized_entry_key(entry_key)?;
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| CatalogError::Database)?;
        let changed = transaction
            .execute(
                "DELETE FROM entry_organization WHERE entry_key = ?1",
                [&entry_key],
            )
            .map_err(|_| CatalogError::Database)?;
        remove_unused_tags(&transaction)?;
        transaction.commit().map_err(|_| CatalogError::Database)?;
        Ok(changed == 1)
    }

    pub fn clear(&self) -> Result<CatalogClearStats, CatalogError> {
        let connection = self.connection()?;
        let stats = CatalogClearStats {
            collection_count: count_rows(&connection, "catalog_collections")?,
            organized_entry_count: count_rows(&connection, "entry_organization")?,
            tag_count: count_rows(&connection, "catalog_tags")?,
        };
        connection
            .execute_batch(
                "BEGIN IMMEDIATE;
                 DELETE FROM catalog_saved_filters;
                 DELETE FROM entry_tags;
                 DELETE FROM entry_organization;
                 DELETE FROM catalog_tags;
                 DELETE FROM catalog_collections;
                 COMMIT;",
            )
            .map_err(|_| CatalogError::Database)?;
        Ok(stats)
    }

    fn connection(&self) -> Result<Connection, CatalogError> {
        let connection = Connection::open(&self.path).map_err(|_| CatalogError::Database)?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(|_| CatalogError::Database)?;
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .map_err(|_| CatalogError::Database)?;
        Ok(connection)
    }
}

fn migrate(connection: &mut Connection) -> Result<(), CatalogError> {
    let version: u32 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|_| CatalogError::Database)?;
    if version > SCHEMA_VERSION {
        return Err(CatalogError::InvalidDatabase);
    }
    if version == 0 {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| CatalogError::Database)?;
        transaction
            .execute_batch(
                "CREATE TABLE catalog_collections (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL CHECK(length(name) BETWEEN 1 AND 128),
                    name_key TEXT NOT NULL UNIQUE CHECK(length(name_key) BETWEEN 1 AND 128),
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                 );
                 CREATE TABLE entry_organization (
                    entry_key TEXT PRIMARY KEY NOT NULL CHECK(length(entry_key) BETWEEN 3 AND 64),
                    collection_id INTEGER REFERENCES catalog_collections(id) ON DELETE SET NULL,
                    updated_at INTEGER NOT NULL
                 );
                 CREATE INDEX entry_organization_collection_idx
                   ON entry_organization(collection_id, entry_key);
                 CREATE TABLE catalog_tags (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL CHECK(length(name) BETWEEN 1 AND 96),
                    name_key TEXT NOT NULL UNIQUE CHECK(length(name_key) BETWEEN 1 AND 96)
                 );
                 CREATE TABLE entry_tags (
                    entry_key TEXT NOT NULL REFERENCES entry_organization(entry_key) ON DELETE CASCADE,
                    tag_id INTEGER NOT NULL REFERENCES catalog_tags(id) ON DELETE CASCADE,
                    PRIMARY KEY(entry_key, tag_id)
                 );
                 CREATE INDEX entry_tags_tag_idx ON entry_tags(tag_id, entry_key);
                 CREATE TABLE catalog_saved_filters (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL CHECK(length(name) BETWEEN 1 AND 128),
                    name_key TEXT NOT NULL UNIQUE CHECK(length(name_key) BETWEEN 1 AND 128),
                    query TEXT NOT NULL CHECK(length(query) <= 512),
                    kind TEXT CHECK(kind IN ('artwork', 'novel', 'ugoira')),
                    collection_id INTEGER REFERENCES catalog_collections(id) ON DELETE SET NULL,
                    tag TEXT CHECK(tag IS NULL OR length(tag) BETWEEN 1 AND 96),
                    sort_order TEXT NOT NULL CHECK(sort_order IN ('newest', 'oldest', 'title', 'size')),
                    stored_after INTEGER,
                    stored_before INTEGER,
                    min_size_bytes INTEGER,
                    max_size_bytes INTEGER,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                 );",
            )
            .map_err(|_| CatalogError::Database)?;
        transaction
            .pragma_update(None, "user_version", SCHEMA_VERSION)
            .map_err(|_| CatalogError::Database)?;
        transaction.commit().map_err(|_| CatalogError::Database)?;
    }
    if version == 1 {
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| CatalogError::Database)?;
        transaction
            .execute_batch(
                "CREATE TABLE catalog_saved_filters (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL CHECK(length(name) BETWEEN 1 AND 128),
                    name_key TEXT NOT NULL UNIQUE CHECK(length(name_key) BETWEEN 1 AND 128),
                    query TEXT NOT NULL CHECK(length(query) <= 512),
                    kind TEXT CHECK(kind IN ('artwork', 'novel', 'ugoira')),
                    collection_id INTEGER REFERENCES catalog_collections(id) ON DELETE SET NULL,
                    tag TEXT CHECK(tag IS NULL OR length(tag) BETWEEN 1 AND 96),
                    sort_order TEXT NOT NULL CHECK(sort_order IN ('newest', 'oldest', 'title', 'size')),
                    stored_after INTEGER,
                    stored_before INTEGER,
                    min_size_bytes INTEGER,
                    max_size_bytes INTEGER,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                 );",
            )
            .map_err(|_| CatalogError::Database)?;
        transaction
            .pragma_update(None, "user_version", SCHEMA_VERSION)
            .map_err(|_| CatalogError::Database)?;
        transaction.commit().map_err(|_| CatalogError::Database)?;
    }
    connection
        .pragma_update(None, "journal_mode", "WAL")
        .map_err(|_| CatalogError::Database)?;
    connection
        .pragma_update(None, "synchronous", "NORMAL")
        .map_err(|_| CatalogError::Database)?;
    Ok(())
}

fn read_collections(connection: &Connection) -> Result<Vec<CatalogCollection>, CatalogError> {
    let mut statement = connection
        .prepare("SELECT id, name FROM catalog_collections ORDER BY name_key, id")
        .map_err(|_| CatalogError::Database)?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|_| CatalogError::Database)?;
    let mut collections = Vec::new();
    for row in rows {
        let (id, name) = row.map_err(|_| CatalogError::Database)?;
        if id <= 0 || normalized_name(&name, MAX_COLLECTION_NAME_BYTES).as_deref() != Ok(&name) {
            return Err(CatalogError::InvalidDatabase);
        }
        collections.push(CatalogCollection {
            id,
            name,
            entry_count: 0,
        });
    }
    Ok(collections)
}

fn read_saved_filters(connection: &Connection) -> Result<Vec<SavedCatalogFilter>, CatalogError> {
    let mut statement = connection
        .prepare(
            "SELECT id, name, query, kind, collection_id, tag, sort_order,
                    stored_after, stored_before, min_size_bytes, max_size_bytes
             FROM catalog_saved_filters ORDER BY name_key, id",
        )
        .map_err(|_| CatalogError::Database)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                CatalogFilterDraft {
                    name: row.get(1)?,
                    query: row.get(2)?,
                    kind: row.get(3)?,
                    collection_id: row.get(4)?,
                    tag: row.get(5)?,
                    sort_order: row.get(6)?,
                    stored_after: optional_sql_u64(row.get(7)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    stored_before: optional_sql_u64(row.get(8)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    min_size_bytes: optional_sql_u64(row.get(9)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    max_size_bytes: optional_sql_u64(row.get(10)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                },
            ))
        })
        .map_err(|_| CatalogError::Database)?;
    let mut filters = Vec::new();
    for row in rows {
        let (id, filter) = row.map_err(|_| CatalogError::InvalidDatabase)?;
        if id <= 0 || normalized_filter(&filter).as_ref() != Ok(&filter) {
            return Err(CatalogError::InvalidDatabase);
        }
        filters.push(SavedCatalogFilter { id, filter });
    }
    Ok(filters)
}

fn normalized_filter(draft: &CatalogFilterDraft) -> Result<CatalogFilterDraft, CatalogError> {
    let name = normalized_name(&draft.name, MAX_COLLECTION_NAME_BYTES)?;
    let query = draft.query.trim();
    if query.len() > MAX_FILTER_QUERY_BYTES || query.chars().any(char::is_control) {
        return Err(CatalogError::InvalidInput);
    }
    let kind = match draft.kind.as_deref() {
        None => None,
        Some(kind @ ("artwork" | "novel" | "ugoira")) => Some(kind.to_owned()),
        _ => return Err(CatalogError::InvalidInput),
    };
    if draft.collection_id.is_some_and(|id| id <= 0) {
        return Err(CatalogError::InvalidInput);
    }
    let tag = draft
        .tag
        .as_deref()
        .map(|tag| normalized_name(tag, MAX_TAG_NAME_BYTES))
        .transpose()?;
    if !matches!(
        draft.sort_order.as_str(),
        "newest" | "oldest" | "title" | "size"
    ) || matches!((draft.stored_after, draft.stored_before), (Some(after), Some(before)) if after > before)
        || matches!((draft.min_size_bytes, draft.max_size_bytes), (Some(min), Some(max)) if min > max)
    {
        return Err(CatalogError::InvalidInput);
    }
    Ok(CatalogFilterDraft {
        name,
        query: query.to_owned(),
        kind,
        collection_id: draft.collection_id,
        tag,
        sort_order: draft.sort_order.clone(),
        stored_after: draft.stored_after,
        stored_before: draft.stored_before,
        min_size_bytes: draft.min_size_bytes,
        max_size_bytes: draft.max_size_bytes,
    })
}

fn normalized_tag_map(tags: &[String]) -> Result<BTreeMap<String, String>, CatalogError> {
    if tags.len() > MAX_TAGS_PER_ENTRY {
        return Err(CatalogError::InvalidInput);
    }
    let mut normalized = BTreeMap::new();
    for tag in tags {
        let tag = normalized_name(tag, MAX_TAG_NAME_BYTES)?;
        normalized.insert(tag.to_lowercase(), tag);
    }
    Ok(normalized)
}

fn read_entry_tag_map(
    connection: &Connection,
    entry_key: &str,
) -> Result<BTreeMap<String, String>, CatalogError> {
    let mut statement = connection
        .prepare(
            "SELECT t.name_key, t.name FROM entry_tags et
             INNER JOIN catalog_tags t ON t.id = et.tag_id
             WHERE et.entry_key = ?1 ORDER BY t.name_key, t.id",
        )
        .map_err(|_| CatalogError::Database)?;
    let rows = statement
        .query_map([entry_key], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|_| CatalogError::Database)?;
    let mut tags = BTreeMap::new();
    for row in rows {
        let (key, value) = row.map_err(|_| CatalogError::Database)?;
        tags.insert(key, value);
    }
    Ok(tags)
}

fn write_entry_organization(
    connection: &Connection,
    entry_key: &str,
    collection_id: Option<i64>,
    tags: &BTreeMap<String, String>,
) -> Result<(), CatalogError> {
    connection
        .execute("DELETE FROM entry_tags WHERE entry_key = ?1", [entry_key])
        .map_err(|_| CatalogError::Database)?;
    if collection_id.is_none() && tags.is_empty() {
        connection
            .execute(
                "DELETE FROM entry_organization WHERE entry_key = ?1",
                [entry_key],
            )
            .map_err(|_| CatalogError::Database)?;
        return Ok(());
    }
    connection
        .execute(
            "INSERT INTO entry_organization (entry_key, collection_id, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(entry_key) DO UPDATE SET
               collection_id = excluded.collection_id, updated_at = excluded.updated_at",
            params![entry_key, collection_id, to_sql_u64(unix_seconds())],
        )
        .map_err(|_| CatalogError::Database)?;
    for (name_key, name) in tags {
        connection
            .execute(
                "INSERT INTO catalog_tags (name, name_key) VALUES (?1, ?2)
                 ON CONFLICT(name_key) DO UPDATE SET name = excluded.name",
                params![name, name_key],
            )
            .map_err(map_insert_error)?;
        let tag_id = connection
            .query_row(
                "SELECT id FROM catalog_tags WHERE name_key = ?1",
                [name_key],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|_| CatalogError::Database)?;
        connection
            .execute(
                "INSERT INTO entry_tags (entry_key, tag_id) VALUES (?1, ?2)",
                params![entry_key, tag_id],
            )
            .map_err(|_| CatalogError::Database)?;
    }
    Ok(())
}

fn ensure_collection_name_available(
    connection: &Connection,
    name_key: &str,
    except_id: Option<i64>,
) -> Result<(), CatalogError> {
    let existing = connection
        .query_row(
            "SELECT id FROM catalog_collections WHERE name_key = ?1",
            [name_key],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|_| CatalogError::Database)?;
    if existing.is_some_and(|id| Some(id) != except_id) {
        Err(CatalogError::Conflict)
    } else {
        Ok(())
    }
}

fn remove_empty_organizations(connection: &Connection) -> Result<(), CatalogError> {
    connection
        .execute(
            "DELETE FROM entry_organization
             WHERE collection_id IS NULL
               AND NOT EXISTS (
                 SELECT 1 FROM entry_tags WHERE entry_tags.entry_key = entry_organization.entry_key
               )",
            [],
        )
        .map_err(|_| CatalogError::Database)?;
    Ok(())
}

fn remove_unused_tags(connection: &Connection) -> Result<(), CatalogError> {
    connection
        .execute(
            "DELETE FROM catalog_tags
             WHERE NOT EXISTS (SELECT 1 FROM entry_tags WHERE entry_tags.tag_id = catalog_tags.id)",
            [],
        )
        .map_err(|_| CatalogError::Database)?;
    Ok(())
}

fn count_rows(connection: &Connection, table: &str) -> Result<u32, CatalogError> {
    let sql = match table {
        "catalog_collections" => "SELECT COUNT(*) FROM catalog_collections",
        "entry_organization" => "SELECT COUNT(*) FROM entry_organization",
        "catalog_tags" => "SELECT COUNT(*) FROM catalog_tags",
        _ => return Err(CatalogError::InvalidInput),
    };
    let count = connection
        .query_row(sql, [], |row| row.get::<_, i64>(0))
        .map_err(|_| CatalogError::Database)?;
    sql_u32(count)
}

fn normalized_entry_key(candidate: &str) -> Result<String, CatalogError> {
    let (kind, resource_id) = candidate
        .split_once('-')
        .ok_or(CatalogError::InvalidInput)?;
    if !matches!(kind, "artwork" | "novel" | "ugoira") {
        return Err(CatalogError::InvalidInput);
    }
    let resource_id = resource_id
        .parse::<u64>()
        .map_err(|_| CatalogError::InvalidInput)?;
    if resource_id == 0 {
        return Err(CatalogError::InvalidInput);
    }
    let normalized = format!("{kind}-{resource_id}");
    if normalized != candidate {
        return Err(CatalogError::InvalidInput);
    }
    Ok(normalized)
}

fn normalized_name(candidate: &str, max_bytes: usize) -> Result<String, CatalogError> {
    let value = candidate.trim();
    let valid = !value.is_empty()
        && value.len() <= max_bytes
        && !value.chars().any(|character| character.is_control());
    if valid {
        Ok(value.to_owned())
    } else {
        Err(CatalogError::InvalidInput)
    }
}

fn map_insert_error(error: rusqlite::Error) -> CatalogError {
    match error.sqlite_error_code() {
        Some(ErrorCode::ConstraintViolation) => CatalogError::Conflict,
        _ => CatalogError::Database,
    }
}

fn sql_u32(value: i64) -> Result<u32, CatalogError> {
    u32::try_from(value).map_err(|_| CatalogError::InvalidDatabase)
}

fn optional_sql_u64(value: Option<i64>) -> Result<Option<u64>, CatalogError> {
    value
        .map(|value| u64::try_from(value).map_err(|_| CatalogError::InvalidDatabase))
        .transpose()
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn to_sql_u64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::{
        BatchOrganizationChange, CatalogError, CatalogFilterDraft, LocalCatalog, MAX_TAGS_PER_ENTRY,
    };
    use rusqlite::Connection;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);

    fn test_root(label: &str) -> PathBuf {
        let sequence = TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "pixiv-client-local-catalog-{label}-{}-{sequence}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn migrates_persists_and_normalizes_organization() {
        let root = test_root("persistence");
        let path = root.join("catalog.sqlite3");
        let catalog = LocalCatalog::open(&path).unwrap();
        assert_eq!(catalog.schema_version().unwrap(), 2);
        let collection = catalog.create_collection(" 稍后阅读 ").unwrap();
        let organization = catalog
            .organize_entry(
                "artwork-42",
                Some(collection.id),
                &[" 风景 ".into(), "FAVORITE".into(), "favorite".into()],
            )
            .unwrap();
        assert_eq!(organization.tags, ["favorite", "风景"]);

        drop(catalog);
        let reopened = LocalCatalog::open(path).unwrap();
        let snapshot = reopened
            .snapshot(&["artwork-42".into(), "novel-7".into()])
            .unwrap();
        assert_eq!(snapshot.collections[0].name, "稍后阅读");
        assert_eq!(snapshot.collections[0].entry_count, 1);
        assert_eq!(snapshot.entries, [organization]);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn portable_snapshot_restores_with_collection_id_remapping() {
        let source_root = test_root("portable-source");
        let source = LocalCatalog::open(source_root.join("catalog.sqlite3")).unwrap();
        let collection = source.create_collection("Reference").unwrap();
        source
            .organize_entry("artwork-42", Some(collection.id), &["blue".into()])
            .unwrap();
        source
            .save_filter(&CatalogFilterDraft {
                name: "Blue references".into(),
                query: String::new(),
                kind: Some("artwork".into()),
                collection_id: Some(collection.id),
                tag: Some("blue".into()),
                sort_order: "newest".into(),
                stored_after: None,
                stored_before: None,
                min_size_bytes: None,
                max_size_bytes: None,
            })
            .unwrap();
        let portable = source.portable_snapshot().unwrap();

        let target_root = test_root("portable-target");
        let target = LocalCatalog::open(target_root.join("catalog.sqlite3")).unwrap();
        target.create_collection("Existing").unwrap();
        let restored = target.restore_snapshot(&portable, false).unwrap();
        let restored_collection = restored
            .collections
            .iter()
            .find(|value| value.name == "Reference")
            .unwrap();
        assert_ne!(restored_collection.id, collection.id);
        assert_eq!(
            restored.entries[0].collection_id,
            Some(restored_collection.id)
        );
        assert_eq!(
            restored.saved_filters[0].filter.collection_id,
            Some(restored_collection.id)
        );
        assert_eq!(restored.entries[0].tags, ["blue"]);
    }

    #[test]
    fn rejects_invalid_keys_names_duplicates_and_tag_overflow() {
        let root = test_root("validation");
        let catalog = LocalCatalog::open(root.join("catalog.sqlite3")).unwrap();
        assert_eq!(
            catalog.create_collection("  "),
            Err(CatalogError::InvalidInput)
        );
        let collection = catalog.create_collection("参考").unwrap();
        assert_eq!(
            catalog.create_collection("参考"),
            Err(CatalogError::Conflict)
        );
        assert_eq!(
            catalog.create_collection("参考\n"),
            Err(CatalogError::Conflict)
        );
        assert_eq!(
            catalog.organize_entry("artwork-01", None, &[]),
            Err(CatalogError::InvalidInput)
        );
        assert_eq!(
            catalog.organize_entry("artwork-1", Some(collection.id + 99), &[]),
            Err(CatalogError::CollectionNotFound)
        );
        let too_many = (0..=MAX_TAGS_PER_ENTRY)
            .map(|index| format!("tag-{index}"))
            .collect::<Vec<_>>();
        assert_eq!(
            catalog.organize_entry("artwork-1", None, &too_many),
            Err(CatalogError::InvalidInput)
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn deleting_collection_preserves_tags_and_removing_entry_cleans_orphans() {
        let root = test_root("deletion");
        let catalog = LocalCatalog::open(root.join("catalog.sqlite3")).unwrap();
        let collection = catalog.create_collection("绘画参考").unwrap();
        catalog
            .organize_entry("artwork-9", Some(collection.id), &["姿势".into()])
            .unwrap();
        catalog.delete_collection(collection.id).unwrap();
        let snapshot = catalog.snapshot(&["artwork-9".into()]).unwrap();
        assert!(snapshot.collections.is_empty());
        assert_eq!(snapshot.entries[0].collection_id, None);
        assert_eq!(snapshot.entries[0].tags, ["姿势"]);
        assert!(catalog.remove_entry("artwork-9").unwrap());
        assert!(!catalog.remove_entry("artwork-9").unwrap());
        let cleared = catalog.clear().unwrap();
        assert_eq!(cleared.tag_count, 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn visibility_filter_does_not_destroy_temporarily_missing_metadata() {
        let root = test_root("visibility");
        let catalog = LocalCatalog::open(root.join("catalog.sqlite3")).unwrap();
        catalog
            .organize_entry("novel-88", None, &["长篇".into()])
            .unwrap();
        assert!(catalog.snapshot(&[]).unwrap().entries.is_empty());
        let restored = catalog.snapshot(&["novel-88".into()]).unwrap();
        assert_eq!(restored.entries[0].tags, ["长篇"]);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn clear_keeps_database_and_adjacent_user_files() {
        let root = test_root("clear-boundary");
        let marker = root.join("user-file.txt");
        fs::write(&marker, b"keep").unwrap();
        let path = root.join("catalog.sqlite3");
        let catalog = LocalCatalog::open(&path).unwrap();
        catalog.create_collection("临时").unwrap();
        catalog
            .organize_entry("ugoira-5", None, &["动画".into()])
            .unwrap();
        let stats = catalog.clear().unwrap();
        assert_eq!(stats.collection_count, 1);
        assert_eq!(stats.organized_entry_count, 1);
        assert_eq!(stats.tag_count, 1);
        assert!(path.is_file());
        assert_eq!(fs::read(marker).unwrap(), b"keep");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn refuses_a_database_from_a_newer_schema() {
        let root = test_root("future-schema");
        let path = root.join("catalog.sqlite3");
        let connection = Connection::open(&path).unwrap();
        connection.pragma_update(None, "user_version", 999).unwrap();
        drop(connection);
        assert!(matches!(
            LocalCatalog::open(path),
            Err(CatalogError::InvalidDatabase)
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn batch_organization_is_atomic_and_saved_filters_migrate() {
        let root = test_root("batch-filter");
        let catalog = LocalCatalog::open(root.join("catalog.sqlite3")).unwrap();
        assert_eq!(catalog.schema_version().unwrap(), 2);
        let collection = catalog.create_collection("Reference").unwrap();
        catalog
            .organize_entry("artwork-1", None, &["old".into()])
            .unwrap();
        catalog
            .organize_entry("novel-2", None, &["old".into(), "keep".into()])
            .unwrap();

        let changed = catalog
            .batch_organize_entries(&BatchOrganizationChange {
                entry_keys: vec!["artwork-1".into(), "novel-2".into()],
                update_collection: true,
                collection_id: Some(collection.id),
                add_tags: vec!["new".into()],
                remove_tags: vec!["old".into()],
            })
            .unwrap();
        assert_eq!(changed.len(), 2);
        assert!(changed
            .iter()
            .all(|entry| entry.collection_id == Some(collection.id)));
        assert!(changed
            .iter()
            .all(|entry| entry.tags.contains(&"new".to_owned())));
        assert!(changed
            .iter()
            .all(|entry| !entry.tags.contains(&"old".to_owned())));

        let saved = catalog
            .save_filter(&CatalogFilterDraft {
                name: "Large novels".into(),
                query: "chapter".into(),
                kind: Some("novel".into()),
                collection_id: Some(collection.id),
                tag: Some("keep".into()),
                sort_order: "size".into(),
                stored_after: Some(10),
                stored_before: Some(20),
                min_size_bytes: Some(1024),
                max_size_bytes: Some(2048),
            })
            .unwrap();
        let snapshot = catalog
            .snapshot(&["artwork-1".into(), "novel-2".into()])
            .unwrap();
        assert_eq!(snapshot.saved_filters, std::slice::from_ref(&saved));
        assert!(catalog.delete_filter(saved.id).unwrap());

        let before = catalog
            .snapshot(&["artwork-1".into(), "novel-2".into()])
            .unwrap();
        assert_eq!(
            catalog.batch_organize_entries(&BatchOrganizationChange {
                entry_keys: vec!["artwork-1".into(), "bad-key".into()],
                update_collection: false,
                collection_id: None,
                add_tags: vec!["partial".into()],
                remove_tags: vec![],
            }),
            Err(CatalogError::InvalidInput)
        );
        assert_eq!(
            catalog
                .snapshot(&["artwork-1".into(), "novel-2".into()])
                .unwrap(),
            before
        );

        let preserved = catalog
            .batch_organize_entries(&BatchOrganizationChange {
                entry_keys: vec!["artwork-1".into()],
                update_collection: false,
                collection_id: None,
                add_tags: vec!["preserved".into()],
                remove_tags: vec![],
            })
            .unwrap();
        assert_eq!(preserved[0].collection_id, Some(collection.id));
        fs::remove_dir_all(root).unwrap();
    }
}
