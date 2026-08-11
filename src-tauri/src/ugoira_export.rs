use crate::{
    exports, offline_library, paths, perform_ugoira_download, storage_manager, ApiCommandError,
    AuthenticatedDataState, PreparedUgoira, PreparedUgoiraFrame, SessionState,
};
use pixiv_client_api::UgoiraMetadata;
use pixiv_client_library::OfflineKind;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::Manager;

const MAX_EXPORT_FRAMES: usize = 10_000;
const MAX_FRAME_DIMENSION: u64 = 8_192;
const MAX_DECODED_MEMORY_BYTES: u64 = 512 * 1024 * 1024;
const MAX_EXPORT_DURATION_MS: u64 = 4 * 60 * 60 * 1_000;
const EXPORT_STAGING_DIRECTORY: &str = "ugoira-export-v1";
static EXPORT_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UgoiraExportFormat {
    Gif,
    Apng,
    Webm,
}

impl UgoiraExportFormat {
    fn extension(self) -> &'static str {
        match self {
            Self::Gif => "gif",
            Self::Apng => "png",
            Self::Webm => "webm",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UgoiraExportPhase {
    Queued,
    Preparing,
    Encoding,
    Exporting,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UgoiraExportTask {
    id: String,
    illustration_id: String,
    format: UgoiraExportFormat,
    phase: UgoiraExportPhase,
    completed_units: u64,
    total_units: u64,
    destination: Option<String>,
    failure: Option<String>,
}

struct TaskRecord {
    snapshot: UgoiraExportTask,
    cancelled: Arc<AtomicBool>,
}

#[derive(Default)]
pub(crate) struct UgoiraExportState {
    tasks: Mutex<HashMap<String, TaskRecord>>,
}

impl UgoiraExportState {
    fn insert(
        &self,
        task: UgoiraExportTask,
        cancelled: Arc<AtomicBool>,
    ) -> Result<(), ApiCommandError> {
        let mut tasks = self
            .tasks
            .lock()
            .map_err(|_| ApiCommandError::StateUnavailable)?;
        if tasks.len() >= 32 {
            let finished = tasks
                .iter()
                .find(|(_, record)| {
                    matches!(
                        record.snapshot.phase,
                        UgoiraExportPhase::Completed
                            | UgoiraExportPhase::Failed
                            | UgoiraExportPhase::Cancelled
                    )
                })
                .map(|(id, _)| id.clone());
            if let Some(id) = finished {
                tasks.remove(&id);
            }
        }
        if tasks.len() >= 32 {
            return Err(ApiCommandError::StateUnavailable);
        }
        tasks.insert(
            task.id.clone(),
            TaskRecord {
                snapshot: task,
                cancelled,
            },
        );
        Ok(())
    }

    fn snapshot(&self, id: &str) -> Result<UgoiraExportTask, ApiCommandError> {
        self.tasks
            .lock()
            .map_err(|_| ApiCommandError::StateUnavailable)?
            .get(id)
            .map(|record| record.snapshot.clone())
            .ok_or(ApiCommandError::InvalidIdentifier)
    }

    fn update(&self, id: &str, update: impl FnOnce(&mut UgoiraExportTask)) {
        if let Ok(mut tasks) = self.tasks.lock() {
            if let Some(record) = tasks.get_mut(id) {
                update(&mut record.snapshot);
            }
        }
    }

    fn cancel(&self, id: &str) -> Result<UgoiraExportTask, ApiCommandError> {
        let mut tasks = self
            .tasks
            .lock()
            .map_err(|_| ApiCommandError::StateUnavailable)?;
        let record = tasks
            .get_mut(id)
            .ok_or(ApiCommandError::InvalidIdentifier)?;
        if matches!(
            record.snapshot.phase,
            UgoiraExportPhase::Completed | UgoiraExportPhase::Failed | UgoiraExportPhase::Cancelled
        ) {
            return Ok(record.snapshot.clone());
        }
        record.cancelled.store(true, Ordering::Release);
        Ok(record.snapshot.clone())
    }
}

#[tauri::command]
pub(crate) async fn start_ugoira_export(
    illustration_id: String,
    format: UgoiraExportFormat,
    app: tauri::AppHandle,
) -> Result<UgoiraExportTask, ApiCommandError> {
    let parsed = illustration_id
        .parse::<u64>()
        .map_err(|_| ApiCommandError::InvalidIdentifier)?;
    if parsed == 0 {
        return Err(ApiCommandError::InvalidIdentifier);
    }
    let id = EXPORT_SEQUENCE.fetch_add(1, Ordering::Relaxed).to_string();
    let cancelled = Arc::new(AtomicBool::new(false));
    let task = UgoiraExportTask {
        id: id.clone(),
        illustration_id: parsed.to_string(),
        format,
        phase: UgoiraExportPhase::Queued,
        completed_units: 0,
        total_units: 1,
        destination: None,
        failure: None,
    };
    app.state::<UgoiraExportState>()
        .insert(task.clone(), cancelled.clone())?;
    let worker_app = app.clone();
    tauri::async_runtime::spawn(async move {
        run_export(worker_app, id, parsed.to_string(), format, cancelled).await;
    });
    Ok(task)
}

#[tauri::command]
pub(crate) fn get_ugoira_export_task(
    task_id: String,
    state: tauri::State<'_, UgoiraExportState>,
) -> Result<UgoiraExportTask, ApiCommandError> {
    state.snapshot(&task_id)
}

#[tauri::command]
pub(crate) fn cancel_ugoira_export_task(
    task_id: String,
    state: tauri::State<'_, UgoiraExportState>,
) -> Result<UgoiraExportTask, ApiCommandError> {
    state.cancel(&task_id)
}

async fn run_export(
    app: tauri::AppHandle,
    task_id: String,
    illustration_id: String,
    format: UgoiraExportFormat,
    cancelled: Arc<AtomicBool>,
) {
    let state = app.state::<UgoiraExportState>();
    state.update(&task_id, |task| task.phase = UgoiraExportPhase::Preparing);
    let result = run_export_inner(&app, &task_id, &illustration_id, format, &cancelled).await;
    state.update(&task_id, |task| match result {
        Ok(destination) => {
            task.phase = UgoiraExportPhase::Completed;
            task.completed_units = task.total_units;
            task.destination = Some(destination);
        }
        Err("cancelled") => {
            task.phase = UgoiraExportPhase::Cancelled;
            task.failure = Some("cancelled".into());
        }
        Err(failure) => {
            task.phase = UgoiraExportPhase::Failed;
            task.failure = Some(failure.into());
        }
    });
}

async fn run_export_inner(
    app: &tauri::AppHandle,
    task_id: &str,
    illustration_id: &str,
    format: UgoiraExportFormat,
    cancelled: &Arc<AtomicBool>,
) -> Result<String, &'static str> {
    checkpoint(cancelled)?;
    let prepared = match load_prepared_ugoira(app, illustration_id) {
        Ok(value) => value,
        Err(_) => {
            let session = app.state::<SessionState>();
            let data = app.state::<AuthenticatedDataState>().inner().clone();
            perform_ugoira_download(
                illustration_id.to_owned(),
                app,
                &session,
                data,
                None,
                Some(cancelled.clone()),
            )
            .await
            .map_err(|_| {
                if cancelled.load(Ordering::Acquire) {
                    "cancelled"
                } else {
                    "download_failed"
                }
            })?
        }
    };
    let total_duration_ms = validate_frames(&prepared.frames)?;
    let staging_root = paths::app_cache_dir(app)
        .map_err(|_| "staging_unavailable")?
        .join(EXPORT_STAGING_DIRECTORY);
    fs::create_dir_all(&staging_root).map_err(|_| "staging_unavailable")?;
    let task_root = staging_root.join(format!("task-{task_id}"));
    if task_root.exists() {
        fs::remove_dir_all(&task_root).map_err(|_| "staging_cleanup_failed")?;
    }
    fs::create_dir(&task_root).map_err(|_| "staging_unavailable")?;
    let worker_app = app.clone();
    let worker_task_id = task_id.to_owned();
    let worker_task_root = task_root.clone();
    let worker_cancelled = cancelled.clone();
    let result = match tauri::async_runtime::spawn_blocking(move || {
        prepare_and_encode(
            &worker_app,
            &prepared,
            format,
            total_duration_ms,
            &worker_task_root,
            &worker_task_id,
            &worker_cancelled,
        )
    })
    .await
    {
        Ok(Ok(output)) => {
            if let Err(error) = checkpoint(cancelled) {
                Err(error)
            } else {
                app.state::<UgoiraExportState>()
                    .update(task_id, |task| task.phase = UgoiraExportPhase::Exporting);
                match output.file_name().and_then(|value| value.to_str()) {
                    Some(file_name) => exports::export_generated_file(app, &output, file_name)
                        .await
                        .map_err(|_| "export_failed"),
                    None => Err("export_failed"),
                }
            }
        }
        Ok(Err(error)) => Err(error),
        Err(_) => Err("encoding_failed"),
    };
    let _ = fs::remove_dir_all(&task_root);
    result
}

fn prepare_and_encode(
    app: &tauri::AppHandle,
    prepared: &PreparedUgoira,
    format: UgoiraExportFormat,
    total_duration_ms: u64,
    task_root: &Path,
    task_id: &str,
    cancelled: &Arc<AtomicBool>,
) -> Result<PathBuf, &'static str> {
    let library = offline_library(app).map_err(|_| "offline_unavailable")?;
    let total_input_bytes = prepared.entry.size_bytes;
    let estimated_output = total_input_bytes.saturating_mul(2).max(16 * 1024 * 1024);
    let storage = storage_manager(app).map_err(|_| "storage_unavailable")?;
    let status = storage.status().map_err(|_| "storage_unavailable")?;
    let required = total_input_bytes.saturating_add(estimated_output);
    if status.cache_available_bytes < required.saturating_add(status.reserve_bytes) {
        return Err("insufficient_space");
    }

    let input_root = task_root.join("frames");
    fs::create_dir(&input_root).map_err(|_| "staging_unavailable")?;
    let first_frame = prepared.frames.first().ok_or("frame_limit")?;
    checkpoint(cancelled)?;
    let first_asset = library
        .read_asset(&prepared.entry.key, &first_frame.asset_name)
        .map_err(|_| "frame_unavailable")?;
    let first_path = input_root.join("frame-000000.img");
    fs::write(&first_path, first_asset.bytes).map_err(|_| "staging_unavailable")?;
    let (width, height) = probe_dimensions(&first_path)?;
    validate_dimensions(width, height, prepared.frames.len())?;
    for (index, frame) in prepared.frames.iter().enumerate().skip(1) {
        checkpoint(cancelled)?;
        let asset = library
            .read_asset(&prepared.entry.key, &frame.asset_name)
            .map_err(|_| "frame_unavailable")?;
        fs::write(
            input_root.join(format!("frame-{index:06}.img")),
            asset.bytes,
        )
        .map_err(|_| "staging_unavailable")?;
    }
    checkpoint(cancelled)?;

    let concat = task_root.join("frames.txt");
    write_concat_file(&concat, &input_root, &prepared.frames).map_err(|_| "staging_unavailable")?;
    let output = task_root.join(format!(
        "ugoira-{}.{}",
        prepared.entry.resource_id,
        format.extension()
    ));
    let progress = task_root.join("progress.txt");
    app.state::<UgoiraExportState>().update(task_id, |task| {
        task.phase = UgoiraExportPhase::Encoding;
        task.total_units = total_duration_ms.max(1);
    });
    encode_with_ffmpeg(
        &concat,
        &output,
        &progress,
        format,
        total_duration_ms,
        cancelled,
        |value| {
            app.state::<UgoiraExportState>().update(task_id, |task| {
                task.completed_units = value.min(task.total_units)
            });
        },
    )?;
    checkpoint(cancelled)?;
    Ok(output)
}

fn load_prepared_ugoira(
    app: &tauri::AppHandle,
    illustration_id: &str,
) -> Result<PreparedUgoira, ApiCommandError> {
    let library = offline_library(app)?;
    let key = format!("ugoira-{illustration_id}");
    let entry = library
        .list_entries()?
        .into_iter()
        .find(|entry| entry.key == key && entry.kind == OfflineKind::Ugoira)
        .ok_or(ApiCommandError::OfflineNotFound)?;
    let metadata_asset = library.read_asset(&entry.key, "metadata.json")?;
    let metadata: UgoiraMetadata = serde_json::from_slice(&metadata_asset.bytes)
        .map_err(|_| ApiCommandError::InvalidResponse)?;
    let frames = metadata
        .frames
        .into_iter()
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
    Ok(PreparedUgoira { entry, frames })
}

fn validate_frames(frames: &[PreparedUgoiraFrame]) -> Result<u64, &'static str> {
    if frames.is_empty() || frames.len() > MAX_EXPORT_FRAMES {
        return Err("frame_limit");
    }
    frames.iter().try_fold(0_u64, |total, frame| {
        if !(1..=60_000).contains(&frame.delay_ms) {
            return Err("invalid_frame_delay");
        }
        let next = total.saturating_add(frame.delay_ms as u64);
        if next > MAX_EXPORT_DURATION_MS {
            Err("duration_limit")
        } else {
            Ok(next)
        }
    })
}

