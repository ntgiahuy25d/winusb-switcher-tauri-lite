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

        // Unix zip entries with S_IFLNK mode (0o120000) store the symlink target as the
        // file content. DEB-derived SEGGER packages use this for libjlinkarm.so -> libjlinkarm.so.9
        // etc. If we write them as plain files, JLinkExe cannot dlopen the library.
        #[cfg(unix)]
        {
            const S_IFLNK: u32 = 0o120_000;
            if let Some(mode) = file.unix_mode() {
                if mode & 0o170_000 == S_IFLNK {
                    let mut target_buf = Vec::with_capacity(256);
                    file.read_to_end(&mut target_buf)
                        .map_err(|e| AppError::Io(e.to_string()))?;
                    let target = String::from_utf8_lossy(&target_buf).trim().to_string();
                    // Remove stale entry (regular file or old symlink) before creating.
                    let _ = std::fs::remove_file(&out_path);
                    std::os::unix::fs::symlink(&target, &out_path)
                        .map_err(|e| AppError::Io(format!("symlink {} -> {}: {}", out_path.display(), target, e)))?;
                    log::debug!("[jlink] symlink {} -> {}", out_path.display(), target);
                    continue;
                }
            }
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

/// When files under `/opt/SEGGER` are root-owned (e.g. after `pkexec` extract), unprivileged `chmod` fails
/// with EPERM. Fall back to **one** PolicyKit prompt for all paths: `pkexec chmod +x file1 file2 ...`.
#[cfg(target_os = "linux")]
fn try_pkexec_chmod_x_many(paths: &[PathBuf]) {
    use std::process::Command;

    if paths.is_empty() {
        return;
    }

    let mut cmd = Command::new("pkexec");
    cmd.arg("chmod").arg("+x");
    for p in paths {
        cmd.arg(p);
    }

    match cmd.status() {
        Ok(s) if s.success() => {
            log::info!(
                "[jlink] pkexec chmod +x ({} file(s), e.g. {})",
                paths.len(),
                paths[0].display()
            );
        }
        Ok(s) => {
            log::warn!("[jlink] pkexec chmod +x batch failed with status {}", s);
        }
        Err(e) => {
            log::warn!("[jlink] pkexec chmod +x batch: {}", e);
        }
    }
}

/// Typical SEGGER Linux CLI/GUI binaries next to `JLinkExe` (DEB layout; no `.exe` suffix on Linux).
#[cfg(target_os = "linux")]
const LINUX_SEGGER_EXECUTABLE_NAMES: &[&str] = &[
    "JLinkExe",
    "JLinkConfigExe",
    "JLinkGDBServer",
    "JLinkGDBServerCLExe",
    "JLinkGUIServerExe",
    "JFlashLiteExe",
    "JLinkLicenseManagerExe",
    "JLinkRegistrationExe",
];

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
fn linux_segger_install_dirs(dst_root: &Path) -> Vec<PathBuf> {
    let mut dirs = vec![dst_root.to_path_buf()];
    let nested = dst_root.join(BUNDLED_DIR_NAME);
    if nested.is_dir() {
        dirs.push(nested);
    }
    dirs
}

#[cfg(target_os = "linux")]
pub fn linux_post_extract_fixups(dst_root: &Path) -> AppResult<()> {
    // The DEB-derived folder often loses executable bits when we package/extract via zip.
    // Under `/opt`, extraction may be root-owned; batch `pkexec chmod` so PolicyKit prompts once.
    let mut need_pkexec: Vec<PathBuf> = Vec::new();
    for dir in linux_segger_install_dirs(dst_root) {
        for name in LINUX_SEGGER_EXECUTABLE_NAMES {
            let p = dir.join(name);
            if p.is_file() {
                if let Err(e) = set_exec_bit(&p) {
                    log::warn!("[jlink] chmod {}: {}", p.display(), e);
                    need_pkexec.push(p);
                }
            }
        }
    }
    if !need_pkexec.is_empty() {
        try_pkexec_chmod_x_many(&need_pkexec);
    }
    Ok(())
}

/// SEGGER Linux packages ship `99-jlink.rules` (sometimes `70-jlink.rules`) next to the tools,
/// or under `ETC/udev/rules.d/` in some tarball layouts. Minimal zips may omit rules entirely.
#[cfg(target_os = "linux")]
fn linux_find_jlink_rules_in_tree(dir: &Path, max_depth: u32) -> Option<PathBuf> {
    if max_depth == 0 || !dir.is_dir() {
        return None;
    }
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let p = entry.path();
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if p.is_file() && name.ends_with(".rules") && name.contains("jlink") {
            return Some(p);
        }
        if p.is_dir() {
            if let Some(found) = linux_find_jlink_rules_in_tree(&p, max_depth - 1) {
                return Some(found);
            }
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn linux_segger_udev_rules_src(dst_root: &Path) -> Option<PathBuf> {
    let flat_bases = [dst_root.join(BUNDLED_DIR_NAME), dst_root.to_path_buf()];
    for name in ["99-jlink.rules", "70-jlink.rules"] {
        for base in &flat_bases {
            let candidates = [
                base.join(name),
                base.join("ETC").join("udev").join("rules.d").join(name),
                base.join("etc").join("udev").join("rules.d").join(name),
            ];
            for p in candidates {
                if p.is_file() {
                    return Some(p);
                }
            }
        }
    }
    for base in &flat_bases {
        if let Some(p) = linux_find_jlink_rules_in_tree(base, 12) {
            return Some(p);
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn embedded_segger_udev_rules_bytes() -> &'static [u8] {
    include_bytes!("../resources/segger-99-jlink.rules")
}

/// Install SEGGER udev rules (requires write access to `/etc/udev` unless running under pkexec).
#[cfg(target_os = "linux")]
pub fn linux_install_segger_udev_rules_bytes(content: &[u8], source_label: &str) -> AppResult<()> {
    use std::process::Command;

    let dest = Path::new("/etc/udev/rules.d/99-jlink.rules");
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| AppError::Io(e.to_string()))?;
    }
    std::fs::write(dest, content).map_err(|e| AppError::Io(e.to_string()))?;

    let s = Command::new("udevadm")
        .args(["control", "--reload-rules"])
        .status()
        .map_err(|e| AppError::Platform(e.to_string()))?;
    if !s.success() {
        return Err(AppError::Platform(format!(
            "udevadm control --reload-rules returned {}",
            s
        )));
    }
    let s2 = Command::new("udevadm")
        .arg("trigger")
        .status()
        .map_err(|e| AppError::Platform(e.to_string()))?;
    if !s2.success() {
        return Err(AppError::Platform(format!("udevadm trigger returned {}", s2)));
    }

    log::info!(
        "[jlink] Installed udev rules ({}) -> {}",
        source_label,
        dest.display()
    );
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn linux_install_segger_udev_rules_from_src(rules_src: &Path) -> AppResult<()> {
    if !rules_src.is_file() {
        return Err(AppError::Internal(format!(
            "udev rules file not found: {}",
            rules_src.display()
        )));
    }
    let bytes = std::fs::read(rules_src).map_err(|e| AppError::Io(e.to_string()))?;
    linux_install_segger_udev_rules_bytes(&bytes, &format!("{}", rules_src.display()))
}

/// After extraction: install rules from the zip tree, or from an **embedded fallback** if the minimal
/// archive omitted `99-jlink.rules` (previously caused silent “success” with no `/etc/udev` file).
#[cfg(target_os = "linux")]
pub fn linux_try_install_segger_udev_after_extract(dst_root: &Path) -> AppResult<()> {
    match linux_segger_udev_rules_src(dst_root) {
        Some(p) => {
            let b = std::fs::read(&p).map_err(|e| AppError::Io(e.to_string()))?;
            linux_install_segger_udev_rules_bytes(&b, &format!("{}", p.display()))
        }
        None => {
            log::warn!(
                "[jlink] No udev rules file under {} after extract — installing embedded fallback",
                dst_root.display()
            );
            linux_install_segger_udev_rules_bytes(
                embedded_segger_udev_rules_bytes(),
                "embedded segger-99-jlink.rules",
            )
        }
    }
}

#[cfg(target_os = "linux")]
fn app_error_is_permission_denied(e: &AppError) -> bool {
    match e {
        AppError::Io(s) | AppError::Platform(s) | AppError::Internal(s) => {
            s.contains("Permission denied") || s.contains("os error 13")
        }
        _ => false,
    }
}

/// Ensure bundled udev rules are installed even when J-Link was already on disk (upgrade / skip extract).
/// Skips work if `/etc/udev/rules.d/99-jlink.rules` already matches the desired bytes (tree or embedded).
#[cfg(target_os = "linux")]
fn linux_ensure_segger_udev_installed(dst_root: &Path) -> AppResult<()> {
    use std::borrow::Cow;

    let src = linux_segger_udev_rules_src(dst_root);
    let desired: Cow<'static, [u8]> = match &src {
        Some(p) => match std::fs::read(p) {
            Ok(b) if !b.is_empty() => Cow::Owned(b),
            _ => Cow::Borrowed(embedded_segger_udev_rules_bytes()),
        },
        None => Cow::Borrowed(embedded_segger_udev_rules_bytes()),
    };

    let dest = Path::new("/etc/udev/rules.d/99-jlink.rules");
    if dest.is_file() {
        if let Ok(cur) = std::fs::read(dest) {
            if cur == desired.as_ref() {
                log::debug!("[jlink] udev rules already match desired copy; skipping install");
                return Ok(());
            }
        }
    }

    match linux_install_segger_udev_rules_bytes(desired.as_ref(), "ensure") {
        Ok(()) => Ok(()),
        Err(e) if app_error_is_permission_denied(&e) => {
            log::info!("[jlink] udev install needs elevation — requesting pkexec");
            let tmp = std::env::temp_dir().join(format!(
                "winusb-switcher-lite-udev-{}.rules",
                std::process::id()
            ));
            std::fs::write(&tmp, desired.as_ref()).map_err(|e| AppError::Io(e.to_string()))?;
            let r = elevate_udev_install_with_pkexec(&tmp);
            let _ = std::fs::remove_file(&tmp);
            r
        }
        Err(e) => Err(e),
    }
}

/// When `/opt` is user-writable, extraction does not use pkexec; install udev with one extra PolicyKit prompt if needed.
#[cfg(target_os = "linux")]
fn elevate_udev_install_with_pkexec(rules_src: &Path) -> AppResult<()> {
    use std::process::Command;

    let exe = std::env::current_exe().map_err(|e| AppError::Internal(e.to_string()))?;
    let rules_src = rules_src
        .canonicalize()
        .map_err(|e| AppError::Io(e.to_string()))?;

    let status = Command::new("pkexec")
        .arg(exe)
        .arg("--lite-install-udev")
        .arg(&rules_src)
        .status()
        .map_err(|e| AppError::Platform(format!("Failed to launch pkexec for udev: {}", e)))?;

    if !status.success() {
        return Err(AppError::Platform(format!(
            "udev install authorization failed or command returned {}",
            status
        )));
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

/// Check whether the current process can write into `dir` (which may not exist yet).
/// Returns `true` when root access will be required.
#[cfg(target_os = "linux")]
fn linux_dst_needs_root(dir: &Path) -> bool {
    // Walk up to the first existing ancestor and check writability with a probe file.
    let mut check = dir;
    loop {
        if check.exists() {
            let probe = check.join(".jlink_write_probe");
            let ok = std::fs::File::create(&probe).is_ok();
            let _ = std::fs::remove_file(&probe);
            return !ok;
        }
        match check.parent() {
            Some(p) => check = p,
            None => return true,
        }
    }
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

        if linux_dst_needs_root(&dst_root) {
            // Single pkexec: extract + udev rules + chmod → one PolicyKit dialog.
            log::info!("[jlink] /opt/SEGGER not writable by current user — using pkexec (one prompt)");
            elevate_extract_with_pkexec(&zip_path, &dst_root)?;
            // fixups + udev already performed by the pkexec helper.
        } else {
            extract_zip(&zip_path, &dst_root)?;
            // Fixups needed after user-level extraction (files extracted without +x).
            if let Err(e) = linux_post_extract_fixups(&dst_root) {
                log::warn!("[jlink] Post-extract fixups failed: {}", e);
            }
            if let Some(src) = linux_segger_udev_rules_src(&dst_root) {
                if let Err(e) = linux_install_segger_udev_rules_from_src(&src) {
                    log::warn!(
                        "[jlink] Could not install udev rules without elevation ({}); requesting pkexec",
                        e
                    );
                    elevate_udev_install_with_pkexec(&src)?;
                }
            }
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

    // Udev setup must run even if we did not extract this session (existing /opt tree from older versions).
    if let Err(e) = linux_ensure_segger_udev_installed(&dst_root) {
        log::warn!("[jlink] Automatic udev install did not complete: {}", e);
    }

    platform::ensure_jlink_runtime_env(&install_dir.to_string_lossy().to_string());
    Ok(install_dir)
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
pub fn ensure_extracted_and_on_path(_app: &AppHandle) -> AppResult<PathBuf> {
    Err(AppError::Internal(
        "WinUSB Switcher Lite bundled J-Link is not implemented for this OS yet".to_string(),
    ))
}

