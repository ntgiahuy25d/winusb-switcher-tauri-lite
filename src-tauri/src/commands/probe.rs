//! Tauri commands for probe operations.
//! Thin wrappers that delegate to the jlink subsystem.

use tauri::State;
use crate::error::AppError;
use crate::jlink::{self, types::{Probe, UsbDriverMode, UsbDriverResult}};
use crate::state::JLinkState;

/// Combined detect + scan — called on app startup and after install.
#[tauri::command]
pub async fn detect_and_scan(
    state: State<'_, JLinkState>,
) -> Result<serde_json::Value, AppError> {
    log::info!("[cmd] detect_and_scan");

    let status = tokio::task::spawn_blocking(jlink::detect::detect).await?;

    if status.installed {
        // Always prefer the resolved path from detection (which will point at the bundled J-Link
        // once bootstrap has prepended the install dir to PATH).
        if let Some(path) = status.path.as_deref() {
            state.set(path.to_string());
        }
    }

    let probes = if status.installed {
        let bin = state.get();
        tokio::task::spawn_blocking(move || jlink::scan::scan_probes(&bin)).await??
    } else {
        vec![]
    };

    log::info!("[cmd] detect_and_scan complete — installed={} probes={}", status.installed, probes.len());
    Ok(serde_json::json!({ "status": status, "probes": probes }))
}

/// Scan probes only (J-Link already known to be installed).
#[tauri::command]
pub async fn scan_probes(
    state: State<'_, JLinkState>,
) -> Result<Vec<Probe>, AppError> {
    log::info!("[cmd] scan_probes");
    let bin = state.get();
    tokio::task::spawn_blocking(move || jlink::scan::scan_probes(&bin))
        .await?
}

/// Switch USB driver for the probe at given index.
/// mode: "winUsb" → WebUSBEnable, "segger" → WebUSBDisable
#[tauri::command]
pub async fn switch_usb_driver(
    probe_index: usize,
    mode: UsbDriverMode,
    state: State<'_, JLinkState>,
) -> Result<UsbDriverResult, AppError> {
    log::info!("[cmd] switch_usb_driver probe_index={} mode={:?}", probe_index, mode);
    let bin = state.get();
    Ok(tokio::task::spawn_blocking(move || jlink::usb_driver::switch(&bin, probe_index, mode))
        .await?)
}

/// Returns the compiled OS and CPU architecture of this binary.
/// Values come from `std::env::consts` so they always match the actual build target.
#[tauri::command]
pub fn get_arch_info() -> serde_json::Value {
    serde_json::json!({
        "os":   std::env::consts::OS,
        "arch": std::env::consts::ARCH,
    })
}