fn validate_dimensions(width: u64, height: u64, frames: usize) -> Result<(), &'static str> {
    if width == 0 || height == 0 || width > MAX_FRAME_DIMENSION || height > MAX_FRAME_DIMENSION {
        return Err("dimension_limit");
    }
    let working_frames = u64::try_from(frames.min(8)).map_err(|_| "memory_limit")?;
    let decoded = width
        .saturating_mul(height)
        .saturating_mul(4)
        .saturating_mul(working_frames);
    if decoded > MAX_DECODED_MEMORY_BYTES {
        Err("memory_limit")
    } else {
        Ok(())
    }
}

fn checkpoint(cancelled: &AtomicBool) -> Result<(), &'static str> {
    if cancelled.load(Ordering::Acquire) {
        Err("cancelled")
    } else {
        Ok(())
    }
}

fn ffmpeg_program() -> String {
    std::env::var("PIXNYA_FFMPEG_PATH").unwrap_or_else(|_| "ffmpeg".into())
}
fn ffprobe_program() -> String {
    std::env::var("PIXNYA_FFPROBE_PATH").unwrap_or_else(|_| "ffprobe".into())
}

fn probe_dimensions(frame: &Path) -> Result<(u64, u64), &'static str> {
    let output = Command::new(ffprobe_program())
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height",
            "-of",
            "csv=p=0:s=x",
        ])
        .arg(frame)
        .output()
        .map_err(|_| "encoder_unavailable")?;
    if !output.status.success() {
        return Err("unsupported_frame_format");
    }
    let value = String::from_utf8(output.stdout).map_err(|_| "unsupported_frame_format")?;
    let (width, height) = value
        .trim()
        .split_once('x')
        .ok_or("unsupported_frame_format")?;
    Ok((
        width.parse().map_err(|_| "unsupported_frame_format")?,
        height.parse().map_err(|_| "unsupported_frame_format")?,
    ))
}

