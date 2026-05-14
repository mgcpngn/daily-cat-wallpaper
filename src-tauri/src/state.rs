use daily_cat_core::config::CatImageType;
use daily_cat_core::sources::{
    magnific_search_query, pexels_search_query, pixabay_search_query, the_cat_api_breed_ids,
    wikimedia_search_query,
};
use daily_cat_core::{AppConfig, CatCandidate, ImageQuality, SourceError, SourcePlanner};
use directories::ProjectDirs;
use image::GenericImage;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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

impl AppStateError {
    fn is_recoverable_source_error(&self) -> bool {
        matches!(
            self,
            Self::Network(_) | Self::Json(_) | Self::Image(_) | Self::NoCandidate
        )
    }
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
            match self.resolve_source(&source, quality, &[], planner).await {
                Ok(Some(path)) => return Ok(path),
                Ok(None) => {}
                Err(error) if error.is_recoverable_source_error() => {}
                Err(error) => return Err(error),
            }
        }

        if let Some(path) = previous_cached_image(&self.data_dir, quality, &[]) {
            return Ok(path);
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
            match self.resolve_source(&source, quality, excluded, planner).await {
                Ok(Some(path)) => return Ok(path),
                Ok(None) => {}
                Err(error) if error.is_recoverable_source_error() => {}
                Err(error) => return Err(error),
            }
        }

        if let Some(path) = previous_cached_image(&self.data_dir, quality, excluded) {
            return Ok(path);
        }

        Err(AppStateError::NoCandidate)
    }

    async fn resolve_source(
        &self,
        source: &str,
        quality: &ImageQuality,
        excluded: &[PathBuf],
        planner: &SourcePlanner,
    ) -> Result<Option<PathBuf>, AppStateError> {
        if let Some(dir) = source.strip_prefix("local:") {
            return Ok(first_local_image(Path::new(dir), quality, excluded));
        }

        let candidates = match source {
            "magnific" => self
                .magnific_candidates(
                    planner.magnific_api_key.as_deref().unwrap_or_default(),
                    &planner.breeds,
                    &planner.image_types,
                )
                .await?,
            "pixabay" => self
                .pixabay_candidates(
                    planner.pixabay_api_key.as_deref().unwrap_or_default(),
                    &planner.breeds,
                    &planner.image_types,
                    quality,
                )
                .await?,
            "pexels" => self
                .pexels_candidates(
                    planner.pexels_api_key.as_deref().unwrap_or_default(),
                    &planner.breeds,
                    &planner.image_types,
                    quality,
                )
                .await?,
            "wikimedia" => self
                .wikimedia_candidates(&planner.breeds, &planner.image_types)
                .await?,
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
                width: Some(quality.preferred_width),
                height: Some(quality.preferred_height),
            }]),
            "thecatapi" => self.the_cat_api_candidates(&planner.breeds).await?,
            "generated" => {
                let output_dir = self.data_dir.join("generated");
                let path = generate_fallback_cat_image(&output_dir, quality, &planner.breeds)?;
                return Ok(Some(path));
            }
            _ => None,
        };

        let Some(candidates) = candidates else {
            return Ok(None);
        };

        let cache_dir = self.data_dir.join("cache");
        fs::create_dir_all(&cache_dir)?;
        let mut fallback = None;
        for candidate in candidates {
            if !candidate_meets_quality(&candidate, quality) {
                continue;
            }
            let extension = image_extension_from_uri(&candidate.uri).unwrap_or("jpg");
            let output_path = cache_dir.join(format!(
                "{}.{}",
                sanitize_file_name(&candidate.id),
                extension
            ));
            let Ok(response) = self.client.get(&candidate.uri).send().await else {
                continue;
            };
            let Ok(bytes) = response.bytes().await else {
                continue;
            };
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

    async fn magnific_candidates(
        &self,
        api_key: &str,
        breeds: &[String],
        image_types: &[CatImageType],
    ) -> Result<Option<Vec<CatCandidate>>, AppStateError> {
        if api_key.trim().is_empty() {
            return Ok(None);
        }

        #[derive(Debug, Deserialize)]
        struct MagnificResourcesResponse {
            data: Vec<MagnificResource>,
        }

        #[derive(Debug, Deserialize)]
        struct MagnificResource {
            id: String,
        }

        #[derive(Debug, Deserialize)]
        struct MagnificDownloadResponse {
            data: MagnificDownloadData,
        }

        #[derive(Debug, Deserialize)]
        struct MagnificDownloadData {
            #[serde(alias = "signed_url", alias = "download_url")]
            url: String,
        }

        let query = magnific_search_query(breeds, image_types);
        let resources = self
            .client
            .get("https://api.magnific.com/v1/resources")
            .header("x-magnific-api-key", api_key.trim())
            .query(&[("query", query.as_str()), ("type", "image"), ("limit", "12")])
            .send()
            .await?
            .json::<MagnificResourcesResponse>()
            .await?;

        let mut candidates = Vec::new();
        for resource in resources.data {
            let download = self
                .client
                .get(format!(
                    "https://api.magnific.com/v1/resources/{}/download",
                    resource.id
                ))
                .header("x-magnific-api-key", api_key.trim())
                .query(&[("image_size", "original")])
                .send()
                .await?
                .json::<MagnificDownloadResponse>()
                .await?;
            candidates.push(CatCandidate {
                id: format!("magnific-{}", resource.id),
                uri: download.data.url,
                source: "magnific".to_string(),
                width: None,
                height: None,
            });
        }

        if candidates.is_empty() {
            Ok(None)
        } else {
            Ok(Some(candidates))
        }
    }

    async fn pixabay_candidates(
        &self,
        api_key: &str,
        breeds: &[String],
        image_types: &[CatImageType],
        quality: &ImageQuality,
    ) -> Result<Option<Vec<CatCandidate>>, AppStateError> {
        if api_key.trim().is_empty() {
            return Ok(None);
        }

        #[derive(Debug, Deserialize)]
        struct PixabayResponse {
            hits: Vec<PixabayImage>,
        }

        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct PixabayImage {
            id: u64,
            large_image_url: Option<String>,
            full_hd_url: Option<String>,
            image_url: Option<String>,
            image_width: u32,
            image_height: u32,
        }

        let query = pixabay_search_query(breeds, image_types);
        let response = self
            .client
            .get("https://pixabay.com/api/")
            .query(&[
                ("key", api_key.trim()),
                ("q", query.as_str()),
                ("image_type", "photo"),
                ("orientation", "horizontal"),
                ("category", "animals"),
                ("safesearch", "true"),
                ("order", "popular"),
                ("per_page", "30"),
            ])
            .send()
            .await?
            .json::<PixabayResponse>()
            .await?;

        let candidates = response
            .hits
            .into_iter()
            .filter_map(|image| {
                let (uri, width, height) = if let Some(url) = image.image_url {
                    (url, image.image_width, image.image_height)
                } else if let Some(url) = image.full_hd_url {
                    (url, 1920, 1080)
                } else if quality.allow_low_resolution_fallback {
                    (image.large_image_url?, 1280, 960)
                } else {
                    return None;
                };
                Some(CatCandidate {
                    id: format!("pixabay-{}", image.id),
                    uri,
                    source: "pixabay".to_string(),
                    width: Some(width),
                    height: Some(height),
                })
            })
            .collect::<Vec<_>>();

        if candidates.is_empty() {
            Ok(None)
        } else {
            Ok(Some(candidates))
        }
    }

    async fn pexels_candidates(
        &self,
        api_key: &str,
        breeds: &[String],
        image_types: &[CatImageType],
        quality: &ImageQuality,
    ) -> Result<Option<Vec<CatCandidate>>, AppStateError> {
        if api_key.trim().is_empty() {
            return Ok(None);
        }

        #[derive(Debug, Deserialize)]
        struct PexelsResponse {
            photos: Vec<PexelsPhoto>,
        }

        #[derive(Debug, Deserialize)]
        struct PexelsPhoto {
            id: u64,
            width: u32,
            height: u32,
            src: PexelsSrc,
        }

        #[derive(Debug, Deserialize)]
        struct PexelsSrc {
            original: String,
            large2x: Option<String>,
        }

        let query = pexels_search_query(breeds, image_types);
        let response = self
            .client
            .get("https://api.pexels.com/v1/search")
            .bearer_auth(api_key.trim())
            .query(&[
                ("query", query.as_str()),
                ("orientation", "landscape"),
                ("per_page", "30"),
            ])
            .send()
            .await?
            .json::<PexelsResponse>()
            .await?;

        let candidates = response
            .photos
            .into_iter()
            .filter_map(|photo| {
                let uri = if photo.width >= quality.min_width && photo.height >= quality.min_height
                {
                    photo.src.original
                } else {
                    photo.src.large2x?
                };
                Some(CatCandidate {
                    id: format!("pexels-{}", photo.id),
                    uri,
                    source: "pexels".to_string(),
                    width: Some(photo.width),
                    height: Some(photo.height),
                })
            })
            .collect::<Vec<_>>();

        if candidates.is_empty() {
            Ok(None)
        } else {
            Ok(Some(candidates))
        }
    }

    async fn wikimedia_candidates(
        &self,
        breeds: &[String],
        image_types: &[CatImageType],
    ) -> Result<Option<Vec<CatCandidate>>, AppStateError> {
        #[derive(Debug, Deserialize)]
        struct CommonsResponse {
            query: Option<CommonsQuery>,
        }

        #[derive(Debug, Deserialize)]
        struct CommonsQuery {
            pages: HashMap<String, CommonsPage>,
        }

        #[derive(Debug, Deserialize)]
        struct CommonsPage {
            title: String,
            imageinfo: Option<Vec<CommonsImageInfo>>,
        }

        #[derive(Debug, Deserialize)]
        struct CommonsImageInfo {
            url: String,
            width: u32,
            height: u32,
            mime: String,
        }

        let query = wikimedia_search_query(breeds, image_types);
        let response = self
            .client
            .get("https://commons.wikimedia.org/w/api.php")
            .query(&[
                ("action", "query"),
                ("format", "json"),
                ("generator", "search"),
                ("gsrnamespace", "6"),
                ("gsrlimit", "20"),
                ("prop", "imageinfo"),
                ("iiprop", "url|size|mime"),
                ("gsrsearch", query.as_str()),
            ])
            .send()
            .await?
            .json::<CommonsResponse>()
            .await?;

        let mut candidates = response
            .query
            .into_iter()
            .flat_map(|query| query.pages.into_values())
            .flat_map(|page| {
                page.imageinfo
                    .unwrap_or_default()
                    .into_iter()
                    .map(move |info| (page.title.clone(), info))
            })
            .filter(|(_, info)| matches!(info.mime.as_str(), "image/jpeg" | "image/png" | "image/webp"))
            .map(|(title, info)| CatCandidate {
                id: format!("commons-{title}"),
                uri: info.url,
                source: "wikimedia".to_string(),
                width: Some(info.width),
                height: Some(info.height),
            })
            .collect::<Vec<_>>();
        candidates.sort_by_key(|candidate| {
            std::cmp::Reverse(candidate.width.unwrap_or(0) as u64 * candidate.height.unwrap_or(0) as u64)
        });

        if candidates.is_empty() {
            Ok(None)
        } else {
            Ok(Some(candidates))
        }
    }

    async fn the_cat_api_candidates(
        &self,
        breeds: &[String],
    ) -> Result<Option<Vec<CatCandidate>>, AppStateError> {
        #[derive(Debug, Deserialize)]
        struct TheCatApiImage {
            id: String,
            url: String,
            width: Option<u32>,
            height: Option<u32>,
        }

        let mut request = self
            .client
            .get("https://api.thecatapi.com/v1/images/search")
            .query(&[
                ("mime_types", "jpg,png"),
                ("size", "full"),
                ("limit", "20"),
            ]);
        let breed_ids = the_cat_api_breed_ids(breeds);
        let breed_id_query = breed_ids.join(",");
        if !breed_id_query.is_empty() {
            request = request.query(&[("breed_ids", breed_id_query.as_str())]);
        }

        let images = request.send().await?.json::<Vec<TheCatApiImage>>().await?;

        let candidates = images
            .into_iter()
            .map(|image| CatCandidate {
                id: image.id,
                uri: image.url,
                source: "thecatapi".to_string(),
                width: image.width,
                height: image.height,
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

fn candidate_meets_quality(candidate: &CatCandidate, quality: &ImageQuality) -> bool {
    match (candidate.width, candidate.height) {
        (Some(width), Some(height)) => width >= quality.min_width && height >= quality.min_height,
        _ => true,
    }
}

fn previous_cached_image(
    data_dir: &Path,
    quality: &ImageQuality,
    excluded: &[PathBuf],
) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    for child in ["cache", "generated"] {
        let dir = data_dir.join(child);
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if !path.is_file()
                || !is_supported_image(&path)
                || excluded.contains(&path)
                || !image_meets_quality(&path, quality)
            {
                continue;
            }
            let modified = entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .unwrap_or(UNIX_EPOCH);
            candidates.push((modified, path));
        }
    }

    candidates
        .into_iter()
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, path)| path)
}

