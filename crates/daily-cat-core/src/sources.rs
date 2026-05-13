use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CatCandidate {
    pub id: String,
    pub uri: String,
    pub source: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SourceError {
    #[error("no cat image source is enabled")]
    NoSources,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourcePlanner {
    pub local_dirs: Vec<PathBuf>,
    pub cataas_enabled: bool,
    pub the_cat_api_enabled: bool,
}

impl Default for SourcePlanner {
    fn default() -> Self {
        Self {
            local_dirs: Vec::new(),
            cataas_enabled: true,
            the_cat_api_enabled: true,
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
        if self.cataas_enabled {
            sources.push("cataas".to_string());
        }
        if self.the_cat_api_enabled {
            sources.push("thecatapi".to_string());
        }

        if sources.is_empty() {
            Err(SourceError::NoSources)
        } else {
            Ok(sources)
        }
    }
}
