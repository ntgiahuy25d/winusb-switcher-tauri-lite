#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Linux elevation helper:
    // `pkexec <this_exe> --lite-extract-jlink <zip_path> <dst_dir>`
    // runs extraction as root so we can install to /opt/SEGGER.
    #[cfg(target_os = "linux")]
    {
        let mut args = std::env::args().skip(1);
        if let Some(flag) = args.next() {
            if flag == "--lite-extract-jlink" {
                let zip_path = args.next().unwrap_or_default();
                let dst_dir = args.next().unwrap_or_default();
                if zip_path.is_empty() || dst_dir.is_empty() {
                    eprintln!("Usage: --lite-extract-jlink <zip_path> <dst_dir>");
                    std::process::exit(2);
                }
                let zip_path = std::path::PathBuf::from(zip_path);
                let dst_dir = std::path::PathBuf::from(dst_dir);
                match winusb_switcher_lite_lib::extract_zip(&zip_path, &dst_dir) {
                    Ok(()) => std::process::exit(0),
                    Err(e) => {
                        eprintln!("Extraction failed: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        }
    }

    #[cfg(debug_assertions)]
    {
        let _ = ctrlc::set_handler(|| std::process::exit(0));
    }
    winusb_switcher_lite_lib::run();
}