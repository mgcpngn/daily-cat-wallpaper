use daily_cat_core::{AppConfig, CatCandidate, ImageQuality, SourceError, SourcePlanner};
use directories::ProjectDirs;
use image::GenericImage;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppStateError {
    #[error(transparent)]
    Config(#[from] daily_cat_core::ConfigError),
    #[error(transparent)]
    Source(#[from] SourceError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Network(#[from] reqwest::Error),
    #[error(transparent)]
    Image(#[from] image::ImageError),
    #[error("no image candidate could be resolved")]
    NoCandidate,
    #[error("no display geometry is available")]
    NoDisplays,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct DisplayGeometry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub struct AppState {
    data_dir: PathBuf,
    client: reqwest::Client,
}

impl AppState {
    pub fn new() -> Self {
        let data_dir = ProjectDirs::from("com", "dailycat", "DailyCatWallpaper")
            .map(|dirs| dirs.data_local_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from(".daily-cat-wallpaper"));

        Self {
            data_dir,
            client: reqwest::Client::new(),
        }
    }

    pub fn load_config(&self) -> Result<AppConfig, AppStateError> {
        let path = self.config_path();
        if !path.exists() {
            return Ok(AppConfig::default());
        }

        let config = serde_json::from_slice::<AppConfig>(&fs::read(path)?)?;
        config.validate()?;
        Ok(config)
    }

    pub fn save_config(&self, config: AppConfig) -> Result<AppConfig, AppStateError> {
        config.validate()?;
        fs::create_dir_all(&self.data_dir)?;
        fs::write(self.config_path(), serde_json::to_vec_pretty(&config)?)?;
        Ok(config)
    }

    pub async fn resolve_wallpaper_image(
        &self,
        planner: &SourcePlanner,
        quality: &ImageQuality,
    ) -> Result<PathBuf, AppStateError> {
        let sources = planner.ordered_sources()?;
        for source in sources {
            if let Some(path) = self.resolve_source(&source, quality, &[]).await? {
                return Ok(path);
            }
        }

        Err(AppStateError::NoCandidate)
    }

    pub async fn resolve_wallpaper_images(
        &self,
        planner: &SourcePlanner,
        quality: &ImageQuality,
        count: usize,
    ) -> Result<Vec<PathBuf>, AppStateError> {
        if count.max(1) == 1 {
            return Ok(vec![self.resolve_wallpaper_image(planner, quality).await?]);
        }

        let mut paths = Vec::new();
        for _ in 0..count.max(1) {
            paths.push(
                self.resolve_wallpaper_image_excluding(planner, quality, &paths)
                    .await?,
            );
        }
        Ok(paths)
    }

    pub fn compose_wallpaper(
        &self,
        displays: &[DisplayGeometry],
        image_paths: &[PathBuf],
        assignments: &[usize],
    ) -> Result<PathBuf, AppStateError> {
        let output_dir = self.data_dir.join("generated");
        fs::create_dir_all(&output_dir)?;
        compose_display_wallpaper(&output_dir, displays, image_paths, assignments)
    }

    fn config_path(&self) -> PathBuf {
        self.data_dir.join("config.json")
    }

    async fn resolve_wallpaper_image_excluding(
        &self,
        planner: &SourcePlanner,
        quality: &ImageQuality,
        excluded: &[PathBuf],
    ) -> Result<PathBuf, AppStateError> {
        let sources = planner.ordered_sources()?;
        for source in sources {
            if let Some(path) = self.resolve_source(&source, quality, excluded).await? {
                return Ok(path);
            }
        }

        Err(AppStateError::NoCandidate)
    }

    async fn resolve_source(
        &self,
        source: &str,
        quality: &ImageQuality,
        excluded: &[PathBuf],
    ) -> Result<Option<PathBuf>, AppStateError> {
        if let Some(dir) = source.strip_prefix("local:") {
            return Ok(first_local_image(Path::new(dir), quality, excluded));
        }

        let candidates = match source {
            "cataas" => Some(vec![CatCandidate {
                id: format!(
                    "cataas-{}x{}-{}",
                    quality.preferred_width,
                    quality.preferred_height,
                    unique_suffix()
                ),
                uri: format!(
                    "https://cataas.com/cat?width={}&height={}",
                    quality.preferred_width, quality.preferred_height
                ),
                source: "cataas".to_string(),
            }]),
            "thecatapi" => self.the_cat_api_candidates().await?,
            _ => None,
        };

        let Some(candidates) = candidates else {
            return Ok(None);
        };

        let cache_dir = self.data_dir.join("cache");
        fs::create_dir_all(&cache_dir)?;
        let mut fallback = None;
        for candidate in candidates {
            let extension = image_extension_from_uri(&candidate.uri).unwrap_or("jpg");
            let output_path = cache_dir.join(format!(
                "{}.{}",
                sanitize_file_name(&candidate.id),
                extension
            ));
            let bytes = self
                .client
                .get(&candidate.uri)
                .send()
                .await?
                .bytes()
                .await?;
            fs::write(&output_path, bytes)?;

            if image_meets_quality(&output_path, quality) && !excluded.contains(&output_path) {
                return Ok(Some(output_path));
            }

            if fallback.is_none() && quality.allow_low_resolution_fallback {
                fallback = Some(output_path);
            }
        }

        Ok(fallback)
    }

    async fn the_cat_api_candidates(&self) -> Result<Option<Vec<CatCandidate>>, AppStateError> {
        #[derive(Debug, Deserialize)]
        struct TheCatApiImage {
            id: String,
            url: String,
        }

        let images = self
            .client
            .get("https://api.thecatapi.com/v1/images/search?mime_types=jpg,png&size=full&limit=10")
            .send()
            .await?
            .json::<Vec<TheCatApiImage>>()
            .await?;

        let candidates = images
            .into_iter()
            .map(|image| CatCandidate {
                id: image.id,
                uri: image.url,
                source: "thecatapi".to_string(),
            })
            .collect::<Vec<_>>();

        if candidates.is_empty() {
            Ok(None)
        } else {
            Ok(Some(candidates))
        }
    }
}

fn first_local_image(dir: &Path, quality: &ImageQuality, excluded: &[PathBuf]) -> Option<PathBuf> {
    let entries = fs::read_dir(dir).ok()?;
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.is_file()
                && is_supported_image(path)
                && image_meets_quality(path, quality)
                && !excluded.contains(path)
        })
}

fn is_supported_image(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "jpg" | "jpeg" | "png" | "webp"
            )
        })
        .unwrap_or(false)
}

