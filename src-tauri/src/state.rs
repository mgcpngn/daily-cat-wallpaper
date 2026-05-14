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
use image::{GenericImage, GenericImageView, ImageFormat};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
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
    #[error("AI provider rejected the request: {0}")]
    AiProvider(String),
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
    pub thumbnail_data_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImportImagePayload {
    pub file_name: String,
    pub data_base64: String,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApiKeyValidation {
    pub provider: AiImageProvider,
    pub model: String,
    pub valid: bool,
    pub message: String,
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
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(180))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
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

    pub fn gallery_import_dir(&self) -> PathBuf {
        self.gallery_dir().join("imports")
    }

    pub fn list_gallery_images(
        &self,
        quality: &ImageQuality,
    ) -> Result<Vec<GalleryImage>, AppStateError> {
        let mut images = Vec::new();
        let feedback = self.load_feedback()?;
        for (child, source) in [
            ("imports", "imported"),
            ("downloads", "downloaded"),
            ("generated", "generated"),
        ] {
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
                    thumbnail_data_url: thumbnail_data_url(&path).unwrap_or_default(),
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

    pub fn import_gallery_payloads(
        &self,
        payloads: &[ImportImagePayload],
        quality: &ImageQuality,
    ) -> Result<Vec<GalleryImage>, AppStateError> {
        let mut imported = Vec::new();
        let import_dir = self.gallery_import_dir();
        for payload in payloads {
            let encoded = payload
                .data_base64
                .rsplit_once(',')
                .map(|(_, data)| data)
                .unwrap_or(payload.data_base64.as_str());
            let bytes = base64::engine::general_purpose::STANDARD.decode(encoded.trim())?;
            let target = import_target_path(&import_dir, &payload.file_name);
            if !write_verified_image(&target, &bytes, quality)? {
                continue;
            }
            imported.push(gallery_image_from_path(
                &target,
                &payload.file_name,
                "imported",
                quality,
            )?);
        }
        if imported.is_empty() {
            Err(AppStateError::NoCandidate)
        } else {
            Ok(imported)
        }
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

    pub async fn validate_ai_api_key(
        &self,
        config: &AppConfig,
    ) -> Result<ApiKeyValidation, AppStateError> {
        match config.ai_generation.provider {
            AiImageProvider::OpenAi => {
                let api_key = config
                    .ai_generation
                    .openai_api_key
                    .as_deref()
                    .map(str::trim)
                    .filter(|key| !key.is_empty())
                    .ok_or(AppStateError::MissingAiApiKey)?;
                let model = openai_image_model(config).to_string();
                let response = self
                    .client
                    .get(format!("https://api.openai.com/v1/models/{model}"))
                    .bearer_auth(api_key)
                    .send()
                    .await?;
                provider_response_or_error(response).await?;
                Ok(ApiKeyValidation {
                    provider: AiImageProvider::OpenAi,
                    model,
                    valid: true,
                    message: "OpenAI API key and image model are reachable".to_string(),
                })
            }
            AiImageProvider::GoogleNanoBananaPro => {
                let api_key = config
                    .ai_generation
                    .google_api_key
                    .as_deref()
                    .map(str::trim)
                    .filter(|key| !key.is_empty())
                    .ok_or(AppStateError::MissingAiApiKey)?;
                let model = google_model_name(config);
                let response = self
                    .client
                    .get(format!(
                        "https://generativelanguage.googleapis.com/v1beta/models/{model}"
                    ))
                    .header("x-goog-api-key", api_key)
                    .send()
                    .await?;
                provider_response_or_error(response).await?;
                Ok(ApiKeyValidation {
                    provider: AiImageProvider::GoogleNanoBananaPro,
                    model,
                    valid: true,
                    message: "Gemini API key and image model are reachable".to_string(),
                })
            }
        }
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
        let model = openai_image_model(config);

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
            .await?;
        let response = provider_response_or_error(response)
            .await?
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
        let model = google_model_name(config);

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

        let response = self
            .client
            .post(format!(
                "https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent"
            ))
            .header("x-goog-api-key", api_key)
            .json(&google_image_request(prompt, count))
            .send()
            .await?;
        let response = provider_response_or_error(response)
            .await?
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

async fn provider_response_or_error(
    response: reqwest::Response,
) -> Result<reqwest::Response, AppStateError> {
    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    let body = body.chars().take(600).collect::<String>();
    Err(AppStateError::AiProvider(format!("{status}: {body}")))
}

fn openai_image_model(config: &AppConfig) -> &str {
    if config.ai_generation.transparent_cutout
        && config.ai_generation.openai_model.trim() == "gpt-image-2"
    {
        "gpt-image-1.5"
    } else {
        config.ai_generation.openai_model.trim()
    }
}

fn google_model_name(config: &AppConfig) -> String {
    config
        .ai_generation
        .google_model
        .trim()
        .trim_start_matches("models/")
        .to_string()
}

fn google_image_request(prompt: &str, count: usize) -> serde_json::Value {
    let prompt = if count <= 1 {
        prompt.to_string()
    } else {
        format!("{prompt} Generate {count} distinct cat cutout variants in this response.")
    };
    serde_json::json!({
        "contents": [{"parts": [{"text": prompt}]}],
        "generationConfig": {
            "responseModalities": ["TEXT", "IMAGE"],
            "imageConfig": {
                "aspectRatio": "1:1",
                "imageSize": "2K"
            }
        }
    })
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

fn import_target_path(import_dir: &Path, file_name: &str) -> PathBuf {
    let original = Path::new(file_name);
    let stem = original
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(sanitize_file_name)
        .filter(|stem| !stem.is_empty())
        .unwrap_or_else(|| "cat-image".to_string());
    let extension = original
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .filter(|extension| matches!(extension.as_str(), "jpg" | "jpeg" | "png" | "webp"))
        .unwrap_or_else(|| "png".to_string());
    import_dir.join(format!("{stem}-{}.{}", unique_suffix(), extension))
}

fn gallery_image_from_path(
    path: &Path,
    display_name: &str,
    source: &str,
    quality: &ImageQuality,
) -> Result<GalleryImage, AppStateError> {
    let (width, height) = image::image_dimensions(path)?;
    Ok(GalleryImage {
        path: path.to_path_buf(),
        file_name: display_name.to_string(),
        source: source.to_string(),
        width,
        height,
        meets_quality: width >= quality.min_width && height >= quality.min_height,
        transparent: image_has_transparency(path),
        feedback_score: 0,
        rejected: false,
        thumbnail_data_url: thumbnail_data_url(path)?,
    })
}

fn thumbnail_data_url(path: &Path) -> Result<String, AppStateError> {
    let image = image::open(path)?.thumbnail(512, 512);
    let mut bytes = Cursor::new(Vec::new());
    image.write_to(&mut bytes, ImageFormat::Png)?;
    Ok(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes.into_inner())
    ))
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
        data_dir.join("cat-gallery").join("imports"),
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
    fn import_gallery_payloads_writes_selected_browser_files() {
        let dir = tempdir().unwrap();
        let state = test_state(dir.path());
        let mut bytes = Cursor::new(Vec::new());
        transparent_test_cat(2560, 1440, 240, 120)
            .write_to(&mut bytes, ImageFormat::Png)
            .unwrap();

        let imported = state
            .import_gallery_payloads(
                &[ImportImagePayload {
                    file_name: "picked-cat.png".to_string(),
                    data_base64: base64::engine::general_purpose::STANDARD
                        .encode(bytes.into_inner()),
                }],
                &ImageQuality::default(),
            )
            .unwrap();

        assert_eq!(imported.len(), 1);
        assert_eq!(imported[0].file_name, "picked-cat.png");
        assert!(imported[0].transparent);
        assert!(imported[0].path.starts_with(state.gallery_import_dir()));
    }

    #[test]
    fn import_gallery_payloads_rejects_low_resolution_browser_files() {
        let dir = tempdir().unwrap();
        let state = test_state(dir.path());

        let result = state.import_gallery_payloads(
            &[ImportImagePayload {
                file_name: "small-cat.jpg".to_string(),
                data_base64: base64::engine::general_purpose::STANDARD
                    .encode(encoded_test_image(1280, 720)),
            }],
            &ImageQuality::default(),
        );

        assert!(matches!(result, Err(AppStateError::NoCandidate)));
        assert!(!state.gallery_import_dir().exists());
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
        assert!(images.iter().all(|image| image
            .thumbnail_data_url
            .starts_with("data:image/png;base64,")));
    }

    #[test]
    fn google_image_request_asks_for_image_and_2k_output() {
        let request = google_image_request("transparent cat prompt", 2);

        assert_eq!(
            request["generationConfig"]["responseModalities"],
            serde_json::json!(["TEXT", "IMAGE"])
        );
        assert_eq!(
            request["generationConfig"]["imageConfig"]["imageSize"],
            serde_json::json!("2K")
        );
        assert!(request["contents"][0]["parts"][0]["text"]
            .as_str()
            .unwrap()
            .contains("transparent cat prompt"));
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
}
