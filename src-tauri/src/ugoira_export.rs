use crate::{
    exports, offline_library, paths, perform_ugoira_download, storage_manager, ApiCommandError,
    AuthenticatedDataState, PreparedUgoira, PreparedUgoiraFrame, SessionState,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::Manager;

const MAX_EXPORT_FRAMES: usize = 10_000;
const MAX_FRAME_DIMENSION: u64 = 8_192;
const MAX_DECODED_MEMORY_BYTES: u64 = 512 * 1024 * 1024;
const MAX_EXPORT_DURATION_MS: u64 = 4 * 60 * 60 * 1_000;
const FRAME_PROBE_TIMEOUT: Duration = Duration::from_secs(5);
const MIN_FRAME_VALIDATION_TIMEOUT: Duration = Duration::from_secs(2 * 60);
const FRAME_VALIDATION_BUDGET_PER_FRAME: Duration = Duration::from_millis(300);
const MAX_FRAME_VALIDATION_TIMEOUT: Duration = Duration::from_secs(45 * 60);
const MIN_ENCODING_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const MAX_ENCODING_TIMEOUT: Duration = Duration::from_secs(6 * 60 * 60);
const PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(100);
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
    let mut frame_paths = Vec::with_capacity(prepared.frames.len());
    for (index, frame) in prepared.frames.iter().enumerate() {
        checkpoint(cancelled)?;
        let asset = library
            .read_asset(&prepared.entry.key, &frame.asset_name)
            .map_err(|_| "frame_unavailable")?;
        let frame_path = input_root.join(format!("frame-{index:06}.img"));
        fs::write(&frame_path, asset.bytes).map_err(|_| "staging_unavailable")?;
        frame_paths.push(frame_path);
    }
    checkpoint(cancelled)?;
    validate_all_frame_dimensions(&frame_paths, cancelled)?;

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
    crate::try_existing_prepared_ugoira(app, illustration_id)
        .ok_or(ApiCommandError::OfflineNotFound)
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

fn validate_all_frame_dimensions(
    frames: &[PathBuf],
    cancelled: &AtomicBool,
) -> Result<(), &'static str> {
    let deadline = Instant::now()
        .checked_add(frame_validation_timeout(frames.len()))
        .ok_or("frame_validation_timeout")?;
    validate_all_frame_dimensions_with(frames, cancelled, deadline, |frame, probe_deadline| {
        probe_dimensions(frame, cancelled, probe_deadline)
    })
}

fn frame_validation_timeout(frame_count: usize) -> Duration {
    let bounded_frame_count = frame_count.min(MAX_EXPORT_FRAMES) as u32;
    MIN_FRAME_VALIDATION_TIMEOUT
        .saturating_add(FRAME_VALIDATION_BUDGET_PER_FRAME.saturating_mul(bounded_frame_count))
        .min(MAX_FRAME_VALIDATION_TIMEOUT)
}

fn validate_all_frame_dimensions_with(
    frames: &[PathBuf],
    cancelled: &AtomicBool,
    deadline: Instant,
    mut probe: impl FnMut(&Path, Instant) -> Result<(u64, u64), &'static str>,
) -> Result<(), &'static str> {
    for frame in frames {
        checkpoint(cancelled)?;
        let now = Instant::now();
        if now >= deadline {
            return Err("frame_validation_timeout");
        }
        let probe_deadline = now
            .checked_add(FRAME_PROBE_TIMEOUT)
            .map(|value| value.min(deadline))
            .unwrap_or(deadline);
        let (width, height) = probe(frame, probe_deadline)?;
        validate_dimensions(width, height, frames.len())?;
    }
    Ok(())
}

fn probe_dimensions(
    frame: &Path,
    cancelled: &AtomicBool,
    deadline: Instant,
) -> Result<(u64, u64), &'static str> {
    let mut child = Command::new(ffprobe_program())
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
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "encoder_unavailable")?;
    wait_for_child_until(
        &mut child,
        cancelled,
        deadline,
        "frame_validation_timeout",
        "unsupported_frame_format",
        || {},
    )?;
    let mut stdout = child.stdout.take().ok_or("unsupported_frame_format")?;
    let mut bytes = Vec::with_capacity(32);
    stdout
        .read_to_end(&mut bytes)
        .map_err(|_| "unsupported_frame_format")?;
    let value = String::from_utf8(bytes).map_err(|_| "unsupported_frame_format")?;
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
    let deadline = Instant::now()
        .checked_add(encoding_timeout(total_duration_ms))
        .ok_or("encoding_timeout")?;
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
    let result = wait_for_child_until(
        &mut child,
        cancelled,
        deadline,
        "encoding_timeout",
        "encoding_failed",
        || {
            if let Ok(value) = fs::read_to_string(progress) {
                if let Some(microseconds) = value.lines().rev().find_map(|line| {
                    line.strip_prefix("out_time_us=")
                        .and_then(|v| v.parse::<u64>().ok())
                }) {
                    update((microseconds / 1_000).min(total_duration_ms));
                }
            }
        },
    );
    if let Err(error) = result {
        let _ = fs::remove_file(output);
        return Err(error);
    }
    let metadata = fs::metadata(output).map_err(|_| "encoding_failed")?;
    if metadata.len() == 0 {
        Err("encoding_failed")
    } else {
        Ok(())
    }
}

fn encoding_timeout(total_duration_ms: u64) -> Duration {
    let content_budget = Duration::from_millis(total_duration_ms).saturating_mul(4);
    MIN_ENCODING_TIMEOUT
        .saturating_add(content_budget)
        .min(MAX_ENCODING_TIMEOUT)
}

