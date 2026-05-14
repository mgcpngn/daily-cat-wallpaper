# Daily Cat Wallpaper

Daily Cat Wallpaper is a cross-platform Rust desktop app for automatic cat wallpaper freedom. It lets users choose language, cat breeds, monitor behavior, image quality, image mood, refresh frequency, sources, and interaction preferences from a Tauri configuration UI.

The first implementation targets Windows as the stable platform and ships macOS/Linux wallpaper support as beta because each desktop environment exposes different wallpaper APIs.

## Features

- Rust core with Tauri 2 desktop shell.
- Windows native wallpaper control through `SystemParametersInfoW`, with a generated virtual-desktop-sized span image for every refresh.
- macOS beta wallpaper setting through `osascript`.
- Linux beta wallpaper setting for GNOME/Cinnamon/Unity, KDE Plasma, and Xfce.
- Multilingual configuration UI with automatic locale detection, English fallback, Simplified Chinese, Traditional Chinese, Japanese, and Korean.
- Configurable breeds, display-matched cats by default, fixed 1-5 cat mode, image mood, refresh frequency, local folders, Wikimedia Commons, CATAAS, TheCatAPI, and optional API-key sources for Pexels, Pixabay, and Magnific.
- Breed-aware image selection: known breeds are mapped to TheCatAPI breed IDs, and color/type preferences such as orange tabby, black cat, white cat, and calico are used as Wikimedia Commons search keywords.
- Hard HD image policy: images below 2560x1440 are rejected before they are written to the managed gallery, and low-resolution fallback is not allowed.
- Fixed managed gallery under the app data directory, with downloaded and generated images browsable from the UI, manual selection, and delete controls.
- AI transparent-cat generation for desktop-pet style PNG cutouts. OpenAI uses a transparent-background image model by default; Google Nano Banana Pro is exposed as a user-key beta provider.
- Desktop effect analysis checks the final wallpaper size against the actual virtual desktop, flags low resolution, placeholder art, non-transparent pet assets, likely cropped transparent subjects, and unverified preference matches.
- Closed-loop learning records like/dislike feedback and reasons, then lowers rejected images in future selection instead of only logging feedback.
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

Default sources are local folders, Wikimedia Commons, [TheCatAPI](https://www.thecatapi.com/), and [CATAAS](https://cataas.com/). Optional higher-quality sources use user-provided API keys for [Pexels](https://www.pexels.com/zh-cn/api/key/), [Pixabay](https://pixabay.com/api/docs/), and Magnific. Local images and downloaded candidates are filtered against the configured minimum resolution before they are used or saved.

Downloaded images are stored in `cat-gallery/downloads`; AI-generated transparent cat PNGs are stored in `cat-gallery/generated`. The app can browse these files, apply one manually, delete weak images, and learn from feedback. If every online source fails, the app can still produce an internal transparent placeholder cat asset, but the effect analysis marks it as placeholder art so the user can replace it with downloaded or AI-generated assets.

## Repository Scope

This repository is an independent desktop app. It does not require Lively Wallpaper, Wallpaper Engine, or Bing Wallpaper.

## License

MIT
