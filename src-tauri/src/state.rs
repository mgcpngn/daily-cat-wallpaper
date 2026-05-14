use base64::Engine;
use daily_cat_core::config::CatImageType;
use daily_cat_core::sources::{
    magnific_search_query, pexels_search_query, pixabay_search_query, the_cat_api_breed_ids,
    transparent_cat_prompt, wikimedia_search_query,
};
use daily_cat_core::{
    AiImageProvider, AppConfig, CatCandidate, ImageQuality, SourceError, SourcePlanner,
};
use directories::ProjectDirs;
use image::{GenericImage, GenericImageView};
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
    #[error(transparent)]
    Base64(#[from] base64::DecodeError),
    #[error("gallery image is outside the managed cat gallery")]
    InvalidGalleryPath,
    #[error("AI image generation is not configured with an API key")]
    MissingAiApiKey,
    #[error("AI image generation response did not contain image data")]
    MissingAiImageData,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GalleryImage {
    pub path: PathBuf,
    pub file_name: String,
    pub source: String,
    pub width: u32,
    pub height: u32,
    pub meets_quality: bool,
    pub transparent: bool,
    pub feedback_score: i32,
    pub rejected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WallpaperResult {
    pub path: PathBuf,
    pub analysis: WallpaperAnalysis,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WallpaperAnalysis {
    pub wallpaper_path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub virtual_width: u32,
    pub virtual_height: u32,
    pub fills_virtual_desktop: bool,
    pub meets_resolution: bool,
    pub uses_placeholder_art: bool,
    pub transparent_pet_asset: bool,
    pub likely_cropped: bool,
    pub unverified_preference_match: bool,
    pub issues: Vec<WallpaperIssue>,
    pub score: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WallpaperIssue {
    NotVirtualDesktopSized,
    BelowMinimumResolution,
    PlaceholderGeneratedArt,
    NonTransparentPetAsset,
    SubjectTouchesImageEdge,
    PreferenceMatchUnverified,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FeedbackInput {
    pub path: PathBuf,
    pub liked: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LearningSummary {
    pub liked: u32,
    pub disliked: u32,
    pub rejected_images: u32,
    pub top_reasons: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
struct FeedbackStore {
    records: HashMap<String, FeedbackRecord>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
struct FeedbackRecord {
    liked: u32,
    disliked: u32,
    reasons: Vec<String>,
    last_feedback_nanos: u128,
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

    pub fn gallery_dir(&self) -> PathBuf {
        self.data_dir.join("cat-gallery")
    }

    pub fn gallery_download_dir(&self) -> PathBuf {
        self.gallery_dir().join("downloads")
    }

    pub fn gallery_generated_dir(&self) -> PathBuf {
        self.gallery_dir().join("generated")
    }

    pub fn list_gallery_images(
        &self,
        quality: &ImageQuality,
    ) -> Result<Vec<GalleryImage>, AppStateError> {
        let mut images = Vec::new();
        let feedback = self.load_feedback()?;
        for (child, source) in [("downloads", "downloaded"), ("generated", "generated")] {
            let dir = self.gallery_dir().join(child);
            let Ok(entries) = fs::read_dir(dir) else {
                continue;
            };
            for entry in entries.filter_map(Result::ok) {
                let path = entry.path();
                if !path.is_file() || !is_supported_image(&path) {
                    continue;
                }
                let Ok((width, height)) = image::image_dimensions(&path) else {
                    continue;
                };
                let feedback_score = feedback.score_for_path(&path);
                images.push(GalleryImage {
                    file_name: path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("cat-image")
                        .to_string(),
                    transparent: image_has_transparency(&path),
                    meets_quality: width >= quality.min_width && height >= quality.min_height,
                    source: source.to_string(),
                    width,
                    height,
                    feedback_score,
                    rejected: feedback_score <= -2,
                    path,
                });
            }
        }
        images.sort_by(|left, right| {
            right
                .feedback_score
                .cmp(&left.feedback_score)
                .then_with(|| right.path.cmp(&left.path))
        });
        Ok(images)
    }

    pub fn delete_gallery_image(&self, path: &Path) -> Result<(), AppStateError> {
        self.validate_gallery_path(path)?;
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    pub fn validate_gallery_path(&self, path: &Path) -> Result<(), AppStateError> {
        let gallery = fs::canonicalize(self.gallery_dir()).or_else(|_| {
            fs::create_dir_all(self.gallery_dir())?;
            fs::canonicalize(self.gallery_dir())
        })?;
        let canonical = fs::canonicalize(path)?;
        if canonical.starts_with(gallery) {
            Ok(())
        } else {
            Err(AppStateError::InvalidGalleryPath)
        }
    }

    pub fn gallery_image_is_usable(
        &self,
        path: &Path,
        quality: &ImageQuality,
    ) -> Result<bool, AppStateError> {
        self.validate_gallery_path(path)?;
        let feedback = self.load_feedback()?;
        Ok(path.is_file()
            && is_supported_image(path)
            && image_meets_quality(path, quality)
            && feedback.score_for_path(path) > -2)
    }

    pub fn record_feedback(&self, input: FeedbackInput) -> Result<LearningSummary, AppStateError> {
        if input.path.exists() {
            let _ = self.validate_gallery_path(&input.path);
        }
        let mut feedback = self.load_feedback()?;
        feedback.record(&input.path, input.liked, input.reason.as_deref());
        self.save_feedback(&feedback)?;
        Ok(feedback.summary())
    }

    pub fn learning_summary(&self) -> Result<LearningSummary, AppStateError> {
        Ok(self.load_feedback()?.summary())
    }

    pub fn analyze_wallpaper(
        &self,
        wallpaper_path: &Path,
        displays: &[DisplayGeometry],
        source_paths: &[PathBuf],
        quality: &ImageQuality,
    ) -> Result<WallpaperAnalysis, AppStateError> {
        analyze_wallpaper_effect(wallpaper_path, displays, source_paths, quality)
    }

    pub async fn resolve_wallpaper_image(
        &self,
        planner: &SourcePlanner,
        quality: &ImageQuality,
    ) -> Result<PathBuf, AppStateError> {
        let feedback = self.load_feedback()?;
        let sources = planner.ordered_sources()?;
        for source in sources {
            match self
                .resolve_source(&source, quality, &[], planner, &feedback)
                .await
            {
                Ok(Some(path)) => return Ok(path),
                Ok(None) => {}
                Err(error) if error.is_recoverable_source_error() => {}
                Err(error) => return Err(error),
            }
        }

        if let Some(path) = previous_cached_image(&self.data_dir, quality, &[], &feedback) {
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
        let output_dir = self.data_dir.join("wallpapers");
        fs::create_dir_all(&output_dir)?;
        compose_display_wallpaper(&output_dir, displays, image_paths, assignments)
    }

    pub async fn generate_ai_cat_images(
        &self,
        config: &AppConfig,
        count: usize,
    ) -> Result<Vec<PathBuf>, AppStateError> {
        let count = count.clamp(1, config.ai_generation.count.max(1) as usize);
        let prompt = transparent_cat_prompt(config);
        let bytes = match config.ai_generation.provider {
            AiImageProvider::OpenAi => self.generate_openai_images(config, &prompt, count).await?,
            AiImageProvider::GoogleNanoBananaPro => {
                self.generate_google_images(config, &prompt, count).await?
            }
        };

        let output_dir = self.gallery_generated_dir();
        fs::create_dir_all(&output_dir)?;
        let mut paths = Vec::new();
        for (index, image_bytes) in bytes.iter().enumerate() {
            let output_path =
                output_dir.join(format!("ai-cat-{}-{}.png", unique_suffix(), index + 1));
            write_generated_png(
                &output_path,
                image_bytes,
                &config.image_quality,
                config.ai_generation.transparent_cutout,
            )?;
            paths.push(output_path);
        }

        if paths.is_empty() {
            Err(AppStateError::MissingAiImageData)
        } else {
            Ok(paths)
        }
    }

    fn config_path(&self) -> PathBuf {
        self.data_dir.join("config.json")
    }

    fn feedback_path(&self) -> PathBuf {
        self.data_dir.join("learning-feedback.json")
    }

    fn load_feedback(&self) -> Result<FeedbackStore, AppStateError> {
        let path = self.feedback_path();
        if !path.exists() {
            return Ok(FeedbackStore::default());
        }
        Ok(serde_json::from_slice(&fs::read(path)?)?)
    }

    fn save_feedback(&self, feedback: &FeedbackStore) -> Result<(), AppStateError> {
        fs::create_dir_all(&self.data_dir)?;
        fs::write(self.feedback_path(), serde_json::to_vec_pretty(feedback)?)?;
        Ok(())
    }

    async fn generate_openai_images(
        &self,
        config: &AppConfig,
        prompt: &str,
        count: usize,
    ) -> Result<Vec<Vec<u8>>, AppStateError> {
        let api_key = config
            .ai_generation
            .openai_api_key
            .as_deref()
            .map(str::trim)
            .filter(|key| !key.is_empty())
            .ok_or(AppStateError::MissingAiApiKey)?;
        let model = if config.ai_generation.transparent_cutout
            && config.ai_generation.openai_model.trim() == "gpt-image-2"
        {
            "gpt-image-1.5"
        } else {
            config.ai_generation.openai_model.trim()
        };

        #[derive(Debug, Deserialize)]
        struct OpenAiImageResponse {
            data: Vec<OpenAiImageData>,
        }

        #[derive(Debug, Deserialize)]
        struct OpenAiImageData {
            b64_json: Option<String>,
        }

        let response = self
            .client
            .post("https://api.openai.com/v1/images/generations")
            .bearer_auth(api_key)
            .json(&serde_json::json!({
                "model": model,
                "prompt": prompt,
                "n": count,
                "size": "1024x1536",
                "quality": "high",
                "background": if config.ai_generation.transparent_cutout { "transparent" } else { "opaque" },
                "output_format": "png"
            }))
            .send()
            .await?
            .error_for_status()?
            .json::<OpenAiImageResponse>()
            .await?;

        decode_image_payloads(
            response
                .data
                .into_iter()
                .filter_map(|image| image.b64_json)
                .collect(),
        )
    }

    async fn generate_google_images(
        &self,
        config: &AppConfig,
        prompt: &str,
        count: usize,
    ) -> Result<Vec<Vec<u8>>, AppStateError> {
        let api_key = config
            .ai_generation
            .google_api_key
            .as_deref()
            .map(str::trim)
            .filter(|key| !key.is_empty())
            .ok_or(AppStateError::MissingAiApiKey)?;
        let model = config.ai_generation.google_model.trim();

        #[derive(Debug, Deserialize)]
        struct GoogleResponse {
            candidates: Option<Vec<GoogleCandidate>>,
        }

        #[derive(Debug, Deserialize)]
        struct GoogleCandidate {
            content: GoogleContent,
        }

        #[derive(Debug, Deserialize)]
        struct GoogleContent {
            parts: Vec<GooglePart>,
        }

        #[derive(Debug, Deserialize)]
        struct GooglePart {
            #[serde(alias = "inlineData", alias = "inline_data")]
            inline_data: Option<GoogleInlineData>,
        }

        #[derive(Debug, Deserialize)]
        struct GoogleInlineData {
            data: String,
        }

        let request_prompt = if count <= 1 {
            prompt.to_string()
        } else {
            format!("{prompt} Generate {count} distinct cat cutout variants in this response.")
        };

        let response = self
            .client
            .post(format!(
                "https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent"
            ))
            .header("x-goog-api-key", api_key)
            .json(&serde_json::json!({
                "contents": [{"parts": [{"text": request_prompt}]}],
                "generationConfig": {
                    "responseModalities": ["IMAGE"],
                    "thinkingConfig": {
                        "thinkingLevel": "High",
                        "includeThoughts": false
                    }
                }
            }))
            .send()
            .await?
            .error_for_status()?
            .json::<GoogleResponse>()
            .await?;

        let payloads = response
            .candidates
            .unwrap_or_default()
            .into_iter()
            .flat_map(|candidate| candidate.content.parts)
            .filter_map(|part| part.inline_data.map(|data| data.data))
            .take(count)
            .collect();
        decode_image_payloads(payloads)
    }

    async fn resolve_wallpaper_image_excluding(
        &self,
        planner: &SourcePlanner,
        quality: &ImageQuality,
        excluded: &[PathBuf],
    ) -> Result<PathBuf, AppStateError> {
        let feedback = self.load_feedback()?;
        let sources = planner.ordered_sources()?;
        for source in sources {
            match self
                .resolve_source(&source, quality, excluded, planner, &feedback)
                .await
            {
                Ok(Some(path)) => return Ok(path),
                Ok(None) => {}
                Err(error) if error.is_recoverable_source_error() => {}
                Err(error) => return Err(error),
            }
        }

        if let Some(path) = previous_cached_image(&self.data_dir, quality, excluded, &feedback) {
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
        feedback: &FeedbackStore,
    ) -> Result<Option<PathBuf>, AppStateError> {
        if let Some(dir) = source.strip_prefix("local:") {
            return Ok(first_local_image(
                Path::new(dir),
                quality,
                excluded,
                feedback,
            ));
        }

        let candidates = match source {
            "magnific" => {
                self.magnific_candidates(
                    planner.magnific_api_key.as_deref().unwrap_or_default(),
                    &planner.breeds,
                    &planner.image_types,
                )
                .await?
            }
            "pixabay" => {
                self.pixabay_candidates(
                    planner.pixabay_api_key.as_deref().unwrap_or_default(),
                    &planner.breeds,
                    &planner.image_types,
                )
                .await?
            }
            "pexels" => {
                self.pexels_candidates(
                    planner.pexels_api_key.as_deref().unwrap_or_default(),
                    &planner.breeds,
                    &planner.image_types,
                    quality,
                )
                .await?
            }
            "wikimedia" => {
                self.wikimedia_candidates(&planner.breeds, &planner.image_types)
                    .await?
            }
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
                let output_dir = self.gallery_generated_dir();
                let path = generate_fallback_cat_image(&output_dir, quality, &planner.breeds)?;
                return Ok(Some(path));
            }
            _ => None,
        };

        let Some(candidates) = candidates else {
            return Ok(None);
        };

        let cache_dir = self.gallery_download_dir();
        fs::create_dir_all(&cache_dir)?;
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
            if output_path.exists()
                && image_meets_quality(&output_path, quality)
                && !excluded.contains(&output_path)
                && feedback.score_for_path(&output_path) > -2
            {
                return Ok(Some(output_path));
            }
            let Ok(response) = self.client.get(&candidate.uri).send().await else {
                continue;
            };
            let Ok(bytes) = response.bytes().await else {
                continue;
            };
            if write_verified_image(&output_path, &bytes, quality)?
                && !excluded.contains(&output_path)
                && feedback.score_for_path(&output_path) > -2
            {
                return Ok(Some(output_path));
            }
        }

        Ok(None)
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
            .query(&[
                ("query", query.as_str()),
                ("type", "image"),
                ("limit", "12"),
            ])
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
    ) -> Result<Option<Vec<CatCandidate>>, AppStateError> {
        if api_key.trim().is_empty() {
            return Ok(None);
        }

        #[derive(Debug, Deserialize)]
        struct PixabayResponse {
            hits: Vec<PixabayImage>,
        }

        #[derive(Debug, Deserialize)]
        struct PixabayImage {
            id: u64,
            #[serde(rename = "largeImageURL")]
            large_image_url: Option<String>,
            #[serde(rename = "fullHDURL")]
            full_hd_url: Option<String>,
            #[serde(rename = "imageURL")]
            image_url: Option<String>,
            #[serde(rename = "imageWidth")]
            image_width: u32,
            #[serde(rename = "imageHeight")]
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
                let (uri, width, height) = if let Some(url) = image.full_hd_url {
                    (url, image.image_width, image.image_height)
                } else if let Some(url) = image.large_image_url {
                    (url, image.image_width, image.image_height)
                } else if let Some(url) = image.image_url {
                    (url, image.image_width, image.image_height)
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
            .filter(|(_, info)| {
                matches!(
                    info.mime.as_str(),
                    "image/jpeg" | "image/png" | "image/webp"
                )
            })
            .map(|(title, info)| CatCandidate {
                id: format!("commons-{title}"),
                uri: info.url,
                source: "wikimedia".to_string(),
                width: Some(info.width),
                height: Some(info.height),
            })
            .collect::<Vec<_>>();
        candidates.sort_by_key(|candidate| {
            std::cmp::Reverse(
                candidate.width.unwrap_or(0) as u64 * candidate.height.unwrap_or(0) as u64,
            )
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
            .query(&[("mime_types", "jpg,png"), ("size", "full"), ("limit", "20")]);
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

fn decode_image_payloads(payloads: Vec<String>) -> Result<Vec<Vec<u8>>, AppStateError> {
    let mut images = Vec::new();
    for payload in payloads {
        images.push(base64::engine::general_purpose::STANDARD.decode(payload)?);
    }
    if images.is_empty() {
        Err(AppStateError::MissingAiImageData)
    } else {
        Ok(images)
    }
}

fn write_verified_image(
    output_path: &Path,
    bytes: &[u8],
    quality: &ImageQuality,
) -> Result<bool, AppStateError> {
    let image = image::load_from_memory(bytes)?;
    let (width, height) = image.dimensions();
    if width < quality.min_width || height < quality.min_height {
        return Ok(false);
    }
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(output_path, bytes)?;
    Ok(true)
}

fn write_generated_png(
    output_path: &Path,
    bytes: &[u8],
    quality: &ImageQuality,
    transparent: bool,
) -> Result<(), AppStateError> {
    let mut image = image::load_from_memory(bytes)?.to_rgba8();
    if transparent && !rgba_has_transparency(&image) {
        remove_flat_background_to_alpha(&mut image);
    }
    let scale = (quality.min_width as f32 / image.width().max(1) as f32)
        .max(quality.min_height as f32 / image.height().max(1) as f32)
        .max(1.0);
    if scale > 1.0 {
        let width = ((image.width() as f32 * scale).ceil() as u32).max(quality.min_width);
        let height = ((image.height() as f32 * scale).ceil() as u32).max(quality.min_height);
        image =
            image::imageops::resize(&image, width, height, image::imageops::FilterType::Lanczos3);
    }
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    image::DynamicImage::ImageRgba8(image)
        .save_with_format(output_path, image::ImageFormat::Png)?;
    Ok(())
}

fn image_has_transparency(path: &Path) -> bool {
    image::open(path)
        .map(|image| rgba_has_transparency(&image.to_rgba8()))
        .unwrap_or(false)
}

fn rgba_has_transparency(image: &image::RgbaImage) -> bool {
    image.pixels().any(|pixel| pixel.0[3] < 250)
}

fn remove_flat_background_to_alpha(image: &mut image::RgbaImage) {
    let width = image.width();
    let height = image.height();
    if width == 0 || height == 0 {
        return;
    }
    let corners = [
        image.get_pixel(0, 0).0,
        image.get_pixel(width - 1, 0).0,
        image.get_pixel(0, height - 1).0,
        image.get_pixel(width - 1, height - 1).0,
    ];
    let background = [
        ((corners[0][0] as u16
            + corners[1][0] as u16
            + corners[2][0] as u16
            + corners[3][0] as u16)
            / 4) as u8,
        ((corners[0][1] as u16
            + corners[1][1] as u16
            + corners[2][1] as u16
            + corners[3][1] as u16)
            / 4) as u8,
        ((corners[0][2] as u16
            + corners[1][2] as u16
            + corners[2][2] as u16
            + corners[3][2] as u16)
            / 4) as u8,
    ];
    for pixel in image.pixels_mut() {
        let distance = pixel.0[0].abs_diff(background[0]) as u16
            + pixel.0[1].abs_diff(background[1]) as u16
            + pixel.0[2].abs_diff(background[2]) as u16;
        if distance < 42 {
            pixel.0[3] = 0;
        } else if distance < 88 {
            pixel.0[3] = pixel.0[3].min(((distance - 42) * 6).min(255) as u8);
        }
    }
}

fn first_local_image(
    dir: &Path,
    quality: &ImageQuality,
    excluded: &[PathBuf],
    feedback: &FeedbackStore,
) -> Option<PathBuf> {
    let entries = fs::read_dir(dir).ok()?;
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.is_file()
                && is_supported_image(path)
                && image_meets_quality(path, quality)
                && !excluded.contains(path)
                && feedback.score_for_path(path) > -2
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
    feedback: &FeedbackStore,
) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    for dir in [
        data_dir.join("cat-gallery").join("downloads"),
        data_dir.join("cat-gallery").join("generated"),
        data_dir.join("cache"),
        data_dir.join("generated"),
    ] {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if !path.is_file()
                || !is_supported_image(&path)
                || excluded.contains(&path)
                || !image_meets_quality(&path, quality)
                || feedback.score_for_path(&path) <= -2
            {
                continue;
            }
            let modified = entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .unwrap_or(UNIX_EPOCH);
            candidates.push((feedback.score_for_path(&path), modified, path));
        }
    }

    candidates
        .into_iter()
        .max_by_key(|(score, modified, _)| (*score, *modified))
        .map(|(_, _, path)| path)
}

impl FeedbackStore {
    fn record(&mut self, path: &Path, liked: bool, reason: Option<&str>) {
        let key = feedback_key(path);
        let record = self.records.entry(key).or_default();
        if liked {
            record.liked = record.liked.saturating_add(1);
        } else {
            record.disliked = record.disliked.saturating_add(1);
        }
        if let Some(reason) = reason.map(str::trim).filter(|reason| !reason.is_empty()) {
            record.reasons.push(reason.to_string());
        }
        record.last_feedback_nanos = unique_suffix();
    }

    fn score_for_path(&self, path: &Path) -> i32 {
        self.records
            .get(&feedback_key(path))
            .map(|record| record.liked as i32 - (record.disliked as i32 * 2))
            .unwrap_or(0)
    }

    fn summary(&self) -> LearningSummary {
        let mut liked = 0u32;
        let mut disliked = 0u32;
        let mut rejected_images = 0u32;
        let mut reason_counts: HashMap<String, u32> = HashMap::new();
        for record in self.records.values() {
            liked = liked.saturating_add(record.liked);
            disliked = disliked.saturating_add(record.disliked);
            if record.liked as i32 - (record.disliked as i32 * 2) <= -2 {
                rejected_images = rejected_images.saturating_add(1);
            }
            for reason in &record.reasons {
                *reason_counts.entry(reason.clone()).or_default() += 1;
            }
        }
        let mut reasons = reason_counts.into_iter().collect::<Vec<_>>();
        reasons.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
        LearningSummary {
            liked,
            disliked,
            rejected_images,
            top_reasons: reasons
                .into_iter()
                .take(5)
                .map(|(reason, _)| reason)
                .collect(),
        }
    }
}

fn feedback_key(path: &Path) -> String {
    fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase()
}

fn generate_fallback_cat_image(
    output_dir: &Path,
    quality: &ImageQuality,
    breeds: &[String],
) -> Result<PathBuf, AppStateError> {
    fs::create_dir_all(output_dir)?;
    let width = quality.preferred_width.max(quality.min_width).max(2560);
    let height = quality.preferred_height.max(quality.min_height).max(1440);
    let mut image = image::RgbaImage::from_pixel(width, height, image::Rgba([0, 0, 0, 0]));

    let fur = breed_fallback_color(breeds);
    let center_x = (width * 3) as i32 / 5;
    let center_y = height as i32 / 2;
    let radius_x = width as i32 / 6;
    let radius_y = height as i32 / 4;
    fill_triangle(
        &mut image,
        (center_x - radius_x, center_y - radius_y / 2),
        (
            center_x - radius_x / 2,
            center_y - radius_y - height as i32 / 12,
        ),
        (center_x - radius_x / 8, center_y - radius_y / 4),
        fur,
    );
    fill_triangle(
        &mut image,
        (center_x + radius_x, center_y - radius_y / 2),
        (
            center_x + radius_x / 2,
            center_y - radius_y - height as i32 / 12,
        ),
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
    let mut canvas = desktop_background(width, height);

    for (display_index, display) in displays.iter().enumerate() {
        let assignment = assignments.get(display_index).copied().unwrap_or(0);
        let image_path = image_paths
            .get(assignment)
            .or_else(|| image_paths.first())
            .ok_or(AppStateError::NoCandidate)?;
        let source = image::open(image_path)?.to_rgba8();
        let safe_rect = safe_rect_for_display(display, min_x, min_y);
        if !rgba_has_transparency(&source) {
            fill_display_with_cover(&mut canvas, &source, display, min_x, min_y)?;
        }
        paste_contained(&mut canvas, &source, safe_rect)?;
    }

    fs::create_dir_all(output_dir)?;
    let output_path = output_dir.join(format!("wallpaper-{}.png", unique_suffix()));
    image::DynamicImage::ImageRgba8(canvas).save(&output_path)?;
    Ok(output_path)
}

fn desktop_background(width: u32, height: u32) -> image::RgbaImage {
    let mut image = image::RgbaImage::from_pixel(width, height, image::Rgba([32, 39, 44, 255]));
    for y in 0..height {
        let vertical = (y as f32 / height.max(1) as f32 * 54.0) as u8;
        for x in 0..width {
            let horizontal = (x as f32 / width.max(1) as f32 * 28.0) as u8;
            image.put_pixel(
                x,
                y,
                image::Rgba([
                    36u8.saturating_add(horizontal / 3),
                    43u8.saturating_add(vertical / 4),
                    48u8.saturating_add(horizontal / 5),
                    255,
                ]),
            );
        }
    }
    image
}

fn fill_display_with_cover(
    canvas: &mut image::RgbaImage,
    source: &image::RgbaImage,
    display: &DisplayGeometry,
    min_x: i32,
    min_y: i32,
) -> Result<(), AppStateError> {
    let scale_x = display.width as f32 / source.width().max(1) as f32;
    let scale_y = display.height as f32 / source.height().max(1) as f32;
    let scale = scale_x.max(scale_y).max(0.01);
    let width = ((source.width() as f32 * scale).ceil() as u32).max(display.width);
    let height = ((source.height() as f32 * scale).ceil() as u32).max(display.height);
    let mut tile =
        image::imageops::resize(source, width, height, image::imageops::FilterType::Triangle);
    tile = image::imageops::blur(&tile, 18.0);
    for pixel in tile.pixels_mut() {
        pixel.0[0] = ((pixel.0[0] as u16 + 36) / 2) as u8;
        pixel.0[1] = ((pixel.0[1] as u16 + 43) / 2) as u8;
        pixel.0[2] = ((pixel.0[2] as u16 + 48) / 2) as u8;
    }
    let crop_x = (width - display.width) / 2;
    let crop_y = (height - display.height) / 2;
    let cropped =
        image::imageops::crop_imm(&tile, crop_x, crop_y, display.width, display.height).to_image();
    canvas.copy_from(
        &cropped,
        display.x.saturating_sub(min_x).max(0) as u32,
        display.y.saturating_sub(min_y).max(0) as u32,
    )?;
    Ok(())
}

fn safe_rect_for_display(display: &DisplayGeometry, min_x: i32, min_y: i32) -> DisplayGeometry {
    let left = display.width / 5;
    let right = display.width / 25;
    let top = display.height / 25;
    let bottom = (display.height * 3) / 25;
    DisplayGeometry {
        x: display.x.saturating_sub(min_x) + left as i32,
        y: display.y.saturating_sub(min_y) + top as i32,
        width: display
            .width
            .saturating_sub(left)
            .saturating_sub(right)
            .max(1),
        height: display
            .height
            .saturating_sub(top)
            .saturating_sub(bottom)
            .max(1),
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
    let tile =
        image::imageops::resize(source, width, height, image::imageops::FilterType::Lanczos3);
    let x = rect.x.max(0) as u32 + (rect.width - width) / 2;
    let y = rect.y.max(0) as u32 + (rect.height - height) / 2;
    canvas.copy_from(&tile, x, y)?;
    Ok(())
}

fn analyze_wallpaper_effect(
    wallpaper_path: &Path,
    displays: &[DisplayGeometry],
    source_paths: &[PathBuf],
    quality: &ImageQuality,
) -> Result<WallpaperAnalysis, AppStateError> {
    let (width, height) = image::image_dimensions(wallpaper_path)?;
    let (virtual_width, virtual_height) = virtual_desktop_size(displays).unwrap_or((width, height));
    let fills_virtual_desktop = width == virtual_width && height == virtual_height;
    let meets_resolution = width >= quality.min_width && height >= quality.min_height;
    let uses_placeholder_art = source_paths.iter().any(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.starts_with("generated-cat-"))
            .unwrap_or(false)
    });
    let transparent_pet_asset = source_paths.iter().any(|path| image_has_transparency(path));
    let likely_cropped = source_paths
        .iter()
        .any(|path| transparent_subject_touches_edges(path));
    let unverified_preference_match = source_paths.iter().any(|path| {
        path.to_string_lossy().contains("cat-gallery/downloads")
            || !path.to_string_lossy().contains("cat-gallery/generated")
    });

    let mut issues = Vec::new();
    if !fills_virtual_desktop {
        issues.push(WallpaperIssue::NotVirtualDesktopSized);
    }
    if !meets_resolution {
        issues.push(WallpaperIssue::BelowMinimumResolution);
    }
    if uses_placeholder_art {
        issues.push(WallpaperIssue::PlaceholderGeneratedArt);
    }
    if !transparent_pet_asset {
        issues.push(WallpaperIssue::NonTransparentPetAsset);
    }
    if likely_cropped {
        issues.push(WallpaperIssue::SubjectTouchesImageEdge);
    }
    if unverified_preference_match {
        issues.push(WallpaperIssue::PreferenceMatchUnverified);
    }

    let mut score = 100i32;
    for issue in &issues {
        score -= match issue {
            WallpaperIssue::NotVirtualDesktopSized => 30,
            WallpaperIssue::BelowMinimumResolution => 35,
            WallpaperIssue::PlaceholderGeneratedArt => 25,
            WallpaperIssue::NonTransparentPetAsset => 10,
            WallpaperIssue::SubjectTouchesImageEdge => 25,
            WallpaperIssue::PreferenceMatchUnverified => 8,
        };
    }

    Ok(WallpaperAnalysis {
        wallpaper_path: wallpaper_path.to_path_buf(),
        width,
        height,
        virtual_width,
        virtual_height,
        fills_virtual_desktop,
        meets_resolution,
        uses_placeholder_art,
        transparent_pet_asset,
        likely_cropped,
        unverified_preference_match,
        issues,
        score: score.clamp(0, 100) as u8,
    })
}

fn virtual_desktop_size(displays: &[DisplayGeometry]) -> Option<(u32, u32)> {
    if displays.is_empty() {
        return None;
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
    Some(((max_x - min_x).max(1) as u32, (max_y - min_y).max(1) as u32))
}

fn transparent_subject_touches_edges(path: &Path) -> bool {
    let Ok(image) = image::open(path).map(|image| image.to_rgba8()) else {
        return false;
    };
    let mut min_x = image.width();
    let mut min_y = image.height();
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    let mut found = false;
    for (x, y, pixel) in image.enumerate_pixels() {
        if pixel.0[3] > 20 {
            found = true;
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
    }
    found && (min_x == 0 || min_y == 0 || max_x + 1 >= image.width() || max_y + 1 >= image.height())
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
    use image::{ImageFormat, Rgb, RgbImage};
    use std::io::Cursor;
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

        let selected = first_local_image(
            dir.path(),
            &ImageQuality::default(),
            &[],
            &FeedbackStore::default(),
        )
        .unwrap();

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
            &FeedbackStore::default(),
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
            previous_cached_image(
                dir.path(),
                &ImageQuality::default(),
                &[],
                &FeedbackStore::default(),
            )
            .unwrap(),
            large
        );
    }

    #[test]
    fn downloaded_low_resolution_bytes_are_not_written_to_gallery() {
        let dir = tempdir().unwrap();
        let output = dir.path().join("downloads").join("small.jpg");
        let bytes = encoded_test_image(1280, 720);

        let saved = write_verified_image(&output, &bytes, &ImageQuality::default()).unwrap();

        assert!(!saved);
        assert!(!output.exists());
    }

    #[test]
    fn downloaded_2k_or_better_bytes_are_written_to_gallery() {
        let dir = tempdir().unwrap();
        let output = dir.path().join("downloads").join("large.jpg");
        let bytes = encoded_test_image(2560, 1440);

        let saved = write_verified_image(&output, &bytes, &ImageQuality::default()).unwrap();

        assert!(saved);
        assert!(output.exists());
        assert!(image_meets_quality(&output, &ImageQuality::default()));
    }

    #[test]
    fn gallery_location_is_fixed_under_app_data() {
        let dir = tempdir().unwrap();
        let state = test_state(dir.path());

        assert_eq!(state.gallery_dir(), dir.path().join("cat-gallery"));
        assert_eq!(
            state.gallery_download_dir(),
            dir.path().join("cat-gallery").join("downloads")
        );
    }

    #[test]
    fn list_gallery_images_marks_low_resolution_files_for_cleanup() {
        let dir = tempdir().unwrap();
        let state = test_state(dir.path());
        let downloads = state.gallery_download_dir();
        fs::create_dir_all(&downloads).unwrap();
        RgbImage::from_pixel(1280, 720, Rgb([90, 90, 90]))
            .save(downloads.join("small.jpg"))
            .unwrap();
        RgbImage::from_pixel(2560, 1440, Rgb([90, 150, 250]))
            .save(downloads.join("large.jpg"))
            .unwrap();

        let images = state.list_gallery_images(&ImageQuality::default()).unwrap();

        assert_eq!(images.len(), 2);
        assert!(images
            .iter()
            .any(|image| image.file_name == "large.jpg" && image.meets_quality));
        assert!(images
            .iter()
            .any(|image| image.file_name == "small.jpg" && !image.meets_quality));
    }

    #[test]
    fn delete_gallery_image_removes_managed_file() {
        let dir = tempdir().unwrap();
        let state = test_state(dir.path());
        let downloads = state.gallery_download_dir();
        fs::create_dir_all(&downloads).unwrap();
        let path = downloads.join("delete-me.jpg");
        RgbImage::from_pixel(2560, 1440, Rgb([90, 150, 250]))
            .save(&path)
            .unwrap();

        state.delete_gallery_image(&path).unwrap();

        assert!(!path.exists());
    }

    #[test]
    fn wallpaper_analysis_flags_output_that_does_not_fill_virtual_desktop() {
        let dir = tempdir().unwrap();
        let wallpaper = dir.path().join("too-small.png");
        RgbImage::from_pixel(1200, 800, Rgb([90, 150, 250]))
            .save(&wallpaper)
            .unwrap();
        let displays = vec![DisplayGeometry {
            x: 0,
            y: 0,
            width: 2560,
            height: 1440,
        }];

        let analysis =
            analyze_wallpaper_effect(&wallpaper, &displays, &[], &ImageQuality::default()).unwrap();

        assert!(!analysis.fills_virtual_desktop);
        assert!(analysis
            .issues
            .contains(&WallpaperIssue::NotVirtualDesktopSized));
        assert!(analysis
            .issues
            .contains(&WallpaperIssue::BelowMinimumResolution));
    }

    #[test]
    fn wallpaper_analysis_passes_full_size_transparent_pet_asset() {
        let dir = tempdir().unwrap();
        let wallpaper = dir.path().join("wallpaper.png");
        RgbImage::from_pixel(2560, 1440, Rgb([40, 44, 50]))
            .save(&wallpaper)
            .unwrap();
        let source = dir.path().join("ai-cat.png");
        transparent_test_cat(2560, 1440, 240, 120)
            .save(&source)
            .unwrap();
        let displays = vec![DisplayGeometry {
            x: 0,
            y: 0,
            width: 2560,
            height: 1440,
        }];

        let analysis = analyze_wallpaper_effect(
            &wallpaper,
            &displays,
            std::slice::from_ref(&source),
            &ImageQuality::default(),
        )
        .unwrap();

        assert!(analysis.fills_virtual_desktop);
        assert!(analysis.meets_resolution);
        assert!(analysis.transparent_pet_asset);
        assert!(!analysis.likely_cropped);
        assert!(!analysis
            .issues
            .contains(&WallpaperIssue::NonTransparentPetAsset));
    }

    #[test]
    fn user_feedback_rejects_image_from_future_selection() {
        let dir = tempdir().unwrap();
        let state = test_state(dir.path());
        let downloads = state.gallery_download_dir();
        fs::create_dir_all(&downloads).unwrap();
        let disliked = downloads.join("bad-cat.jpg");
        let liked = downloads.join("good-cat.jpg");
        RgbImage::from_pixel(2560, 1440, Rgb([90, 90, 90]))
            .save(&disliked)
            .unwrap();
        RgbImage::from_pixel(2560, 1440, Rgb([90, 150, 250]))
            .save(&liked)
            .unwrap();
        state
            .record_feedback(FeedbackInput {
                path: disliked.clone(),
                liked: false,
                reason: Some("wrong breed".to_string()),
            })
            .unwrap();

        let selected = first_local_image(
            &downloads,
            &ImageQuality::default(),
            &[],
            &state.load_feedback().unwrap(),
        )
        .unwrap();

        assert_eq!(selected, liked);
        assert_eq!(state.learning_summary().unwrap().rejected_images, 1);
    }

    fn test_state(data_dir: &Path) -> AppState {
        AppState {
            data_dir: data_dir.to_path_buf(),
            client: reqwest::Client::new(),
        }
    }

    fn transparent_test_cat(
        width: u32,
        height: u32,
        inset_x: u32,
        inset_y: u32,
    ) -> image::RgbaImage {
        let mut image = image::RgbaImage::from_pixel(width, height, image::Rgba([0, 0, 0, 0]));
        for y in inset_y..height - inset_y {
            for x in inset_x..width - inset_x {
                image.put_pixel(x, y, image::Rgba([230, 140, 80, 255]));
            }
        }
        image
    }

    fn encoded_test_image(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = Cursor::new(Vec::new());
        RgbImage::from_pixel(width, height, Rgb([90, 150, 250]))
            .write_to(&mut bytes, ImageFormat::Jpeg)
            .unwrap();
        bytes.into_inner()
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
