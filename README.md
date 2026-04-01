# WinUSB Switcher Lite

A **Tauri 2** desktop app that switches SEGGER J-Link probes to **WinUSB** driver mode. Unlike the full WinUSB Switcher, **Lite ships a fixed J-Link V930a payload** inside the installer—there is **no** in-app download or SEGGER installer flow.

Built with **Rust** (`src-tauri`) and **React + TypeScript** (`src/renderer`).

## What you get

| Platform | Bundled J-Link | First-run behavior |
|----------|----------------|-------------------|
| **Windows x64** | Windows zip only (installers do **not** include Linux archives) | Unpacks to `%AppData%\Roaming\SEGGER\JLink_V930a` |
| **Linux x64** | Linux zip only | Unpacks under `/opt/SEGGER/JLink_V930a`; **pkexec** may prompt if elevation is required |
| **macOS** | 22-byte empty ZIP stub only (build-time; satisfies Tauri’s resource glob) | Bundled J-Link extraction is not implemented for macOS yet — the app cannot use the Lite flow on macOS until a real Darwin payload exists |

Release **installers** are built per OS; each artifact contains **only** the J-Link zip for that target. Canonical zips live in **`src-tauri/jlink-bundles/`** (tracked with **Git LFS**). At dev/build time, **`scripts/stage-jlink-for-build.mjs`** copies the matching zip into **`src-tauri/resources/jlink/`** (gitignored) so Tauri bundles a single payload.

## Release and download

**Installers** (`.exe` / `.msi`, `.deb` / `.AppImage`, `.dmg` where applicable) are attached to **GitHub Releases** after a successful tag build.

| What | Workflow | When |
|------|----------|------|
| **CI** | `CI` | Push or PR to `main` |
| **Build + release** | `Build WinUSB Switcher Lite` | Push a **`v*`** tag (or run the workflow manually; the **release** job runs only on tag pushes) |

**Publish a version (maintainers):**

1. Keep **CI** green on `main`.
2. Set the **same** semver in `package.json`, `src-tauri/tauri.conf.json`, and `src-tauri/Cargo.toml` (no `v` prefix in those files). After editing `Cargo.toml`, run `cargo check --manifest-path src-tauri/Cargo.toml` so `Cargo.lock` stays in sync.
3. Commit and push to `main`, then tag and push:
   ```bash
   git checkout main && git pull
   git tag v1.0.2
   git push origin main
   git push origin v1.0.2
   ```
4. Wait for **Build WinUSB Switcher Lite** → **release**; open the **Releases** tab for assets.

**Repo settings:** **Settings → Actions → General → Workflow permissions → Read and write** so the release job can upload assets.

## Clone and Git LFS

Large J-Link zips are stored with **Git LFS**. After cloning:

```bash
git lfs install
git lfs pull
```

If you see errors about an invalid zip or “LFS pointer” at runtime, the real files were not pulled—run `git lfs pull` and rebuild.

## Technology stack

| Area | Notes |
|------|--------|
| **Shell** | Tauri 2, system webview |
| **UI** | React 18, TypeScript, Vite 6, Tailwind 3, Zustand |
| **Backend** | Rust, Tokio, `zip` for extraction |
| **J-Link** | Commander CLI (`JLink.exe` / `JLinkExe`) from the bundled tree |

## Features

- One-time **bootstrap** unpacks embedded J-Link and prepends it to `PATH` for the process.
- Detect J-Link, scan probes, **Switch to WinUSB** (includes firmware auto-update step before driver switch, same idea as WinUSB Switcher).

## Development

**Requirements:** Node **20+**, **Yarn classic 1.x**, **Rust stable**, OS packages per [Tauri prerequisites](https://tauri.app/start/prerequisites/).

```bash
yarn install
yarn tauri:dev    # full app (runs staging script, then Vite + Rust)
```

- **`yarn dev`** alone only runs Vite—Tauri IPC and J-Link commands will not work.
- **`yarn tauri:build`** runs the frontend build, **`stage-jlink-for-build.mjs`**, then produces release binaries/installers under `src-tauri/target/release/` and `bundle/`.

## Project layout (high level)

```text
.
├── scripts/
│   └── stage-jlink-for-build.mjs   # Copies one OS/arch zip into resources/jlink for bundling
├── src/renderer/                   # React UI
├── src/shared/types.ts             # IPC command names
└── src-tauri/
    ├── jlink-bundles/              # Git LFS: windows/x86_64, linux/x86_64, …
    ├── resources/                  # _bundle_manifest.txt; jlink/ staged locally (ignored)
    ├── src/
    │   ├── bundled_jlink.rs        # Extract + PATH; Linux pkexec helper
    │   ├── commands/lite.rs        # prepare_bundled_jlink
    │   └── jlink/                  # detect, scan, usb_driver, firmware, …
    └── tauri.conf.json
```

## CI/CD

Workflows live under [`.github/workflows/`](.github/workflows/). Checkout uses **`lfs: true`** so CI pulls LFS objects before `yarn tauri:build`.

## Troubleshooting

- **Invalid zip / EOCD / LFS pointer** — Install Git LFS, `git lfs pull`, rebuild.
- **Linux permission denied under `/opt`** — Approve the **pkexec** prompt or install manually with appropriate permissions.
- **“J-Link not found” after bootstrap** — Ensure staging ran (use `yarn tauri:dev` / `yarn tauri:build`), and on Linux that `JLinkExe` under `/opt/SEGGER/JLink_V930a` is executable.

## License

MIT
