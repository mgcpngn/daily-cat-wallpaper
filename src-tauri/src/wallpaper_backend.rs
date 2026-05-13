use daily_cat_core::{BackendCapabilities, InteractionKind, WallpaperBackend};
use std::path::Path;
use thiserror::Error;

#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::process::Command;

#[derive(Debug, Error)]
pub enum WallpaperError {
    #[error("wallpaper path does not exist: {0}")]
    MissingPath(String),
    #[error("wallpaper command failed: {0}")]
    CommandFailed(String),
    #[cfg(not(target_os = "windows"))]
    #[error("platform does not support this wallpaper operation yet")]
    Unsupported,
}

#[derive(Debug, Default)]
pub struct NativeWallpaperBackend;

impl NativeWallpaperBackend {
    pub fn new() -> Self {
        Self
    }
}

impl WallpaperBackend for NativeWallpaperBackend {
    type Error = WallpaperError;

    fn capabilities(&self) -> BackendCapabilities {
        platform_capabilities()
    }

    fn set_wallpaper(&self, image_path: &Path) -> Result<(), Self::Error> {
        if !image_path.exists() {
            return Err(WallpaperError::MissingPath(
                image_path.to_string_lossy().to_string(),
            ));
        }

        set_wallpaper_platform(image_path)
    }

    fn start_interaction_layer(&self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn stop_interaction_layer(&self) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[cfg(target_os = "windows")]
fn platform_capabilities() -> BackendCapabilities {
    BackendCapabilities {
        platform: "windows".to_string(),
        static_wallpaper: true,
        interaction_overlay: true,
        supported_interactions: vec![
            InteractionKind::Breathing,
            InteractionKind::MouseProximity,
            InteractionKind::ClickPaw,
            InteractionKind::KeyboardBongo,
            InteractionKind::Sound,
        ],
        beta: false,
    }
}

#[cfg(target_os = "macos")]
fn platform_capabilities() -> BackendCapabilities {
    BackendCapabilities {
        platform: "macos".to_string(),
        static_wallpaper: true,
        interaction_overlay: false,
        supported_interactions: vec![InteractionKind::Breathing],
        beta: true,
    }
}

#[cfg(target_os = "linux")]
fn platform_capabilities() -> BackendCapabilities {
    BackendCapabilities {
        platform: "linux".to_string(),
        static_wallpaper: true,
        interaction_overlay: false,
        supported_interactions: vec![InteractionKind::Breathing],
        beta: true,
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn platform_capabilities() -> BackendCapabilities {
    BackendCapabilities {
        platform: std::env::consts::OS.to_string(),
        static_wallpaper: false,
        interaction_overlay: false,
        supported_interactions: Vec::new(),
        beta: true,
    }
}

#[cfg(target_os = "windows")]
fn set_wallpaper_platform(image_path: &Path) -> Result<(), WallpaperError> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, SPIF_SENDWININICHANGE, SPIF_UPDATEINIFILE, SPI_SETDESKWALLPAPER,
    };

    let mut encoded: Vec<u16> = image_path.as_os_str().encode_wide().collect();
    encoded.push(0);

    let ok = unsafe {
        SystemParametersInfoW(
            SPI_SETDESKWALLPAPER,
            0,
            encoded.as_ptr() as *mut _,
            SPIF_UPDATEINIFILE | SPIF_SENDWININICHANGE,
        )
    };

    if ok == 0 {
        Err(WallpaperError::CommandFailed(
            "SystemParametersInfoW returned failure".to_string(),
        ))
    } else {
        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn set_wallpaper_platform(image_path: &Path) -> Result<(), WallpaperError> {
    let script = format!(
        "tell application \"System Events\" to tell every desktop to set picture to POSIX file \"{}\"",
        image_path.to_string_lossy().replace('"', "\\\"")
    );
    run_command(Command::new("osascript").arg("-e").arg(script))
}

#[cfg(target_os = "linux")]
fn set_wallpaper_platform(image_path: &Path) -> Result<(), WallpaperError> {
    let uri = format!("file://{}", image_path.to_string_lossy());
    let desktop = std::env::var("XDG_CURRENT_DESKTOP")
        .or_else(|_| std::env::var("DESKTOP_SESSION"))
        .unwrap_or_default()
        .to_ascii_lowercase();

    if desktop.contains("gnome") || desktop.contains("cinnamon") || desktop.contains("unity") {
        run_command(
            Command::new("gsettings")
                .arg("set")
                .arg("org.gnome.desktop.background")
                .arg("picture-uri")
                .arg(&uri),
        )?;
        let _ = run_command(
            Command::new("gsettings")
                .arg("set")
                .arg("org.gnome.desktop.background")
                .arg("picture-uri-dark")
                .arg(&uri),
        );
        return Ok(());
    }

    if desktop.contains("kde") || desktop.contains("plasma") {
        return run_command(Command::new("plasma-apply-wallpaperimage").arg(image_path));
    }

    if desktop.contains("xfce") {
        return run_command(
            Command::new("xfconf-query")
                .arg("-c")
                .arg("xfce4-desktop")
                .arg("-p")
                .arg("/backdrop/screen0/monitor0/workspace0/last-image")
                .arg("-s")
                .arg(image_path),
        );
    }

    Err(WallpaperError::Unsupported)
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn set_wallpaper_platform(_image_path: &Path) -> Result<(), WallpaperError> {
    Err(WallpaperError::Unsupported)
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn run_command(command: &mut Command) -> Result<(), WallpaperError> {
    let output = command
        .output()
        .map_err(|error| WallpaperError::CommandFailed(error.to_string()))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(WallpaperError::CommandFailed(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ))
    }
}
