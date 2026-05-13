# Daily Cat Wallpaper

Daily Cat Wallpaper is a cross-platform Rust desktop app for automatic cat wallpaper freedom. It lets users choose cat breeds, same-screen cat count, image mood, refresh frequency, sources, and interaction preferences from a Tauri configuration UI.

The first implementation targets Windows as the stable platform and ships macOS/Linux wallpaper support as beta because each desktop environment exposes different wallpaper APIs.

## Features

- Rust core with Tauri 2 desktop shell.
- Windows native wallpaper control through `SystemParametersInfoW`.
- macOS beta wallpaper setting through `osascript`.
- Linux beta wallpaper setting for GNOME/Cinnamon/Unity, KDE Plasma, and Xfce.
- Configurable breeds, 1-5 same-screen cats, image mood, refresh frequency, local folders, CATAAS, and TheCatAPI.
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

Default sources are local folders, [CATAAS](https://cataas.com/), and [TheCatAPI](https://www.thecatapi.com/). Pexels and Unsplash are reserved for user-provided API keys in a later source plugin so the open-source app does not ship embedded third-party credentials.

## Repository Scope

This repository is an independent desktop app. It does not require Lively Wallpaper, Wallpaper Engine, or Bing Wallpaper.

## License

MIT
