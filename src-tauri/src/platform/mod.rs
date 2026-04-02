//! Platform abstraction for PATH management and JLink search directories.

use std::path::PathBuf;

#[cfg(target_os = "windows")]
pub mod windows;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "linux")]
pub mod linux;

pub struct PlatformConfig {
    pub jlink_bin: &'static str,
    pub jlink_executable: &'static str,
}

pub fn config() -> PlatformConfig {
    #[cfg(target_os = "windows")]
    return PlatformConfig {
        jlink_bin: "JLink",
        jlink_executable: "JLink.exe",
    };
    #[cfg(target_os = "macos")]
    return PlatformConfig {
        jlink_bin: "JLinkExe",
        jlink_executable: "JLinkExe",
    };
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    return PlatformConfig {
        jlink_bin: "JLinkExe",
        jlink_executable: "JLinkExe",
    };
}

pub fn search_dirs() -> Vec<PathBuf> {
    #[cfg(target_os = "windows")]
    return windows::search_dirs();
    #[cfg(target_os = "macos")]
    return macos::search_dirs();
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    return linux::search_dirs();
}

/// Find directory containing JLink executable in known locations.
pub fn find_jlink_in_search_dirs() -> Option<PathBuf> {
    let executable = config().jlink_executable;
    for base in search_dirs() {
        if !base.exists() { continue; }
        if let Ok(entries) = std::fs::read_dir(&base) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && path.join(executable).exists() {
                    return Some(path);
                }
            }
        }
        if base.join(executable).exists() {
            return Some(base);
        }
    }
    None
}

/// Update current process PATH so the new dir is usable in this session.
pub fn prepend_to_process_path(dir: &str) {
    let path_key = std::env::vars()
        .find(|(k, _)| k.to_lowercase() == "path")
        .map(|(k, _)| k)
        .unwrap_or_else(|| "PATH".to_string());

    let current = std::env::var(&path_key).unwrap_or_default();
    if !current.to_lowercase().contains(&dir.to_lowercase()) {
        let separator = if cfg!(target_os = "windows") { ";" } else { ":" };
        // Prepend so our bundled J-Link wins over any stale PATH entries.
        std::env::set_var(&path_key, format!("{}{}{}", dir, separator, current));
    }
}

/// Linux: SEGGER `JLinkExe` loads `libjlinkarm.so` from the install directory. If only `PATH` is set,
/// the dynamic linker may still fail with **"Could not open J-Link shared library"** unless the
/// directory is also on `LD_LIBRARY_PATH` (RPATH/`$ORIGIN` can be insufficient in some layouts).
#[cfg(target_os = "linux")]
#[allow(dead_code)]
pub fn prepend_ld_library_path(dir: &str) {
    const KEY: &str = "LD_LIBRARY_PATH";
    let current = std::env::var(KEY).unwrap_or_default();
    if current.split(':').any(|p| !p.is_empty() && p == dir) {
        return;
    }
    let sep = ':';
    std::env::set_var(KEY, format!("{}{}{}", dir, sep, current));
    log::info!("[jlink] Prepended {} to {}", dir, KEY);
}

/// After locating a J-Link install directory, apply PATH (all platforms) and Linux shared-library path.
pub fn ensure_jlink_runtime_env(install_dir: &str) {
    prepend_to_process_path(install_dir);
    #[cfg(target_os = "linux")]
    {
        apply_ld_library_path_segger_layout(install_dir);
        // Used by `jlink::runner` so the J-Link child process runs with the same working directory
        // as a manual install (some SEGGER layouts rely on `$ORIGIN`/relative paths).
        std::env::set_var("WINUSB_JLINK_INSTALL_DIR", install_dir);
    }
}

/// Linux: set `LD_LIBRARY_PATH` for a typical SEGGER tree in one shot. Some packages put `*.so`
/// under `x86_64/` or `x86/`; order is **install root first**, then host-relevant arch dirs.
#[cfg(target_os = "linux")]
fn apply_ld_library_path_segger_layout(install_dir: &str) {
    const KEY: &str = "LD_LIBRARY_PATH";
    let base = std::path::Path::new(install_dir);

    let mut front: Vec<String> = Vec::new();
    let push_unique = |v: &mut Vec<String>, s: String| {
        if !v.iter().any(|e| e == &s) {
            v.push(s);
        }
    };

    push_unique(&mut front, install_dir.to_string());

    // Prefer native arch before 32-bit `x86/` on 64-bit hosts (wrong ELF breaks dlopen).
    let sub_order: &[&str] = match std::env::consts::ARCH {
        "x86_64" => &["x86_64", "amd64", "x86"],
        "aarch64" => &["aarch64", "arm64"],
        _ => &["x86_64", "amd64", "x86", "aarch64", "arm64"],
    };
    for sub in sub_order {
        let p = base.join(sub);
        if p.is_dir() {
            push_unique(&mut front, p.to_string_lossy().into_owned());
        }
    }

    let current = std::env::var(KEY).unwrap_or_default();
    for seg in current.split(':') {
        if seg.is_empty() {
            continue;
        }
        push_unique(&mut front, seg.to_string());
    }

    let joined = front.join(":");
    std::env::set_var(KEY, &joined);
    log::info!("[jlink] {}={}", KEY, joined);
}
