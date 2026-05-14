# Daily Cat Wallpaper

Daily Cat Wallpaper is a cross-platform Rust desktop app for automatic cat wallpaper freedom. It lets users choose language, cat breeds, monitor behavior, image quality, image mood, refresh frequency, sources, and interaction preferences from a Tauri configuration UI.

The first implementation targets Windows as the stable platform and ships macOS/Linux wallpaper support as beta because each desktop environment exposes different wallpaper APIs.

## Features

- Rust core with Tauri 2 desktop shell.
- Windows native wallpaper control through `SystemParametersInfoW`, with a generated multi-monitor span image when more than one display is detected.
- macOS beta wallpaper setting through `osascript`.
- Linux beta wallpaper setting for GNOME/Cinnamon/Unity, KDE Plasma, and Xfce.
- Multilingual configuration UI with automatic locale detection, English fallback, Simplified Chinese, Traditional Chinese, Japanese, and Korean.
- Configurable breeds, display-matched cats by default, fixed 1-5 cat mode, image mood, refresh frequency, local folders, Wikimedia Commons, CATAAS, TheCatAPI, and optional API-key sources for Pexels, Pixabay, and Magnific.
- Breed-aware image selection: known breeds are mapped to TheCatAPI breed IDs, and color/type preferences such as orange tabby, black cat, white cat, and calico are used as Wikimedia Commons search keywords.
- HD-first image policy: default minimum 2560x1440, preferred online request size 3840x2160, low-resolution fallback disabled by default, cache reuse when online sources fail, and an offline generated cat wallpaper fallback.
- Optional interaction preferences for breathing, mouse proximity, click paw, keyboard Bongo, and sound. Sound is off by default.
- React configuration UI with live layout preview and manual refresh.

## Platform Status

| Platform | Status | Notes |
| --- | --- | --- |
| Windows x64/arm64 | Stable target | Native static wallpaper setting is implemented. |
| macOS x64/arm64 | Beta | Static wallpaper setting uses AppleScript. |
| Linux x64/arm64 | Beta | Static wallpaper setting supports common desktop environments on a best-effort basis. |

## Development

Prerequisites:

- Rust stable toolchain with `rustfmt` and `clippy`.
- Node.js 22 or newer.
- Tauri platform prerequisites for your OS.

Run checks:

```powershell
cargo test --workspace
npm install
npm run build
cargo check
```

Run the app in development:

```powershell
npm run tauri dev
```

## Image Sources

Default sources are local folders, Wikimedia Commons, [TheCatAPI](https://www.thecatapi.com/), and [CATAAS](https://cataas.com/). Optional higher-quality sources use user-provided API keys for [Pexels](https://www.pexels.com/zh-cn/api/key/), [Pixabay](https://pixabay.com/api/docs/), and Magnific. Local images and downloaded candidates are filtered against the configured minimum resolution before they are used. If no online candidate survives, the app reuses the newest valid cached image and then falls back to a generated HD cat wallpaper so manual refresh does not fail empty.

## Repository Scope

This repository is an independent desktop app. It does not require Lively Wallpaper, Wallpaper Engine, or Bing Wallpaper.

## License

MIT