fn generate_fallback_cat_image(
    output_dir: &Path,
    quality: &ImageQuality,
    breeds: &[String],
) -> Result<PathBuf, AppStateError> {
    fs::create_dir_all(output_dir)?;
    let width = quality.preferred_width.max(quality.min_width).max(1920);
    let height = quality.preferred_height.max(quality.min_height).max(1080);
    let mut image = image::RgbaImage::from_pixel(width, height, image::Rgba([246, 234, 218, 255]));
    for y in 0..height {
        let shade = ((y as f32 / height as f32) * 32.0) as u8;
        for x in 0..width {
            let warmth = ((x as f32 / width as f32) * 18.0) as u8;
            image.put_pixel(
                x,
                y,
                image::Rgba([
                    245u8.saturating_sub(shade / 2),
                    232u8.saturating_sub(shade),
                    214u8.saturating_sub(warmth),
                    255,
                ]),
            );
        }
    }

    let fur = breed_fallback_color(breeds);
    let center_x = (width * 3) as i32 / 5;
    let center_y = height as i32 / 2;
    let radius_x = width as i32 / 6;
    let radius_y = height as i32 / 4;
    fill_triangle(
        &mut image,
        (center_x - radius_x, center_y - radius_y / 2),
        (center_x - radius_x / 2, center_y - radius_y - height as i32 / 12),
        (center_x - radius_x / 8, center_y - radius_y / 4),
        fur,
    );
    fill_triangle(
        &mut image,
        (center_x + radius_x, center_y - radius_y / 2),
        (center_x + radius_x / 2, center_y - radius_y - height as i32 / 12),
        (center_x + radius_x / 8, center_y - radius_y / 4),
        fur,
    );
    fill_ellipse(&mut image, center_x, center_y, radius_x, radius_y, fur);
    fill_ellipse(
        &mut image,
        center_x - radius_x / 3,
        center_y - radius_y / 8,
        radius_x / 7,
        radius_y / 9,
        image::Rgba([36, 50, 44, 255]),
    );
    fill_ellipse(
        &mut image,
        center_x + radius_x / 3,
        center_y - radius_y / 8,
        radius_x / 7,
        radius_y / 9,
        image::Rgba([36, 50, 44, 255]),
    );
    fill_ellipse(
        &mut image,
        center_x,
        center_y + radius_y / 8,
        radius_x / 10,
        radius_y / 12,
        image::Rgba([88, 48, 52, 255]),
    );
    for stripe in [-2, -1, 0, 1, 2] {
        draw_line(
            &mut image,
            center_x + stripe * radius_x / 8,
            center_y - radius_y / 2,
            center_x + stripe * radius_x / 12,
            center_y - radius_y / 5,
            image::Rgba([160, 84, 42, 255]),
        );
    }

    let output_path = output_dir.join(format!("generated-cat-{}.png", unique_suffix()));
    image::DynamicImage::ImageRgba8(image).save(&output_path)?;
    Ok(output_path)
}

