mod state;
mod wallpaper_backend;

use daily_cat_core::{
    AppConfig, BackendCapabilities, Canvas, LayoutEngine, Rect, SafeArea, SourcePlanner,
    WallpaperBackend,
};
use state::AppState;
use std::path::PathBuf;
use tauri::State;
use wallpaper_backend::NativeWallpaperBackend;

#[tauri::command]
fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    state.load_config().map_err(|error| error.to_string())
}

#[tauri::command]
fn save_config(state: State<'_, AppState>, config: AppConfig) -> Result<AppConfig, String> {
    state.save_config(config).map_err(|error| error.to_string())
}

#[tauri::command]
fn preview_layout(cat_count: u8, width: u32, height: u32) -> Vec<Rect> {
    LayoutEngine.slots(
        Canvas { width, height },
        SafeArea {
            left: 420,
            right: 80,
            top: 80,
            bottom: 160,
        },
        cat_count,
    )
}

#[tauri::command]
fn platform_capabilities() -> BackendCapabilities {
    NativeWallpaperBackend::new().capabilities()
}

#[tauri::command]
async fn refresh_wallpaper(state: State<'_, AppState>) -> Result<PathBuf, String> {
    let config = state.load_config().map_err(|error| error.to_string())?;
    let source_planner = SourcePlanner {
        local_dirs: config
            .sources
            .local_dirs
            .iter()
            .map(PathBuf::from)
            .collect(),
        cataas_enabled: config.sources.cataas,
        the_cat_api_enabled: config.sources.the_cat_api,
    };
    let image_path = state
        .resolve_wallpaper_image(&source_planner)
        .await
        .map_err(|error| error.to_string())?;

    NativeWallpaperBackend::new()
        .set_wallpaper(&image_path)
        .map_err(|error| error.to_string())?;

    Ok(image_path)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            get_config,
            save_config,
            preview_layout,
            platform_capabilities,
            refresh_wallpaper
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Daily Cat Wallpaper");
}
