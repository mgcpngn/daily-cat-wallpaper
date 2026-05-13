use daily_cat_core::{AppConfig, CatCandidate, SourceError, SourcePlanner};
use directories::ProjectDirs;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
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
    #[error("no image candidate could be resolved")]
    NoCandidate,
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
    ) -> Result<PathBuf, AppStateError> {
        let sources = planner.ordered_sources()?;
        for source in sources {
            if let Some(path) = self.resolve_source(&source).await? {
                return Ok(path);
            }
        }

        Err(AppStateError::NoCandidate)
    }

    fn config_path(&self) -> PathBuf {
        self.data_dir.join("config.json")
    }

    async fn resolve_source(&self, source: &str) -> Result<Option<PathBuf>, AppStateError> {
        if let Some(dir) = source.strip_prefix("local:") {
            return Ok(first_local_image(Path::new(dir)));
        }

        let candidate = match source {
            "cataas" => Some(CatCandidate {
                id: "cataas-random".to_string(),
                uri: "https://cataas.com/cat?width=1920&height=1080".to_string(),
                source: "cataas".to_string(),
            }),
            "thecatapi" => self.the_cat_api_candidate().await?,
            _ => None,
        };

        let Some(candidate) = candidate else {
            return Ok(None);
        };

        let cache_dir = self.data_dir.join("cache");
        fs::create_dir_all(&cache_dir)?;
        let output_path = cache_dir.join(format!("{}.jpg", sanitize_file_name(&candidate.id)));
        let bytes = self
            .client
            .get(&candidate.uri)
            .send()
            .await?
            .bytes()
            .await?;
        fs::write(&output_path, bytes)?;
        Ok(Some(output_path))
    }

    async fn the_cat_api_candidate(&self) -> Result<Option<CatCandidate>, AppStateError> {
        #[derive(Debug, Deserialize)]
        struct TheCatApiImage {
            id: String,
            url: String,
        }

        let images = self
            .client
            .get("https://api.thecatapi.com/v1/images/search?mime_types=jpg,png&limit=1")
            .send()
            .await?
            .json::<Vec<TheCatApiImage>>()
            .await?;

        Ok(images.into_iter().next().map(|image| CatCandidate {
            id: image.id,
            uri: image.url,
            source: "thecatapi".to_string(),
        }))
    }
}

fn first_local_image(dir: &Path) -> Option<PathBuf> {
    let entries = fs::read_dir(dir).ok()?;
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.is_file()
                && path
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .map(|extension| {
                        matches!(
                            extension.to_ascii_lowercase().as_str(),
                            "jpg" | "jpeg" | "png" | "webp"
                        )
                    })
                    .unwrap_or(false)
        })
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
