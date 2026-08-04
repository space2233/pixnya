use std::path::PathBuf;
use tauri::Manager;

#[cfg(debug_assertions)]
const TEST_ROOT_ENVIRONMENT: &str = "PIXIV_CLIENT_TEST_ROOT";

#[cfg(debug_assertions)]
pub(crate) fn isolated_test_root() -> Option<PathBuf> {
    let root = std::env::var_os(TEST_ROOT_ENVIRONMENT).map(PathBuf::from)?;
    root.is_absolute().then_some(root)
}

#[cfg(not(debug_assertions))]
pub(crate) fn isolated_test_root() -> Option<PathBuf> {
    None
}

pub(crate) fn app_data_dir(app: &tauri::AppHandle) -> tauri::Result<PathBuf> {
    if let Some(root) = isolated_test_root() {
        return Ok(root.join("data"));
    }
    app.path().app_data_dir()
}

pub(crate) fn app_cache_dir(app: &tauri::AppHandle) -> tauri::Result<PathBuf> {
    if let Some(root) = isolated_test_root() {
        return Ok(root.join("cache"));
    }
    app.path().app_cache_dir()
}
