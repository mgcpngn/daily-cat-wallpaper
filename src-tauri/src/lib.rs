mod state;
mod wallpaper_backend;

use daily_cat_core::{
    AppConfig, BackendCapabilities, Canvas, LayoutEngine, Rect, SafeArea, SourcePlanner,
    WallpaperBackend,
};
use state::{AppState, DisplayGeometry};
use std::path::PathBuf;
use tauri::{AppHandle, State};
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
fn display_summary(app: AppHandle) -> Vec<DisplayGeometry> {
    monitor_geometries(&app)
}

#[tauri::command]
async fn refresh_wallpaper(app: AppHandle, state: State<'_, AppState>) -> Result<PathBuf, String> {
    let config = state.load_config().map_err(|error| error.to_string())?;
    let source_planner = source_planner_from_config(&config);
    let displays = display_geometries_or_default(&app, &config);
    let assignments = LayoutEngine.cat_assignments(displays.len(), &config);
    let unique_cat_count = assignments.iter().max().map(|index| index + 1).unwrap_or(1);
    let image_paths = state
        .resolve_wallpaper_images(&source_planner, &config.image_quality, unique_cat_count)
        .await
        .map_err(|error| error.to_string())?;
    let image_path = if displays.len() > 1 {
        state
            .compose_wallpaper(&displays, &image_paths, &assignments)
            .map_err(|error| error.to_string())?
    } else {
        image_paths
            .first()
            .cloned()
            .ok_or_else(|| "no image candidate could be resolved".to_string())?
    };

    NativeWallpaperBackend::new()
        .set_wallpaper(&image_path)
        .map_err(|error| error.to_string())?;

    Ok(image_path)
}

#[tauri::command]
async fn prefetch_wallpapers(
    state: State<'_, AppState>,
    count: usize,
) -> Result<Vec<PathBuf>, String> {
    let config = state.load_config().map_err(|error| error.to_string())?;
    let source_planner = source_planner_from_config(&config);
    state
        .resolve_wallpaper_images(&source_planner, &config.image_quality, count.clamp(1, 24))
        .await
        .map_err(|error| error.to_string())
}

fn source_planner_from_config(config: &AppConfig) -> SourcePlanner {
    SourcePlanner {
        local_dirs: config
            .sources
            .local_dirs
            .iter()
            .map(PathBuf::from)
            .collect(),
        wikimedia_commons_enabled: config.sources.wikimedia_commons,
        cataas_enabled: config.sources.cataas,
        the_cat_api_enabled: config.sources.the_cat_api,
        breeds: config.breeds.clone(),
        image_types: config.image_types.clone(),
        pixabay_api_key: config.sources.pixabay_api_key.clone(),
        magnific_api_key: config.sources.magnific_api_key.clone(),
        pexels_api_key: config.sources.pexels_api_key.clone(),
    }
}

fn display_geometries_or_default(app: &AppHandle, config: &AppConfig) -> Vec<DisplayGeometry> {
    let displays = monitor_geometries(app);
    if displays.is_empty() {
        vec![DisplayGeometry {
            x: 0,
            y: 0,
            width: config.image_quality.preferred_width,
            height: config.image_quality.preferred_height,
        }]
    } else {
        displays
    }
}

fn monitor_geometries(app: &AppHandle) -> Vec<DisplayGeometry> {
    app.available_monitors()
        .map(|monitors| {
            monitors
                .into_iter()
                .map(|monitor| DisplayGeometry {
                    x: monitor.position().x,
                    y: monitor.position().y,
                    width: monitor.size().width,
                    height: monitor.size().height,
                })
                .collect()
        })
        .unwrap_or_default()
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
            display_summary,
            refresh_wallpaper,
            prefetch_wallpapers
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Daily Cat Wallpaper");
}