fn breed_fallback_color(breeds: &[String]) -> image::Rgba<u8> {
    let breed = breeds
        .iter()
        .map(|breed| breed.trim().to_ascii_lowercase())
        .find(|breed| breed != "mixed")
        .unwrap_or_default();
    if breed.contains("orange") || breed.contains("tabby") {
        image::Rgba([226, 132, 55, 255])
    } else if breed.contains("black") {
        image::Rgba([42, 38, 36, 255])
    } else if breed.contains("white") {
        image::Rgba([238, 233, 220, 255])
    } else if breed.contains("calico") {
        image::Rgba([218, 170, 112, 255])
    } else {
        image::Rgba([174, 130, 92, 255])
    }
}

fn fill_ellipse(
    image: &mut image::RgbaImage,
    center_x: i32,
    center_y: i32,
    radius_x: i32,
    radius_y: i32,
    color: image::Rgba<u8>,
) {
    for y in (center_y - radius_y).max(0)..=(center_y + radius_y).min(image.height() as i32 - 1) {
        for x in (center_x - radius_x).max(0)..=(center_x + radius_x).min(image.width() as i32 - 1)
        {
            let dx = (x - center_x) as f32 / radius_x.max(1) as f32;
            let dy = (y - center_y) as f32 / radius_y.max(1) as f32;
            if dx * dx + dy * dy <= 1.0 {
                image.put_pixel(x as u32, y as u32, color);
            }
        }
    }
}

