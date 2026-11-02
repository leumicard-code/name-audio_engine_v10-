// ============================================================================
// config.rs — Persistent User Settings, EQ Curve & Playlist Storage
// ============================================================================

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::theme::ThemeMode;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedTrack {
    pub path: PathBuf,
    pub title: String,
    pub artist: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct UserConfig {
    pub theme: ThemeMode,
    pub volume_pct: i32,
    pub eq_bands: [f64; 10],
    pub playlist: Vec<SavedTrack>,
    pub current_idx: Option<usize>,
}

impl Default for UserConfig {
    fn default() -> Self {
        Self {
            theme: load_legacy_theme().unwrap_or(ThemeMode::DarkElegance),
            volume_pct: 80,
            eq_bands: [0.0; 10],
            playlist: Vec::new(),
            current_idx: None,
        }
    }
}

fn app_config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("audio_engine_v10")
}

fn config_file_path() -> PathBuf {
    app_config_dir().join("config.json")
}

fn load_legacy_theme() -> Option<ThemeMode> {
    let legacy = app_config_dir().join("theme.json");
    fs::read_to_string(legacy)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
}

impl UserConfig {
    /// Loads saved state from disk, falling back cleanly to defaults.
    pub fn load() -> Self {
        let mut cfg: Self = fs::read_to_string(config_file_path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        cfg.volume_pct = cfg.volume_pct.clamp(0, 100);
        for gain in &mut cfg.eq_bands {
            *gain = gain.clamp(-12.0, 12.0);
        }

        // Retain only audio files that still exist on disk
        cfg.playlist.retain(|t| t.path.exists());
        if let Some(idx) = cfg.current_idx {
            if idx >= cfg.playlist.len() {
                cfg.current_idx = if cfg.playlist.is_empty() { None } else { Some(0) };
            }
        }

        cfg
    }

    /// Saves current state atomically to XDG config directory.
    pub fn save(&self) {
        let dir = app_config_dir();
        let _ = fs::create_dir_all(&dir);
        let path = config_file_path();
        let tmp = dir.join("config.json.tmp");
        if let Ok(json) = serde_json::to_string_pretty(self) {
            if fs::write(&tmp, json).is_ok() {
                let _ = fs::rename(&tmp, &path);
            }
        }
    }
}
