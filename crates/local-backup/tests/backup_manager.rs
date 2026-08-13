use pixiv_client_local_backup::{
    BackupCreateRequest, BackupError, BackupManager, FrontendBackupState, OfflineBackupFile,
    OfflineBackupSource, PortableBackupData,
};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write;
use zip::write::SimpleFileOptions;

fn sample_data() -> PortableBackupData {
    PortableBackupData {
        frontend: FrontendBackupState {
            search_history: vec!["猫".to_owned()],
            novel_reading_progress: BTreeMap::from([("42".to_owned(), 730_000)]),
            sidebar_expanded: false,
            reduced_motion: true,
            r18_default_visible: false,
        },
        catalog: serde_json::json!({"collections": [], "entries": [], "savedFilters": []}),
        history: serde_json::json!({"enabled": true, "entries": []}),
        downloads: serde_json::json!({"tasks": []}),
    }
}

fn rewrite_member(path: &std::path::Path, target: &str, replacement: &[u8]) {
    let source = File::open(path).unwrap();
    let mut archive = zip::ZipArchive::new(source).unwrap();
    let rewritten = path.with_extension("rewritten");
    let mut writer = zip::ZipWriter::new(File::create(&rewritten).unwrap());
    for index in 0..archive.len() {
        let mut member = archive.by_index(index).unwrap();
        let name = member.name().to_owned();
        writer
            .start_file(&name, SimpleFileOptions::default())
            .unwrap();
        if name == target {
            writer.write_all(replacement).unwrap();
        } else {
            std::io::copy(&mut member, &mut writer).unwrap();
        }
    }
    writer.finish().unwrap();
    std::fs::rename(rewritten, path).unwrap();
}

#[test]
fn creates_inspects_and_restores_a_portable_backup() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("portable.pixnyabackup");
    let manager = BackupManager::new("1.4.0");
    let data = sample_data();

    let created = manager
        .create(
            &path,
            BackupCreateRequest {
                data: data.clone(),
                include_offline: false,
                offline_files: vec![],
            },
        )
        .unwrap();
    assert_eq!(created.format_version, 1);
    assert_eq!(created.offline_file_count, 0);

    let preview = manager.inspect(&path).unwrap();
    assert_eq!(preview.application_version, "1.4.0");
    assert_eq!(preview.component_count, 4);
    assert!(!preview.contains_credentials);

    let restored = manager.restore(&path).unwrap();
    assert_eq!(restored.data, data);
    assert!(restored.offline_files.is_empty());
}

#[test]
fn rejects_tampered_members_before_returning_restore_data() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("tampered.pixnyabackup");
    BackupManager::new("1.4.0")
        .create(
            &path,
            BackupCreateRequest {
                data: sample_data(),
                include_offline: false,
                offline_files: vec![],
            },
        )
        .unwrap();

    rewrite_member(&path, "components/history.json", br#"{"enabled":false}"#);

    assert!(matches!(
        BackupManager::new("1.4.0").restore(&path),
        Err(pixiv_client_local_backup::BackupError::IntegrityMismatch)
    ));
}

#[test]
fn rejects_future_versions_before_reading_components() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("future.pixnyabackup");
    BackupManager::new("1.4.0")
        .create(
            &path,
            BackupCreateRequest {
                data: sample_data(),
                include_offline: false,
                offline_files: vec![],
            },
        )
        .unwrap();
    let source = File::open(&path).unwrap();
    let mut archive = zip::ZipArchive::new(source).unwrap();
    let mut manifest = String::new();
    std::io::Read::read_to_string(
        &mut archive.by_name("manifest.json").unwrap(),
        &mut manifest,
    )
    .unwrap();
    let replacement = manifest.replace("\"formatVersion\":1", "\"formatVersion\":999");
    drop(archive);
    rewrite_member(&path, "manifest.json", replacement.as_bytes());

    assert!(matches!(
        BackupManager::new("1.4.0").inspect(&path),
        Err(pixiv_client_local_backup::BackupError::UnsupportedVersion)
    ));
}

#[test]
fn rejects_offline_members_when_the_manifest_does_not_include_offline_data() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("unexpected-offline.pixnyabackup");
    let result = BackupManager::new("1.4.0").create(
        &path,
        BackupCreateRequest {
            data: sample_data(),
            include_offline: false,
            offline_files: vec![OfflineBackupFile {
                relative_path: "illust/42/page-0.jpg".into(),
                bytes: b"image".to_vec(),
            }],
        },
    );
    assert_eq!(result, Err(BackupError::InvalidInput));
}

#[test]
fn streams_offline_sources_to_and_from_a_verified_directory() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("page-0.jpg");
    std::fs::write(&source, b"offline-image").unwrap();
    let backup = directory.path().join("offline.pixnyabackup");
    let manager = BackupManager::new("1.4.0");

    let summary = manager
        .create_from_sources(
            &backup,
            sample_data(),
            true,
            vec![OfflineBackupSource {
                relative_path: "artwork-42/page-0.jpg".into(),
                source_path: source,
            }],
        )
        .unwrap();
    assert!(summary.offline_included);
    assert_eq!(summary.offline_file_count, 1);

    let restored_root = directory.path().join("restored-offline");
    let restored = manager
        .restore_to_directory(&backup, &restored_root)
        .unwrap();
    assert_eq!(restored, sample_data());
    assert_eq!(
        std::fs::read(restored_root.join("artwork-42/page-0.jpg")).unwrap(),
        b"offline-image"
    );
}
