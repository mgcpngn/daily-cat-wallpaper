pub mod config;
pub mod layout;
pub mod scheduler;
pub mod sources;
pub mod wallpaper;

pub use config::{AppConfig, ConfigError};
pub use layout::{Canvas, LayoutEngine, Rect, SafeArea};
pub use scheduler::{RefreshDecision, RefreshTrigger, Scheduler};
pub use sources::{CatCandidate, SourceError, SourcePlanner};
pub use wallpaper::{BackendCapabilities, InteractionKind, WallpaperBackend};
