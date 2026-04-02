//! Bundled J-Link runtime for WinUSB Switcher Lite.
//!
//! Lite builds ship with a specific J-Link distribution embedded in the app bundle.
//! On first run, we extract it into a user-writable location and prepend it to the
//! current process PATH so all J-Link invocations work normally.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};
use crate::platform;

const BUNDLED_DIR_NAME: &str = "JLink_V930a";
const BUNDLED_ZIP_NAME: &str = "JLink_V930a.zip";

#[derive(Clone, Copy)]
enum BundledArch {
    X86_64,
    Aarch64,
    X86,
    Arm,
}

impl BundledArch {
    fn from_rust_arch() -> Option<Self> {
        match std::env::consts::ARCH {
            "x86_64" => Some(Self::X86_64),
            "aarch64" => Some(Self::Aarch64),
            "x86" => Some(Self::X86),
            "arm" => Some(Self::Arm),
            _ => None,
        }
    }

    fn as_dir_name(self) -> &'static str {
        match self {
            Self::X86_64 => "x86_64",
            Self::Aarch64 => "aarch64",
            Self::X86 => "x86",
            Self::Arm => "arm",
        }
    }
}

#[cfg(target_os = "windows")]
fn segger_roaming_dir() -> Option<PathBuf> {
    std::env::var("USERPROFILE")
        .ok()
        .map(|p| PathBuf::from(p).join("AppData").join("Roaming").join("SEGGER"))
}

fn bundled_zip_path(app: &AppHandle, os: &str, arch: BundledArch) -> AppResult<PathBuf> {
    let res_dir = app
        .path()
        .resource_dir()
        .map_err(|e| AppError::Internal(format!("resource_dir: {}", e)))?;

    // Depending on platform/build tooling, resources may be nested under `resources/`.
    let candidates = [
        res_dir
            .join("resources")
            .join("jlink")
            .join(os)
            .join(arch.as_dir_name())
            .join(BUNDLED_ZIP_NAME),
        res_dir
            .join("jlink")
            .join(os)
            .join(arch.as_dir_name())
            .join(BUNDLED_ZIP_NAME),
    ];

    for c in candidates {
        if c.is_file() {
            return Ok(c);
        }
    }

    Err(AppError::Internal(format!(
        "Bundled J-Link zip not found in resources (looked under {})",
        res_dir.display()
    )))
}

fn safe_join(base: &Path, rel: &Path) -> Option<PathBuf> {
    // Prevent Zip Slip: reject absolute paths and path traversal.
    if rel.is_absolute() {
        return None;
    }
    let mut out = PathBuf::from(base);
    for comp in rel.components() {
        match comp {
            std::path::Component::Normal(c) => out.push(c),
            std::path::Component::CurDir => {}
            _ => return None,
        }
    }
    Some(out)
}

pub fn extract_zip(zip_path: &Path, dst_dir: &Path) -> AppResult<()> {
    std::fs::create_dir_all(dst_dir).map_err(|e| AppError::Io(e.to_string()))?;

    // Quick sanity-check: users cloning via Git without LFS will get a tiny text pointer file,
    // not the actual zip payload. Detect and return an actionable error message.
    let mut header = [0u8; 64];
    let mut header_file =
        std::fs::File::open(zip_path).map_err(|e| AppError::Io(e.to_string()))?;
    let n = header_file
        .read(&mut header)
        .map_err(|e| AppError::Io(e.to_string()))?;
    let header_str = String::from_utf8_lossy(&header[..n]);
    if header_str.starts_with("version https://git-lfs.github.com/spec/v1") {
        return Err(AppError::Platform(format!(
            "Bundled J-Link payload is missing (Git LFS pointer file detected).\n\
            If you cloned the repo, install Git LFS and run:\n\
            \n\
              git lfs install\n\
              git lfs pull\n\
            \n\
            Then rebuild the app."
        )));
    }
    if n >= 2 && &header[..2] != b"PK" {
        return Err(AppError::Internal(format!(
            "Bundled J-Link zip is invalid or incomplete: {}",
            zip_path.display()
        )));
    }

    let f = std::fs::File::open(zip_path).map_err(|e| AppError::Io(e.to_string()))?;
    let mut archive = zip::ZipArchive::new(f).map_err(|e| AppError::Internal(e.to_string()))?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let name = file.name().to_string();
        let rel = Path::new(&name);
        let out_path = safe_join(dst_dir, rel)
            .ok_or_else(|| AppError::Internal(format!("Unsafe zip entry path: {}", name)))?;

        if file.is_dir() {
            std::fs::create_dir_all(&out_path).map_err(|e| AppError::Io(e.to_string()))?;
            continue;
        }

        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| AppError::Io(e.to_string()))?;
        }

        let mut out = std::fs::File::create(&out_path).map_err(|e| AppError::Io(e.to_string()))?;
        let mut buf = Vec::with_capacity(file.size().min(1024 * 1024) as usize);
        file.read_to_end(&mut buf)
            .map_err(|e| AppError::Io(e.to_string()))?;
        out.write_all(&buf).map_err(|e| AppError::Io(e.to_string()))?;
    }

    Ok(())
}

#[cfg(unix)]
fn set_exec_bit(path: &Path) -> AppResult<()> {
    use std::os::unix::fs::PermissionsExt;
    let meta = std::fs::metadata(path).map_err(|e| AppError::Io(e.to_string()))?;
    let mut perms = meta.permissions();
    perms.set_mode(perms.mode() | 0o111);
    std::fs::set_permissions(path, perms).map_err(|e| AppError::Io(e.to_string()))?;
    Ok(())
}

