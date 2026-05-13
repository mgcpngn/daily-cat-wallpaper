# Architecture

Daily Cat Wallpaper separates desktop control from user configuration.

## Core Crate

`crates/daily-cat-core` owns the portable domain model:

- `AppConfig` validates user preferences.
- `SourcePlanner` orders local and online cat sources.
- `LayoutEngine` computes safe wallpaper regions for 1-5 cats.
- `Scheduler` decides whether a refresh should run.
- `WallpaperBackend` defines the platform contract.

The core crate has no Tauri dependency and is covered by Rust tests.

## Tauri Shell

`src-tauri` owns host integration:

- Loads and saves config under the user-local app data directory.
- Resolves local or remote cat image candidates.
- Calls the current platform wallpaper backend.
- Exposes Tauri commands for the React configuration UI.

Windows uses native Win32 wallpaper APIs. macOS and Linux use command-based beta backends because their wallpaper APIs vary by desktop environment.

## Frontend

`src/main.tsx` is the configuration console. It reads and writes the Rust config, previews layout slots, and exposes manual refresh. It intentionally keeps all durable behavior in Rust so the UI can evolve without changing platform control logic.