fn fill_triangle(
    image: &mut image::RgbaImage,
    a: (i32, i32),
    b: (i32, i32),
    c: (i32, i32),
    color: image::Rgba<u8>,
) {
    let min_x = a.0.min(b.0).min(c.0).max(0);
    let max_x = a.0.max(b.0).max(c.0).min(image.width() as i32 - 1);
    let min_y = a.1.min(b.1).min(c.1).max(0);
    let max_y = a.1.max(b.1).max(c.1).min(image.height() as i32 - 1);
    let area = edge(a, b, c);
    if area == 0 {
        return;
    }
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let p = (x, y);
            let w0 = edge(b, c, p);
            let w1 = edge(c, a, p);
            let w2 = edge(a, b, p);
            if (w0 >= 0 && w1 >= 0 && w2 >= 0) || (w0 <= 0 && w1 <= 0 && w2 <= 0) {
                image.put_pixel(x as u32, y as u32, color);
            }
        }
    }
}

fn edge(a: (i32, i32), b: (i32, i32), c: (i32, i32)) -> i32 {
    (c.0 - a.0) * (b.1 - a.1) - (c.1 - a.1) * (b.0 - a.0)
}

fn draw_line(
    image: &mut image::RgbaImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    color: image::Rgba<u8>,
) {
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut error = dx + dy;
    let mut x = x0;
    let mut y = y0;
    loop {
        if x >= 0 && y >= 0 && x < image.width() as i32 && y < image.height() as i32 {
            image.put_pixel(x as u32, y as u32, color);
        }
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * error;
        if e2 >= dy {
            error += dy;
            x += sx;
        }
        if e2 <= dx {
            error += dx;
            y += sy;
        }
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
        let source = image::open(image_path)?.to_rgba8();
        let safe_rect = safe_rect_for_display(display, min_x, min_y);
        paste_contained(&mut canvas, &source, safe_rect)?;
    }

    fs::create_dir_all(output_dir)?;
    let output_path = output_dir.join(format!("wallpaper-{}.png", unique_suffix()));
    image::DynamicImage::ImageRgba8(canvas).save(&output_path)?;
    Ok(output_path)
}

