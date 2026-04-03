# WinUSB Switcher Lite

A **Tauri 2** desktop app that switches SEGGER J-Link probes to **WinUSB** driver mode. Unlike the full WinUSB Switcher, **Lite ships a fixed J-Link V930a payload** inside the installer—there is **no** in-app download or SEGGER installer flow.

Built with **Rust** (`src-tauri`) and **React + TypeScript** (`src/renderer`).

## What you get

| Platform | Bundled J-Link | First-run behavior |
|----------|----------------|-------------------|
| **Windows x64** | Windows zip only (installers do **not** include Linux archives) | Unpacks to `%AppData%\Roaming\SEGGER\JLink_V930a` |
| **Linux x64** | Linux zip only | Unpacks under **`/opt/SEGGER`** (zip may add a `JLink_V930a/` subfolder). If `/opt` is not writable, **one `pkexec` prompt** runs **extract + SEGGER `99-jlink` udev rules + executable fixups** together. |
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
   git tag v1.0.7
   git push origin main
   git push origin v1.0.7
   ```
4. Wait for **Build WinUSB Switcher Lite** → **release**; open the **Releases** tab for assets.

   **Dual mirrors:** If you maintain the same app in two GitHub repos, repeat **`git push <remote> main`**, **`git push <remote> v1.0.7`**, and **`git lfs push <remote> main`** for the second remote after the tag exists locally (see [Mirroring](#mirroring)).

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

This repo is a **Tauri 2** app. You need:

- **Node.js** (recommended via **NVM**) for the React/Vite frontend
- **Yarn classic (v1)** for JS dependencies (`yarn.lock`)
- **Rust/Cargo** (via **rustup**) for the Tauri backend
- OS-level dependencies required by **Tauri/WebView**

Tauri’s OS dependency list is here:

- [Tauri prerequisites](https://tauri.app/start/prerequisites/)

### Install toolchains (recommended)

#### Windows

1. **NVM for Windows**: install from the official `nvm-windows` releases.
2. Install Node 20 and enable it:

```bash
nvm install 20
nvm use 20
node --version
```

3. Install **Yarn classic**:

```bash
npm install -g yarn
yarn --version   # expect 1.22.x
```

4. Install **Rust** (rustup) and verify:

```bash
rustc --version
cargo --version
```

5. Install Windows build prerequisites for Tauri (MSVC build tools) per the Tauri docs.

#### macOS

1. Install Xcode Command Line Tools:

```bash
xcode-select --install
```

2. Install **NVM**, then Node 20:

```bash
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.7/install.sh | bash
source ~/.zshrc  # or ~/.bashrc
nvm install 20
nvm use 20
node --version
```

3. Install **Yarn classic**:

```bash
npm install -g yarn
yarn --version
```

4. Install **Rust** (rustup) and verify:

```bash
rustc --version
cargo --version
```

#### Linux (Ubuntu/Debian example)

1. Install build prerequisites for Tauri/WebKitGTK per the Tauri docs.
2. Install **NVM**, then Node 20:

```bash
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.7/install.sh | bash
source ~/.bashrc
nvm install 20
nvm use 20
node --version
```

3. Install **Yarn classic**:

```bash
npm install -g yarn
yarn --version
```

4. Install **Rust** (rustup) and verify:

```bash
rustc --version
cargo --version
```

### Repo-specific notes

- **Git LFS is required**: bundled J-Link zips are stored in LFS.

```bash
git lfs install
git lfs pull
```

- **Tauri CLI**: uses local `@tauri-apps/cli` (devDependency). Use `yarn tauri:dev` / `yarn tauri:build` after `yarn install`.

```bash
yarn install
yarn tauri:dev    # full app (runs staging script, then Vite + Rust)
```

- **`npm run stage-jlink`** / **`yarn stage-jlink`** — run from the repo root if you invoke **`cargo`** / **`tauri`** directly without `beforeDevCommand` / `beforeBuildCommand` (creates `src-tauri/resources/jlink/.../JLink_V930a.zip` from `jlink-bundles/`).

- **`yarn dev`** alone only runs Vite—Tauri IPC and J-Link commands will not work.
- **`yarn tauri:build`** runs the frontend build, **`stage-jlink-for-build.mjs`**, then produces release binaries/installers under `src-tauri/target/release/` and `bundle/`.

## Project layout (high level)

```text
.
├── scripts/
│   ├── stage-jlink-for-build.mjs   # Copies one OS/arch zip into resources/jlink for bundling
│   └── push-testing-remote.sh       # Optional: push main + LFS to a second remote (no tags)
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

To push **`main`** (no tags) to a **second remote** (e.g. another GitHub account for testing / fresh LFS quota): add the remote, authenticate as that user, then run **`yarn push:testing`** (see [`scripts/push-testing-remote.sh`](scripts/push-testing-remote.sh)).

