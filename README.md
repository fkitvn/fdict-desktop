# FDict

Windows app for looking up English and Vietnamese while you read. Select text and press **Alt + Q**.

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

Production installer (`FDict`):

```powershell
yarn tauri build --bundles nsis
```

Output: `src-tauri\target\release\bundle\nsis\FDict_0.1.0_x64-setup.exe`

Development installer (`FDict Dev`, installs beside production):

```powershell
yarn tauri build --bundles nsis --config src-tauri/tauri.dev.conf.json
```

## Publish

Contributors open a branch and a pull request into `prod`. Merging `prod` publishes only when the version changed.

`yarn version:set 1.0.4` writes that version into the three version files. GitHub Actions then builds the installer and records the download link.

A merge that does not change the version does not build a new installer.