fn wait_for_child_until(
    child: &mut Child,
    cancelled: &AtomicBool,
    deadline: Instant,
    timeout_error: &'static str,
    failure_error: &'static str,
    mut poll: impl FnMut(),
) -> Result<(), &'static str> {
    loop {
        if cancelled.load(Ordering::Acquire) {
            terminate_and_reap(child);
            return Err("cancelled");
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                return if status.success() {
                    Ok(())
                } else {
                    Err(failure_error)
                };
            }
            Ok(None) => {}
            Err(_) => {
                terminate_and_reap(child);
                return Err(failure_error);
            }
        }
        if Instant::now() >= deadline {
            terminate_and_reap(child);
            return Err(timeout_error);
        }
        poll();
        std::thread::sleep(PROCESS_POLL_INTERVAL);
    }
}

fn terminate_and_reap(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::time::Instant;

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

    #[test]
    fn frame_validation_budget_scales_with_frame_count_and_stays_bounded() {
        assert_eq!(
            frame_validation_timeout(100),
            Duration::from_secs(2 * 60 + 30)
        );
        assert_eq!(
            frame_validation_timeout(MAX_EXPORT_FRAMES),
            Duration::from_secs(45 * 60)
        );
        assert_eq!(
            frame_validation_timeout(usize::MAX),
            Duration::from_secs(45 * 60)
        );
    }

    #[test]
    fn validates_dimensions_for_every_frame() {
        let paths = [PathBuf::from("first"), PathBuf::from("second")];
        let probes = AtomicUsize::new(0);
        let cancelled = AtomicBool::new(false);

        let result = validate_all_frame_dimensions_with(
            &paths,
            &cancelled,
            Instant::now() + Duration::from_secs(1),
            |path, _| {
                probes.fetch_add(1, Ordering::Relaxed);
                if path == Path::new("second") {
                    Ok((MAX_FRAME_DIMENSION + 1, 100))
                } else {
                    Ok((100, 100))
                }
            },
        );

        assert_eq!(result, Err("dimension_limit"));
        assert_eq!(probes.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn accepts_ordinary_dimensions_after_validating_all_frames() {
        let paths = [PathBuf::from("first"), PathBuf::from("second")];
        let probes = AtomicUsize::new(0);
        let cancelled = AtomicBool::new(false);

        let result = validate_all_frame_dimensions_with(
            &paths,
            &cancelled,
            Instant::now() + Duration::from_secs(1),
            |_, _| {
                probes.fetch_add(1, Ordering::Relaxed);
                Ok((1_920, 1_080))
            },
        );

        assert_eq!(result, Ok(()));
        assert_eq!(probes.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn frame_validation_obeys_total_deadline_before_starting_more_work() {
        let paths = [PathBuf::from("first")];
        let probes = AtomicUsize::new(0);
        let cancelled = AtomicBool::new(false);

        let result =
            validate_all_frame_dimensions_with(&paths, &cancelled, Instant::now(), |_, _| {
                probes.fetch_add(1, Ordering::Relaxed);
                Ok((100, 100))
            });

        assert_eq!(result, Err("frame_validation_timeout"));
        assert_eq!(probes.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn child_process_deadline_kills_and_reaps_the_process() {
        let mut child = spawn_delayed_test_child();
        let cancelled = AtomicBool::new(false);
        let started = Instant::now();

        let result = wait_for_child_until(
            &mut child,
            &cancelled,
            Instant::now() + Duration::from_millis(100),
            "test_timeout",
            "test_failed",
            || {},
        );

        assert_eq!(result, Err("test_timeout"));
        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(child.try_wait().expect("query reaped child").is_some());
    }

    #[test]
    fn child_process_cancellation_kills_and_reaps_the_process() {
        let mut child = spawn_delayed_test_child();
        let cancelled = AtomicBool::new(true);

        let result = wait_for_child_until(
            &mut child,
            &cancelled,
            Instant::now() + Duration::from_secs(2),
            "test_timeout",
            "test_failed",
            || {},
        );

        assert_eq!(result, Err("cancelled"));
        assert!(child.try_wait().expect("query reaped child").is_some());
    }

    #[test]
    fn child_process_success_is_preserved() {
        let mut child = spawn_test_child(None);
        let cancelled = AtomicBool::new(false);

        let result = wait_for_child_until(
            &mut child,
            &cancelled,
            Instant::now() + Duration::from_secs(2),
            "test_timeout",
            "test_failed",
            || {},
        );

        assert_eq!(result, Ok(()));
        assert!(child.try_wait().expect("query reaped child").is_some());
    }

    fn spawn_delayed_test_child() -> Child {
        spawn_test_child(Some("5000"))
    }

    fn spawn_test_child(delay_ms: Option<&str>) -> Child {
        let mut command = Command::new(std::env::current_exe().expect("test executable"));
        command
            .args([
                "--exact",
                "ugoira_export::tests::child_process_timeout_fixture",
                "--nocapture",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if let Some(delay_ms) = delay_ms {
            command.env("PIXNYA_TEST_CHILD_DELAY_MS", delay_ms);
        }
        command.spawn().expect("spawn timeout fixture")
    }

    #[test]
    fn child_process_timeout_fixture() {
        if let Ok(delay) = std::env::var("PIXNYA_TEST_CHILD_DELAY_MS") {
            std::thread::sleep(Duration::from_millis(
                delay.parse().expect("valid fixture delay"),
            ));
        }
    }
}
