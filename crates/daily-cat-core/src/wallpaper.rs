use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum InteractionKind {
    Breathing,
    MouseProximity,
    ClickPaw,
    KeyboardBongo,
    Sound,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackendCapabilities {
    pub platform: String,
    pub static_wallpaper: bool,
    pub interaction_overlay: bool,
    pub supported_interactions: Vec<InteractionKind>,
    pub beta: bool,
}

pub trait WallpaperBackend {
    type Error;

    fn capabilities(&self) -> BackendCapabilities;
    fn set_wallpaper(&self, image_path: &Path) -> Result<(), Self::Error>;
    fn start_interaction_layer(&self) -> Result<(), Self::Error>;
    fn stop_interaction_layer(&self) -> Result<(), Self::Error>;
}