fn safe_rect_for_display(display: &DisplayGeometry, min_x: i32, min_y: i32) -> DisplayGeometry {
    let left = display.width / 5;
    let right = display.width / 25;
    let top = display.height / 25;
    let bottom = (display.height * 3) / 25;
    DisplayGeometry {
        x: display.x.saturating_sub(min_x) + left as i32,
        y: display.y.saturating_sub(min_y) + top as i32,
        width: display.width.saturating_sub(left).saturating_sub(right).max(1),
        height: display.height.saturating_sub(top).saturating_sub(bottom).max(1),
    }
}

fn paste_contained(
    canvas: &mut image::RgbaImage,
    source: &image::RgbaImage,
    rect: DisplayGeometry,
) -> Result<(), AppStateError> {
    let scale_x = rect.width as f32 / source.width().max(1) as f32;
    let scale_y = rect.height as f32 / source.height().max(1) as f32;
    let scale = scale_x.min(scale_y).max(0.01);
    let width = ((source.width() as f32 * scale).round() as u32)
        .max(1)
        .min(rect.width);
    let height = ((source.height() as f32 * scale).round() as u32)
        .max(1)
        .min(rect.height);
    let tile = image::imageops::resize(source, width, height, image::imageops::FilterType::Lanczos3);
    let x = rect.x.max(0) as u32 + (rect.width - width) / 2;
    let y = rect.y.max(0) as u32 + (rect.height - height) / 2;
    canvas.copy_from(&tile, x, y)?;
    Ok(())
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

    #[test]
    fn compose_display_wallpaper_keeps_icon_area_clear() {
        let dir = tempdir().unwrap();
        let cat_path = dir.path().join("cat.png");
        RgbImage::from_pixel(200, 100, Rgb([250, 120, 90]))
            .save(&cat_path)
            .unwrap();
        let displays = vec![DisplayGeometry {
            x: 0,
            y: 0,
            width: 200,
            height: 100,
        }];

        let output = compose_display_wallpaper(dir.path(), &displays, &[cat_path], &[0]).unwrap();
        let composed = image::open(output).unwrap().to_rgb8();

        assert_ne!(composed.get_pixel(5, 20), &Rgb([250, 120, 90]));
    }

    #[test]
    fn compose_display_wallpaper_preserves_full_portrait_image() {
        let dir = tempdir().unwrap();
        let cat_path = dir.path().join("portrait-cat.png");
        let mut image = RgbImage::from_pixel(40, 80, Rgb([40, 40, 40]));
        for y in 0..12 {
            for x in 0..40 {
                image.put_pixel(x, y, Rgb([255, 0, 0]));
            }
        }
        for y in 68..80 {
            for x in 0..40 {
                image.put_pixel(x, y, Rgb([0, 0, 255]));
            }
        }
        image.save(&cat_path).unwrap();
        let displays = vec![DisplayGeometry {
            x: 0,
            y: 0,
            width: 200,
            height: 100,
        }];

        let output = compose_display_wallpaper(dir.path(), &displays, &[cat_path], &[0]).unwrap();
        let composed = image::open(output).unwrap().to_rgb8();

        assert!(contains_rgb(&composed, Rgb([255, 0, 0])));
        assert!(contains_rgb(&composed, Rgb([0, 0, 255])));
    }

    #[test]
    fn generated_fallback_creates_hd_cat_image() {
        let dir = tempdir().unwrap();

        let path = generate_fallback_cat_image(
            dir.path(),
            &ImageQuality::default(),
            &["orange tabby".to_string()],
        )
        .unwrap();
        let image = image::open(path).unwrap().to_rgb8();

        assert_eq!(image.dimensions(), (3840, 2160));
        assert!(contains_near_rgb(&image, Rgb([226, 132, 55]), 12));
    }

    #[test]
    fn previous_cached_image_returns_newest_hd_candidate() {
        let dir = tempdir().unwrap();
        let cache = dir.path().join("cache");
        fs::create_dir_all(&cache).unwrap();
        RgbImage::from_pixel(1280, 720, Rgb([90, 90, 90]))
            .save(cache.join("small.jpg"))
            .unwrap();
        let large = cache.join("large.jpg");
        RgbImage::from_pixel(2560, 1440, Rgb([90, 150, 250]))
            .save(&large)
            .unwrap();

        assert_eq!(
            previous_cached_image(dir.path(), &ImageQuality::default(), &[]).unwrap(),
            large
        );
    }

    fn contains_rgb(image: &RgbImage, expected: Rgb<u8>) -> bool {
        image.pixels().any(|pixel| pixel == &expected)
    }

    fn contains_near_rgb(image: &RgbImage, expected: Rgb<u8>, tolerance: u8) -> bool {
        image.pixels().any(|pixel| {
            pixel
                .0
                .iter()
                .zip(expected.0.iter())
                .all(|(actual, expected)| actual.abs_diff(*expected) <= tolerance)
        })
    }
}
