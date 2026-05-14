mod state;
mod wallpaper_backend;

use daily_cat_core::{
    AppConfig, BackendCapabilities, Canvas, LayoutEngine, Rect, SafeArea, SourcePlanner,
    WallpaperBackend,
};
use state::{
    AppState, DisplayGeometry, FeedbackInput, GalleryImage, ImportImagePayload, LearningSummary,
    WallpaperAnalysis, WallpaperResult,
};
use std::path::PathBuf;
use std::process::Command;
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
async fn refresh_wallpaper(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<WallpaperResult, String> {
    let config = state.load_config().map_err(|error| error.to_string())?;
    let source_planner = source_planner_from_config(&config);
    let displays = display_geometries_or_default(&app, &config);
    let assignments = LayoutEngine.cat_assignments(displays.len(), &config);
    let unique_cat_count = assignments.iter().max().map(|index| index + 1).unwrap_or(1);
    let (image_paths, effective_assignments) = if let Some(selected) = config
        .sources
        .selected_gallery_image
        .as_deref()
        .map(PathBuf::from)
        .filter(|path| {
            state
                .gallery_image_is_usable(path, &config.image_quality)
                .unwrap_or(false)
        }) {
        (vec![selected], vec![0; displays.len().max(1)])
    } else {
        (
            state
                .resolve_wallpaper_images(&source_planner, &config.image_quality, unique_cat_count)
                .await
                .map_err(|error| error.to_string())?,
            assignments,
        )
    };
    let image_path = state
        .compose_wallpaper(&displays, &image_paths, &effective_assignments)
        .map_err(|error| error.to_string())?;
    let analysis = state
        .analyze_wallpaper(&image_path, &displays, &image_paths, &config.image_quality)
        .map_err(|error| error.to_string())?;

    NativeWallpaperBackend::new()
        .set_wallpaper(&image_path)
        .map_err(|error| error.to_string())?;

    Ok(WallpaperResult {
        path: image_path,
        analysis,
    })
}

#[tauri::command]
fn list_gallery_images(state: State<'_, AppState>) -> Result<Vec<GalleryImage>, String> {
    let config = state.load_config().map_err(|error| error.to_string())?;
    state
        .list_gallery_images(&config.image_quality)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_gallery_image(state: State<'_, AppState>, path: PathBuf) -> Result<(), String> {
    state
        .delete_gallery_image(&path)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn gallery_location(state: State<'_, AppState>) -> PathBuf {
    state.gallery_dir()
}

#[tauri::command]
fn open_gallery_folder(state: State<'_, AppState>) -> Result<(), String> {
    let path = state.gallery_dir();
    std::fs::create_dir_all(&path).map_err(|error| error.to_string())?;
    open_directory(&path).map_err(|error| error.to_string())
}

#[tauri::command]
fn import_gallery_images(
    state: State<'_, AppState>,
    payloads: Vec<ImportImagePayload>,
) -> Result<Vec<GalleryImage>, String> {
    let config = state.load_config().map_err(|error| error.to_string())?;
    state
        .import_gallery_payloads(&payloads, &config.image_quality)
        .map_err(|error| {
            if error.to_string() == "no image candidate could be resolved" {
                "No selected image met the 2K quality gate. Choose a transparent or HD cat image."
                    .to_string()
            } else {
                error.to_string()
            }
        })
}

#[tauri::command]
async fn select_gallery_image(
    app: AppHandle,
    state: State<'_, AppState>,
    path: PathBuf,
) -> Result<WallpaperResult, String> {
    let mut config = state.load_config().map_err(|error| error.to_string())?;
    if !state
        .gallery_image_is_usable(&path, &config.image_quality)
        .map_err(|error| error.to_string())?
    {
        return Err("selected cat image does not meet the 2K quality gate".to_string());
    }
    config.sources.selected_gallery_image = Some(path.to_string_lossy().to_string());
    state
        .save_config(config)
        .map_err(|error| error.to_string())?;
    refresh_wallpaper(app, state).await
}

#[tauri::command]
async fn generate_ai_cat_images(
    state: State<'_, AppState>,
    count: usize,
) -> Result<Vec<GalleryImage>, String> {
    let mut config = state.load_config().map_err(|error| error.to_string())?;
    let paths = state
        .generate_ai_cat_images(&config, count)
        .await
        .map_err(|error| error.to_string())?;
    if config.ai_generation.auto_use_generated {
        if let Some(path) = paths.first() {
            config.sources.selected_gallery_image = Some(path.to_string_lossy().to_string());
            state
                .save_config(config.clone())
                .map_err(|error| error.to_string())?;
        }
    }
    state
        .list_gallery_images(&config.image_quality)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn record_wallpaper_feedback(
    state: State<'_, AppState>,
    input: FeedbackInput,
) -> Result<LearningSummary, String> {
    state
        .record_feedback(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn learning_summary(state: State<'_, AppState>) -> Result<LearningSummary, String> {
    state.learning_summary().map_err(|error| error.to_string())
}

#[tauri::command]
fn analyze_wallpaper(
    app: AppHandle,
    state: State<'_, AppState>,
    path: PathBuf,
) -> Result<WallpaperAnalysis, String> {
    let config = state.load_config().map_err(|error| error.to_string())?;
    let displays = display_geometries_or_default(&app, &config);
    state
        .analyze_wallpaper(&path, &displays, &[], &config.image_quality)
        .map_err(|error| error.to_string())
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

fn open_directory(path: &std::path::Path) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer").arg(path).spawn()?;
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(path).spawn()?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open").arg(path).spawn()?;
    }
    Ok(())
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
            prefetch_wallpapers,
            list_gallery_images,
            delete_gallery_image,
            gallery_location,
            open_gallery_folder,
            import_gallery_images,
            select_gallery_image,
            generate_ai_cat_images,
            record_wallpaper_feedback,
            learning_summary,
            analyze_wallpaper
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Daily Cat Wallpaper");
}
