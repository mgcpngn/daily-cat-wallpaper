use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppConfig {
    pub breeds: Vec<String>,
    pub cat_count: u8,
    pub image_types: Vec<CatImageType>,
    pub interactions: InteractionConfig,
    pub schedule: ScheduleConfig,
    pub sources: SourceConfig,
    pub platform_mode: PlatformMode,
    pub launch_at_login: bool,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("cat_count must be between 1 and 5")]
    InvalidCatCount,
    #[error("at least one cat breed must be selected")]
    MissingBreeds,
    #[error("at least one image type must be selected")]
    MissingImageTypes,
    #[error("at least one image source must be enabled")]
    MissingSources,
    #[error("refresh interval must be between 1 and 24 hours")]
    InvalidRefreshInterval,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CatImageType {
    Healing,
    Funny,
    Loaf,
    Kitten,
    Sleepy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InteractionConfig {
    pub breathing: bool,
    pub mouse_proximity: bool,
    pub click_paw: bool,
    pub keyboard_bongo: bool,
    pub sound: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ScheduleConfig {
    OnLogin,
    Daily { time: String },
    EveryHours { hours: u8 },
    ManualOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceConfig {
    pub local_dirs: Vec<String>,
    pub cataas: bool,
    pub the_cat_api: bool,
    pub pexels_api_key: Option<String>,
    pub unsplash_access_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PlatformMode {
    Automatic,
    StaticOnly,
    InteractionBeta,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            breeds: vec!["mixed".to_string()],
            cat_count: 1,
            image_types: vec![CatImageType::Healing, CatImageType::Loaf],
            interactions: InteractionConfig::default(),
            schedule: ScheduleConfig::default(),
            sources: SourceConfig::default(),
            platform_mode: PlatformMode::Automatic,
            launch_at_login: true,
        }
    }
}

impl AppConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if !(1..=5).contains(&self.cat_count) {
            return Err(ConfigError::InvalidCatCount);
        }
        if self.breeds.is_empty() {
            return Err(ConfigError::MissingBreeds);
        }
        if self.image_types.is_empty() {
            return Err(ConfigError::MissingImageTypes);
        }
        if !self.sources.has_any_source() {
            return Err(ConfigError::MissingSources);
        }
        if let ScheduleConfig::EveryHours { hours } = self.schedule {
            if !(1..=24).contains(&hours) {
                return Err(ConfigError::InvalidRefreshInterval);
            }
        }
        Ok(())
    }
}

impl Default for InteractionConfig {
    fn default() -> Self {
        Self {
            breathing: true,
            mouse_proximity: true,
            click_paw: false,
            keyboard_bongo: false,
            sound: false,
        }
    }
}

impl Default for ScheduleConfig {
    fn default() -> Self {
        Self::Daily {
            time: "09:00".to_string(),
        }
    }
}

impl Default for SourceConfig {
    fn default() -> Self {
        Self {
            local_dirs: Vec::new(),
            cataas: true,
            the_cat_api: true,
            pexels_api_key: None,
            unsplash_access_key: None,
        }
    }
}

impl SourceConfig {
    pub fn has_any_source(&self) -> bool {
        !self.local_dirs.is_empty()
            || self.cataas
            || self.the_cat_api
            || self
                .pexels_api_key
                .as_deref()
                .is_some_and(|key| !key.trim().is_empty())
            || self
                .unsplash_access_key
                .as_deref()
                .is_some_and(|key| !key.trim().is_empty())
    }
}
