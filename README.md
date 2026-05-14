# Daily Cat Wallpaper

English | [Simplified Chinese](README.zh-CN.md) | [Traditional Chinese](README.zh-TW.md) | [Japanese](README.ja.md) | [Korean](README.ko.md)

Daily Cat Wallpaper is a cross-platform Rust and Tauri 2 desktop app for feedback-driven cat wallpaper freedom. The project has shifted from a simple wallpaper switcher into a managed cat companion system: it finds or generates high-quality cat assets, builds wallpapers for the real monitor layout, analyzes the result, and learns from user feedback so future wallpapers better match the user's taste.

Windows is the stable first target. macOS and Linux support static wallpaper setting as beta because wallpaper APIs differ across desktop environments.

## Product Direction

The current product goal is not "replace Bing Wallpaper with random cat photos." The goal is a desktop cat experience that feels intentional:

- Cats should match the user's preferred breeds, colors, quantity, mood, and scene.
- Images below 2K should not enter the managed gallery.
- Transparent cat cutouts should support a desktop-pet style wallpaper where the cat looks present on the screen instead of pasted into a generic photo.
- Multi-monitor setups should receive sensible defaults: by default, one different cat per display; when the user explicitly selects one cat, that same cat can be reused across displays.
- Users must be able to browse downloaded and generated images, choose a favorite manually, and delete weak images.
- The app must close the loop: analyze the final desktop output, collect like/dislike feedback and reasons, and lower the chance of rejected images being used again.

## Highlights

- Rust core with a Tauri 2 desktop shell and React configuration UI.
- Multilingual UI and documentation with English as the default language, plus Simplified Chinese, Traditional Chinese, Japanese, and Korean.
- Windows native wallpaper control through `SystemParametersInfoW`, using a generated virtual-desktop-sized wallpaper for every refresh.
- macOS beta wallpaper setting through AppleScript.
- Linux beta wallpaper setting for GNOME, Cinnamon, Unity, KDE Plasma, and Xfce on a best-effort basis.
- Breed-aware and preference-aware source planning for TheCatAPI, Wikimedia Commons, local folders, Pexels, Pixabay, Magnific, and AI-generated assets.
- Hard HD policy: images below `2560x1440` are rejected before being written to the managed gallery.
- Fixed app-managed gallery under the user data directory, split into downloaded images and AI-generated images.
- Built-in gallery browser with preview, quality badges, manual use, and delete controls.
- AI transparent-cat generation for desktop-pet style PNG cutouts. OpenAI uses a transparent-capable image model by default; Google Nano Banana Pro is exposed as a user-key beta provider.
- Desktop effect analysis checks final wallpaper dimensions, virtual desktop fit, resolution, placeholder art, transparency, possible cropping, and preference-match uncertainty.
- Feedback learning records like/dislike decisions and user reasons, then avoids images that are repeatedly rejected.
- Optional interaction preferences for breathing, mouse proximity, click paw, keyboard Bongo, and sound. Sound is off by default.

## Closed-Loop Learning

Daily Cat Wallpaper treats wallpaper selection as an iterative system:

1. Read user preferences: breed, color/type, cat count, scene, source priority, refresh schedule, interaction style, and language.
2. Resolve candidates from local files, managed gallery, online sources, or AI generation.
3. Reject low-resolution images before saving them.
4. Compose a wallpaper for the actual virtual desktop, keeping taskbar and icon-safe areas in mind.
5. Analyze the final wallpaper result and report issues such as undersized output, placeholder art, non-transparent pet assets, likely cropping, or uncertain preference match.
6. Let the user like or dislike the result and add a reason.
7. Persist feedback in the app data directory and avoid rejected images in future selection.

This is the main mechanism that lets the app become more aligned with the user over time.

## Image Quality Policy

The default minimum accepted resolution is `2560x1440`. Low-resolution fallback is disabled. This applies before downloaded images are written to disk, so weak candidates do not pollute the managed gallery.

Existing local files can still be displayed in the gallery with quality badges so the user can clean them up manually.

## AI Cat Generation

The AI generation panel creates prompts from the current UI settings: breed, type/color, scene, quantity, and transparent cutout preference.

Generated images are saved under:

- `cat-gallery/generated`

Downloaded images are saved under:

- `cat-gallery/downloads`

Transparent cat generation is intended for lifelike full-body PNG cutouts with no background. These assets are then composed into a desktop-sized wallpaper. A true always-on-top transparent desktop-pet interaction layer is still future work; the current release focuses on static wallpaper composition plus feedback learning.

## Image Sources

Default and optional sources:

| Source | Status | Notes |
| --- | --- | --- |
| Local folders | Stable | Best source for user-curated HD images. |
| Managed gallery | Stable | Fixed download and generation locations. |
| Wikimedia Commons | Public fallback | Uses breed and type keywords when possible. |
| TheCatAPI | Public/API-key friendly | Breed IDs are used for known breeds. |
| CATAAS | Public fallback | Useful as a last online fallback. |
| Pexels | Optional key | Higher-quality source when the user provides an API key. |
| Pixabay | Optional key | Higher-quality source when the user provides an API key. |
| Magnific | Experimental | Depends on API availability and user configuration. |
| OpenAI Images | Optional key | Transparent cat cutouts by default. |
| Google Nano Banana Pro | Optional key, beta | Image generation provider for configured scenes. |

## Configuration

The app configuration covers:

- Language: automatic locale detection or explicit language selection.
- Breeds and cat type/color preferences.
- Cat count strategy: match display count by default, or fixed `1..5`.
- Image mood and scene.
- Source priority and API keys.
- Local gallery folders.
- AI generation provider, model, prompt scene, count, and auto-use behavior.
- Refresh schedule: login, daily time, interval, or manual refresh.
- Platform mode and startup behavior.
- Interaction preferences and sound toggle.

## Platform Status

| Platform | Status | Notes |
| --- | --- | --- |
| Windows x64/arm64 | Stable target | Native static wallpaper setting is implemented. |
| macOS x64/arm64 | Beta | Static wallpaper setting uses AppleScript. |
| Linux x64/arm64 | Beta | Supports common desktop environments on a best-effort basis. |

## Install

Download the latest builds from the GitHub Releases page:

[Daily Cat Wallpaper Releases](https://github.com/mgcpngn/daily-cat-wallpaper/releases)

Published targets include Windows, macOS, and Linux builds for x64 and arm64 where the GitHub Actions matrix supports them.

## Development

Prerequisites:

- Rust stable toolchain with `rustfmt` and `clippy`.
- Node.js 22 or newer.
- Tauri platform prerequisites for your operating system.

Install dependencies:

```powershell
npm install
```

Run checks:

```powershell
cargo test --workspace
npm run build
cargo check
```

Run the app in development:

```powershell
npm run tauri dev
```

Build a production app:

```powershell
npm run tauri:build
```

## Current Limits

- Windows is the primary stable target. macOS and Linux wallpaper setting is beta.
- Transparent AI cats are composed into static wallpapers. A real transparent overlay desktop-pet layer is planned but not complete.
- Online source quality depends on provider APIs, rate limits, and user-provided keys.
- Breed matching is best-effort. The feedback loop is the authority when the user rejects a result.

## License

MIT