fn image_meets_quality(path: &Path, quality: &ImageQuality) -> bool {
    image::image_dimensions(path)
        .map(|(width, height)| width >= quality.min_width && height >= quality.min_height)
        .unwrap_or(false)
}

fn image_extension_from_uri(uri: &str) -> Option<&'static str> {
    let path = uri.split('?').next().unwrap_or(uri);
    let extension = Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())?
        .to_ascii_lowercase();
    match extension.as_str() {
        "jpg" | "jpeg" => Some("jpg"),
        "png" => Some("png"),
        "webp" => Some("webp"),
        _ => None,
    }
}

fn compose_display_wallpaper(
    output_dir: &Path,
    displays: &[DisplayGeometry],
    image_paths: &[PathBuf],
    assignments: &[usize],
) -> Result<PathBuf, AppStateError> {
    if displays.is_empty() {
        return Err(AppStateError::NoDisplays);
    }
    if image_paths.is_empty() {
        return Err(AppStateError::NoCandidate);
    }

    let min_x = displays.iter().map(|display| display.x).min().unwrap_or(0);
    let min_y = displays.iter().map(|display| display.y).min().unwrap_or(0);
    let max_x = displays
        .iter()
        .map(|display| display.x.saturating_add(display.width as i32))
        .max()
        .unwrap_or(0);
    let max_y = displays
        .iter()
        .map(|display| display.y.saturating_add(display.height as i32))
        .max()
        .unwrap_or(0);
    let width = (max_x - min_x).max(1) as u32;
    let height = (max_y - min_y).max(1) as u32;
    let mut canvas = image::RgbaImage::from_pixel(width, height, image::Rgba([18, 20, 24, 255]));

    for (display_index, display) in displays.iter().enumerate() {
        let assignment = assignments.get(display_index).copied().unwrap_or(0);
        let image_path = image_paths
            .get(assignment)
            .or_else(|| image_paths.first())
            .ok_or(AppStateError::NoCandidate)?;
        let tile = image::open(image_path)?
            .resize_to_fill(
                display.width.max(1),
                display.height.max(1),
                image::imageops::FilterType::Lanczos3,
            )
            .to_rgba8();
        let x = display.x.saturating_sub(min_x) as u32;
        let y = display.y.saturating_sub(min_y) as u32;
        canvas.copy_from(&tile, x, y)?;
    }

    fs::create_dir_all(output_dir)?;
    let output_path = output_dir.join(format!("wallpaper-{}.png", unique_suffix()));
    image::DynamicImage::ImageRgba8(canvas).save(&output_path)?;
    Ok(output_path)
}