### Mirroring

Maintainers may keep **`main`** and **`v*`** tags aligned across:

- `git@github.com:ntgiahuy25d/winusb-switcher-tauri-lite.git`
- `git@github.com:bigzerro12/winusb-switcher-tauri-lite.git`

Use a separate **`git remote`** per account (SSH recommended). After tagging a release:

```bash
git push origin main && git push origin v1.0.7
git lfs push origin main
git push other main && git push other v1.0.7
git lfs push other main
```

Replace **`other`** with your second remote name. Each push must authenticate as a user with **write** access to that repo.

**GitHub Actions fails at checkout with “exceeded its LFS budget”** — That message comes from **GitHub’s Git LFS quota** (storage + bandwidth), not from the app. Anyone cloning with LFS (including `actions/checkout` with `lfs: true`) must be able to download the `JLink_V930a.zip` objects tracked in [`.gitattributes`](.gitattributes).

What you can do:

1. **Restore LFS on the hosting account** (most common): GitHub → **Settings → Billing** for the user or org that owns the repo → add a **Git LFS data pack** or increase limits so transfers work again. Free tiers have a small monthly LFS bandwidth pool; tagged release builds on three OS runners can burn through it quickly.

2. **Stop using LFS for the zip** (avoids LFS bandwidth on every CI run): If each archived zip is **under GitHub’s per-file push limit (100 MB)**, you can migrate the blobs into normal Git (no LFS), update `.gitattributes`, and drop `filter=lfs` for `jlink-bundles/**`. Use Git’s docs or `git lfs migrate export` carefully (this rewrites history unless you only change the tip). After that, CI still works with `lfs: true` (nothing to fetch) or you can set `lfs: false` in workflows.

3. **Host zips outside GitHub** — Keep only small files in the repo and download `JLink_V930a.zip` in CI from object storage or another mirror (requires workflow + secret changes).

## Troubleshooting

- **Invalid zip / EOCD / LFS pointer** — Install Git LFS, `git lfs pull`, rebuild.
- **CI: `This repository exceeded its LFS budget`** — See **[CI/CD](#cicd)** above; increase LFS quota or migrate bundles off LFS.
- **Linux permission denied under `/opt`** — On first run, Lite installs the bundled J-Link under `/opt/SEGGER`. If `/opt` is not writable, you’ll be prompted **once** via **pkexec** to complete extraction + permission fixups. **Canceling that dialog** fails setup and shows the same **Try again** screen as when udev **pkexec** is dismissed (**v1.0.7+**).
- **“J-Link not found” after bootstrap** — Ensure staging ran (use `yarn tauri:dev` / `yarn tauri:build`), and on Linux that `JLinkExe` exists under `/opt/SEGGER` (flat) or `/opt/SEGGER/JLink_V930a` (nested zip) and is executable.
- **Linux can’t see probes / permission denied opening USB device** — The app installs SEGGER’s **`99-jlink.rules`** (from the bundled tree under `/opt/SEGGER`, including `ETC/udev/rules.d/` layouts) **on each startup** if the file is missing or differs from the bundle. The first install may use **`pkexec`** when `/etc` is not writable. Dismissing that prompt leaves setup incomplete (**Try again** in **v1.0.7+**). If you upgraded from a build that skipped udev when `/opt` was already populated, open the app once and approve the prompt, or install rules manually below.

- **Linux: no `99-jlink.rules` on first install** — Some bundled J-Link zips **do not ship** `99-jlink.rules` at all. Older code treated that as “success” and skipped `/etc/udev`. **v1.0.6+** installs **`src-tauri/resources/segger-99-jlink.rules`** (embedded in the binary) whenever the extracted tree has no rules file, and searches the tree recursively for `*jlink*.rules`. (Bootstrap **v1.0.7+** also fails clearly if the udev **pkexec** step is canceled.)

- **Linux: no `99-jlink.rules` after upgrading** — If `JLinkExe` was already under `/opt/SEGGER`, an older build could skip udev. Current builds **re-check on every launch**; alternatively copy rules manually:

```bash
# Example (adjust if your system uses a different file name/path)
sudo cp /opt/SEGGER/JLink_V930a/99-jlink.rules /etc/udev/rules.d/99-jlink.rules
sudo udevadm control --reload-rules
sudo udevadm trigger
```

- **Linux: “Could not open J-Link shared library”** — Ensure you extracted a real SEGGER payload (not a Git LFS pointer) and that the install tree contains valid `libjlinkarm.so*` files/symlinks. If the issue persists, verify system deps (e.g. `libusb-1.0-0`) and run `ldd` on `JLinkExe` to identify missing libraries.

## License

MIT