fn write_concat_file(
    path: &Path,
    input_root: &Path,
    frames: &[PreparedUgoiraFrame],
) -> std::io::Result<()> {
    let mut content = String::from("ffconcat version 1.0\n");
    for (index, frame) in frames.iter().enumerate() {
        let file = input_root
            .join(format!("frame-{index:06}.img"))
            .to_string_lossy()
            .replace('\\', "/")
            .replace('\'', "'\\''");
        content.push_str(&format!(
            "file '{file}'\nduration {:.6}\n",
            frame.delay_ms as f64 / 1000.0
        ));
    }
    let last = input_root
        .join(format!("frame-{:06}.img", frames.len() - 1))
        .to_string_lossy()
        .replace('\\', "/")
        .replace('\'', "'\\''");
    content.push_str(&format!("file '{last}'\n"));
    fs::write(path, content)
}

fn encode_with_ffmpeg(
    concat: &Path,
    output: &Path,
    progress: &Path,
    format: UgoiraExportFormat,
    total_duration_ms: u64,
    cancelled: &AtomicBool,
    mut update: impl FnMut(u64),
) -> Result<(), &'static str> {
    let mut command = Command::new(ffmpeg_program());
    command
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-y",
            "-f",
            "concat",
            "-safe",
            "0",
            "-i",
        ])
        .arg(concat)
        .args(["-vsync", "vfr", "-progress"])
        .arg(progress);
    match format {
        UgoiraExportFormat::Gif => {
            command.args([
                "-filter_complex",
                "split[s0][s1];[s0]palettegen[p];[s1][p]paletteuse",
                "-loop",
                "0",
            ]);
        }
        UgoiraExportFormat::Apng => {
            command.args(["-plays", "0", "-f", "apng"]);
        }
        UgoiraExportFormat::Webm => {
            command.args([
                "-an",
                "-c:v",
                "libvpx-vp9",
                "-pix_fmt",
                "yuv420p",
                "-deadline",
                "good",
            ]);
        }
    }
    let mut child = command
        .arg(output)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "encoder_unavailable")?;
    loop {
        if cancelled.load(Ordering::Acquire) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = fs::remove_file(output);
            return Err("cancelled");
        }
        if let Some(status) = child.try_wait().map_err(|_| "encoding_failed")? {
            if !status.success() {
                let _ = fs::remove_file(output);
                return Err("encoding_failed");
            }
            break;
        }
        if let Ok(value) = fs::read_to_string(progress) {
            if let Some(microseconds) = value.lines().rev().find_map(|line| {
                line.strip_prefix("out_time_us=")
                    .and_then(|v| v.parse::<u64>().ok())
            }) {
                update((microseconds / 1_000).min(total_duration_ms));
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let metadata = fs::metadata(output).map_err(|_| "encoding_failed")?;
    if metadata.len() == 0 {
        Err("encoding_failed")
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frames(count: usize, delay_ms: u32) -> Vec<PreparedUgoiraFrame> {
        (0..count)
            .map(|index| PreparedUgoiraFrame {
                asset_name: format!("frame-{index}.jpg"),
                delay_ms,
            })
            .collect()
    }

    #[test]
    fn validates_timing_and_resource_budgets() {
        assert_eq!(validate_frames(&frames(2, 100)), Ok(200));
        assert_eq!(validate_frames(&[]), Err("frame_limit"));
        assert_eq!(validate_frames(&frames(1, 0)), Err("invalid_frame_delay"));
        assert_eq!(validate_dimensions(100, 100, 5), Ok(()));
        assert_eq!(validate_dimensions(9_000, 100, 1), Err("dimension_limit"));
        assert_eq!(validate_dimensions(8_192, 8_192, 8), Err("memory_limit"));
    }
}