fn sanitize_file_name(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

fn unique_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use daily_cat_core::ImageQuality;
    use image::{Rgb, RgbImage};
    use tempfile::tempdir;

    #[test]
    fn image_meets_quality_requires_minimum_dimensions() {
        let dir = tempdir().unwrap();
        let small_path = dir.path().join("small.jpg");
        let large_path = dir.path().join("large.jpg");
        RgbImage::from_pixel(1280, 720, Rgb([240, 170, 130]))
            .save(&small_path)
            .unwrap();
        RgbImage::from_pixel(2560, 1440, Rgb([240, 170, 130]))
            .save(&large_path)
            .unwrap();
        let quality = ImageQuality::default();

        assert!(!image_meets_quality(&small_path, &quality));
        assert!(image_meets_quality(&large_path, &quality));
    }

    #[test]
    fn first_local_image_skips_low_resolution_files() {
        let dir = tempdir().unwrap();
        RgbImage::from_pixel(1280, 720, Rgb([240, 170, 130]))
            .save(dir.path().join("a-small.jpg"))
            .unwrap();
        RgbImage::from_pixel(2560, 1440, Rgb([240, 170, 130]))
            .save(dir.path().join("b-large.jpg"))
            .unwrap();

        let selected = first_local_image(dir.path(), &ImageQuality::default(), &[]).unwrap();

        assert_eq!(
            selected.file_name().and_then(|name| name.to_str()),
            Some("b-large.jpg")
        );
    }

    #[test]
    fn first_local_image_skips_already_selected_files() {
        let dir = tempdir().unwrap();
        let first = dir.path().join("a-cat.jpg");
        let second = dir.path().join("b-cat.jpg");
        RgbImage::from_pixel(2560, 1440, Rgb([250, 120, 90]))
            .save(&first)
            .unwrap();
        RgbImage::from_pixel(2560, 1440, Rgb([90, 150, 250]))
            .save(&second)
            .unwrap();

        let selected = first_local_image(
            dir.path(),
            &ImageQuality::default(),
            std::slice::from_ref(&first),
        )
        .unwrap();

        assert_eq!(selected, second);
    }

    #[test]
    fn compose_display_wallpaper_reuses_single_image_for_multiple_displays() {
        let dir = tempdir().unwrap();
        let cat_path = dir.path().join("same-cat.png");
        RgbImage::from_pixel(80, 60, Rgb([250, 120, 90]))
            .save(&cat_path)
            .unwrap();
        let displays = vec![
            DisplayGeometry {
                x: 0,
                y: 0,
                width: 80,
                height: 60,
            },
            DisplayGeometry {
                x: 80,
                y: 0,
                width: 80,
                height: 60,
            },
        ];

        let output =
            compose_display_wallpaper(dir.path(), &displays, &[cat_path], &[0, 0]).unwrap();
        let composed = image::open(output).unwrap().to_rgb8();

        assert_eq!(composed.dimensions(), (160, 60));
        assert_eq!(composed.get_pixel(20, 20), &Rgb([250, 120, 90]));
        assert_eq!(composed.get_pixel(100, 20), &Rgb([250, 120, 90]));
    }
}
