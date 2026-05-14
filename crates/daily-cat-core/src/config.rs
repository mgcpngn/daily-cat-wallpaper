use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppConfig {
    #[serde(default)]
    pub language: LanguagePreference,
    pub breeds: Vec<String>,
    #[serde(default)]
    pub cat_count_strategy: CatCountStrategy,
    pub cat_count: u8,
    pub image_types: Vec<CatImageType>,
    #[serde(default)]
    pub image_quality: ImageQuality,
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
    #[error("image quality minimum must be at least 1920x1080 and preferred size cannot be smaller than minimum")]
    InvalidImageQuality,
    #[error("at least one cat breed must be selected")]
    MissingBreeds,
    #[error("at least one image type must be selected")]
    MissingImageTypes,
    #[error("at least one image source must be enabled")]
    MissingSources,
    #[error("refresh interval must be between 1 and 24 hours")]
    InvalidRefreshInterval,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum LanguagePreference {
    #[default]
    Auto,
    English,
    SimplifiedChinese,
    TraditionalChinese,
    Japanese,
    Korean,
}

impl LanguagePreference {
    pub fn fallback_locale(&self) -> &'static str {
        match self {
            Self::Auto | Self::English => "en",
            Self::SimplifiedChinese => "zh-Hans",
            Self::TraditionalChinese => "zh-Hant",
            Self::Japanese => "ja",
            Self::Korean => "ko",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum CatCountStrategy {
    #[default]
    MatchDisplays,
    Fixed,
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
pub struct ImageQuality {
    pub min_width: u32,
    pub min_height: u32,
    pub preferred_width: u32,
    pub preferred_height: u32,
    pub allow_low_resolution_fallback: bool,
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
    #[serde(default = "default_true")]
    pub wikimedia_commons: bool,
    pub cataas: bool,
    pub the_cat_api: bool,
    #[serde(default)]
    pub pixabay_api_key: Option<String>,
    #[serde(default)]
    pub magnific_api_key: Option<String>,
    #[serde(default)]
    pub pexels_api_key: Option<String>,
    #[serde(default)]
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
            language: LanguagePreference::Auto,
            breeds: vec!["mixed".to_string()],
            cat_count_strategy: CatCountStrategy::MatchDisplays,
            cat_count: 1,
            image_types: vec![CatImageType::Healing, CatImageType::Loaf],
            image_quality: ImageQuality::default(),
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
        if !self.image_quality.is_valid() {
            return Err(ConfigError::InvalidImageQuality);
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

impl Default for ImageQuality {
    fn default() -> Self {
        Self {
            min_width: 2560,
            min_height: 1440,
            preferred_width: 3840,
            preferred_height: 2160,
            allow_low_resolution_fallback: false,
        }
    }
}

impl ImageQuality {
    pub fn is_valid(&self) -> bool {
        self.min_width >= 1920
            && self.min_height >= 1080
            && self.preferred_width >= self.min_width
            && self.preferred_height >= self.min_height
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
            wikimedia_commons: true,
            cataas: true,
            the_cat_api: true,
            pixabay_api_key: None,
            magnific_api_key: None,
            pexels_api_key: None,
            unsplash_access_key: None,
        }
    }
}

impl SourceConfig {
    pub fn has_any_source(&self) -> bool {
        // The generated fallback is always available, so users can disable every
        // network source without making refresh impossible.
        true
    }
}

fn default_true() -> bool {
    true
}
