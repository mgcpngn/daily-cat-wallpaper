use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

use crate::config::{AppConfig, CatImageType};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CatCandidate {
    pub id: String,
    pub uri: String,
    pub source: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SourceError {
    #[error("no cat image source is enabled")]
    NoSources,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourcePlanner {
    pub local_dirs: Vec<PathBuf>,
    pub wikimedia_commons_enabled: bool,
    pub cataas_enabled: bool,
    pub the_cat_api_enabled: bool,
    pub breeds: Vec<String>,
    pub image_types: Vec<CatImageType>,
    pub pixabay_api_key: Option<String>,
    pub magnific_api_key: Option<String>,
    pub pexels_api_key: Option<String>,
}

impl Default for SourcePlanner {
    fn default() -> Self {
        Self {
            local_dirs: Vec::new(),
            wikimedia_commons_enabled: true,
            cataas_enabled: true,
            the_cat_api_enabled: true,
            breeds: Vec::new(),
            image_types: Vec::new(),
            pixabay_api_key: None,
            magnific_api_key: None,
            pexels_api_key: None,
        }
    }
}

impl SourcePlanner {
    pub fn ordered_sources(&self) -> Result<Vec<String>, SourceError> {
        let mut sources = Vec::new();

        for dir in &self.local_dirs {
            let normalized = dir.to_string_lossy().replace('\\', "/");
            sources.push(format!("local:{normalized}"));
        }
        if self
            .magnific_api_key
            .as_deref()
            .is_some_and(|key| !key.trim().is_empty())
        {
            sources.push("magnific".to_string());
        }
        if self
            .pixabay_api_key
            .as_deref()
            .is_some_and(|key| !key.trim().is_empty())
        {
            sources.push("pixabay".to_string());
        }
        if self
            .pexels_api_key
            .as_deref()
            .is_some_and(|key| !key.trim().is_empty())
        {
            sources.push("pexels".to_string());
        }
        if self.wikimedia_commons_enabled {
            sources.push("wikimedia".to_string());
        }
        if self.the_cat_api_enabled {
            sources.push("thecatapi".to_string());
        }
        if self.cataas_enabled {
            sources.push("cataas".to_string());
        }

        if sources.is_empty() {
            Err(SourceError::NoSources)
        } else {
            Ok(sources)
        }
    }
}

pub fn the_cat_api_breed_ids(breeds: &[String]) -> Vec<&'static str> {
    breeds
        .iter()
        .filter_map(|breed| match normalize_breed(breed).as_str() {
            "british shorthair" => Some("bsho"),
            "ragdoll" => Some("ragd"),
            "maine coon" => Some("mcoo"),
            "siamese" => Some("siam"),
            _ => None,
        })
        .collect()
}

pub fn wikimedia_search_query(breeds: &[String], image_types: &[CatImageType]) -> String {
    cat_search_query(breeds, image_types)
}

pub fn magnific_search_query(breeds: &[String], image_types: &[CatImageType]) -> String {
    format!("{} HD wallpaper", cat_search_query(breeds, image_types))
}

pub fn pixabay_search_query(breeds: &[String], image_types: &[CatImageType]) -> String {
    format!("{} wallpaper", cat_search_query(breeds, image_types))
}

pub fn pexels_search_query(breeds: &[String], image_types: &[CatImageType]) -> String {
    format!("{} wallpaper", cat_search_query(breeds, image_types))
}

pub fn transparent_cat_prompt(config: &AppConfig) -> String {
    let breed = config
        .breeds
        .iter()
        .map(|breed| normalize_breed(breed))
        .find(|breed| breed != "mixed")
        .unwrap_or_else(|| "mixed breed cat".to_string());
    let moods = config
        .image_types
        .iter()
        .map(|image_type| match image_type {
            CatImageType::Healing => "calm and comforting",
            CatImageType::Funny => "playful and expressive",
            CatImageType::Loaf => "loaf pose",
            CatImageType::Kitten => "kitten-like charm",
            CatImageType::Sleepy => "sleepy and relaxed",
        })
        .collect::<Vec<_>>()
        .join(", ");
    let scene = config.ai_generation.scene.trim();
    let scene = if scene.is_empty() {
        "sitting naturally on the desktop edge"
    } else {
        scene
    };

    format!(
        "Create one lifelike full-body {breed} cat as a transparent PNG cutout. \
         The cat should feel like a small pet living on a computer desktop: {scene}. \
         Mood cues: {moods}. No room, no wall, no floor, no props, no text, no frame, \
         no shadow baked into a background. Isolated subject only, clean alpha channel, \
         detailed fur, natural paws, ears, whiskers, and tail, complete body visible."
    )
}

fn cat_search_query(breeds: &[String], image_types: &[CatImageType]) -> String {
    let breed = breeds
        .iter()
        .map(|breed| normalize_breed(breed))
        .find(|breed| breed != "mixed")
        .unwrap_or_else(|| "cat".to_string());
    let mut terms = if breed.contains("cat") {
        breed
    } else {
        format!("{breed} cat")
    };

    for image_type in image_types {
        let term = match image_type {
            CatImageType::Healing | CatImageType::Funny | CatImageType::Loaf => continue,
            CatImageType::Kitten => "kitten",
            CatImageType::Sleepy => "sleeping",
        };
        if !terms.contains(term) {
            terms.push(' ');
            terms.push_str(term);
        }
    }

    terms
}

pub fn normalize_breed(breed: &str) -> String {
    breed.trim().to_ascii_lowercase()
}
