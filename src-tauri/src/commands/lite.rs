//! WinUSB Switcher Lite–only commands.

use tauri::{AppHandle, State};

use crate::platform;
use crate::state::JLinkState;

/// Extract bundled J-Link to the user profile (if needed) and prepend it to PATH.
/// Runs blocking work on the blocking thread pool so the UI can load first.
#[tauri::command]
pub async fn prepare_bundled_jlink(app: AppHandle, state: State<'_, JLinkState>) -> Result<String, String> {
    let app = app.clone();
    let install_dir = tokio::task::spawn_blocking(move || crate::bundled_jlink::ensure_extracted_and_on_path(&app))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;

    // Prefer executing the full path to avoid any PATH ambiguity.
    let exe = platform::config().jlink_executable;
    let full_bin = install_dir.join(exe).to_string_lossy().into_owned();
    state.set(full_bin.clone());
    Ok(full_bin)
}
