# Fdict

Windows app for looking up English and Vietnamese while you read. Select text and press **Alt + F**.

Requires Windows 10 or later with WebView2, Rust, and [Volta](https://volta.sh/). Node is pinned to 26.3.0.

## Run

```powershell
winget install Volta.Volta
volta install node@26.3.0
yarn
yarn tauri dev
```

Debug builds talk to `http://127.0.0.1:8000` and `http://127.0.0.1:8001`. Release builds talk to `https://fkit.vn` and `https://dict.fkit.vn`.

## Build

Production installer (`Fdict`):

```powershell
$env:FDICT_BUILD_ENV='production'
yarn tauri build --bundles nsis
```

Output: `src-tauri\target\release\bundle\nsis\Fdict_0.1.0_x64-setup.exe`

Development installer (`Fdict Dev`, installs beside production):

```powershell
$env:FDICT_BUILD_ENV='development'
yarn tauri build --bundles nsis --config src-tauri/tauri.dev.conf.json
```

## Publish

Contributors open a branch and a pull request into `prod`. Merging `prod` publishes only when the version changed.

Set the same `x.y.z` version in `package.json`, `src-tauri/tauri.conf.json`, and `src-tauri/Cargo.toml`. It must be higher than the installed app for auto-update to install it. GitHub Actions then builds the Windows installer, creates release `vX.Y.Z`, and records that `.exe` link for the landing download page.

A merge that does not change the version does not build a new installer.

To publish a file you already built, from `fdict-web-app`:

```powershell
yarn node scripts/publish-desktop.mjs --env production --file "..\fdict-desktop\src-tauri\target\release\bundle\nsis\Fdict_0.1.0_x64-setup.exe"
```