/// Linux install root is `/opt/SEGGER`. The zip may unpack either:
/// - flat: `/opt/SEGGER/JLinkExe`, or
/// - nested: `/opt/SEGGER/JLink_V930a/JLinkExe` (legacy layout).
#[cfg(target_os = "linux")]
fn linux_jlink_exe_candidates(dst_root: &Path) -> [PathBuf; 2] {
    [
        dst_root.join("JLinkExe"),
        dst_root.join(BUNDLED_DIR_NAME).join("JLinkExe"),
    ]
}

#[cfg(target_os = "linux")]
fn linux_resolve_jlink_exe(dst_root: &Path) -> Option<PathBuf> {
    for p in linux_jlink_exe_candidates(dst_root) {
        if p.exists() {
            return Some(p);
        }
    }
    None
}

#[cfg(target_os = "linux")]
pub fn linux_post_extract_fixups(dst_root: &Path) -> AppResult<()> {
    // The DEB-derived folder often loses executable bits when we package/extract via zip.
    for p in linux_jlink_exe_candidates(dst_root) {
        if p.exists() {
            if let Err(e) = set_exec_bit(&p) {
                log::warn!("[jlink] chmod {}: {}", p.display(), e);
            }
        }
    }
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn ensure_extracted_and_on_path(app: &AppHandle) -> AppResult<PathBuf> {
    let arch = BundledArch::from_rust_arch()
        .ok_or_else(|| AppError::Internal("Unsupported CPU architecture".to_string()))?;
    let zip_path = bundled_zip_path(app, "windows", arch)?;
    let segger_dir = segger_roaming_dir()
        .ok_or_else(|| AppError::Internal("USERPROFILE not set".to_string()))?;

    let dst_dir = segger_dir.join(BUNDLED_DIR_NAME);
    let jlink_exe = dst_dir.join("JLink.exe");

    if !jlink_exe.exists() {
        log::info!(
            "[jlink] Extracting bundled {} from {} to {}",
            BUNDLED_DIR_NAME,
            zip_path.display(),
            dst_dir.display()
        );
        extract_zip(&zip_path, &dst_dir)?;

        if !jlink_exe.exists() {
            return Err(AppError::Internal(format!(
                "Bundled J-Link extracted, but JLink.exe not found at {}",
                jlink_exe.display()
            )));
        }
    } else {
        log::info!("[jlink] Using bundled J-Link at {}", dst_dir.display());
    }

    platform::ensure_jlink_runtime_env(&dst_dir.to_string_lossy().to_string());
    Ok(dst_dir)
}

#[cfg(target_os = "linux")]
fn elevate_extract_with_pkexec(zip_path: &Path, dst_dir: &Path) -> AppResult<()> {
    use std::process::Command;

    let exe = std::env::current_exe().map_err(|e| AppError::Internal(e.to_string()))?;
    let status = Command::new("pkexec")
        .arg(exe)
        .arg("--lite-extract-jlink")
        .arg(zip_path)
        .arg(dst_dir)
        .status()
        .map_err(|e| AppError::Platform(format!("Failed to launch pkexec: {}", e)))?;

    if !status.success() {
        return Err(AppError::Platform(format!(
            "Authorization failed or extraction command returned {}",
            status
        )));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn ensure_extracted_and_on_path(app: &AppHandle) -> AppResult<PathBuf> {
    let arch = BundledArch::from_rust_arch()
        .ok_or_else(|| AppError::Internal("Unsupported CPU architecture".to_string()))?;
    let zip_path = bundled_zip_path(app, "linux", arch)?;

    // Product requirement: extract under `/opt/SEGGER` (not `/opt/SEGGER/JLink_V930a`).
    // Zip layout may still place a `JLink_V930a/` subfolder inside that tree.
    let dst_root = PathBuf::from("/opt/SEGGER");

    if linux_resolve_jlink_exe(&dst_root).is_none() {
        log::info!(
            "[jlink] Extracting bundled J-Link from {} to {}",
            zip_path.display(),
            dst_root.display()
        );

        match extract_zip(&zip_path, &dst_root) {
            Ok(()) => {}
            Err(AppError::Io(msg)) => {
                // Attempt elevation when /opt is not writable.
                if msg.contains("Permission denied") {
                    elevate_extract_with_pkexec(&zip_path, &dst_root)?;
                } else {
                    return Err(AppError::Io(msg));
                }
            }
            Err(e) => return Err(e),
        }

        // Ensure executable bits are correct. If we elevated via pkexec, the helper
        // performs these fixups as root.
        if let Err(e) = linux_post_extract_fixups(&dst_root) {
            log::warn!("[jlink] Post-extract fixups failed: {}", e);
        }
    }

    let jlink_exe = linux_resolve_jlink_exe(&dst_root).ok_or_else(|| {
        AppError::Internal(format!(
            "Bundled J-Link extracted under {}, but JLinkExe not found (expected {} or {})",
            dst_root.display(),
            dst_root.join("JLinkExe").display(),
            dst_root.join(BUNDLED_DIR_NAME).join("JLinkExe").display()
        ))
    })?;

    let install_dir = jlink_exe
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| AppError::Internal("JLinkExe has no parent path".to_string()))?;

    platform::ensure_jlink_runtime_env(&install_dir.to_string_lossy().to_string());
    Ok(install_dir)
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
pub fn ensure_extracted_and_on_path(_app: &AppHandle) -> AppResult<PathBuf> {
    Err(AppError::Internal(
        "WinUSB Switcher Lite bundled J-Link is not implemented for this OS yet".to_string(),
    ))
}

