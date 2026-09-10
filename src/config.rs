//! Persistent settings + recents store (spec F92/F9), atomic JSON.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::theme::ThemeMode;

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    pub theme: ThemeMode,
    pub zoom: f32,
    pub wrap_editor: bool,
    pub show_line_numbers: bool,
    pub recent: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: ThemeMode::System,
            zoom: 1.0,
            wrap_editor: true,
            show_line_numbers: true,
            recent: Vec::new(),
        }
    }
}

impl Settings {
    pub fn config_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("RustDownViewer")
    }

    fn path() -> PathBuf {
        Self::config_dir().join("config.json")
    }

    pub fn load() -> Self {
        let p = Self::path();
        match std::fs::read_to_string(&p).ok().and_then(|s| serde_json::from_str(&s).ok()) {
            Some(cfg) => cfg,
            None => Self::default(),
        }
    }

    pub fn save(&self) {
        let dir = Self::config_dir();
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(Self::path(), json);
        }
    }

    pub fn push_recent(&mut self, path: &Path) {
        let s = path.display().to_string();
        self.recent.retain(|r| r != &s);
        self.recent.insert(0, s);
        self.recent.truncate(10);
    }

    pub fn recent_paths(&self) -> Vec<PathBuf> {
        self.recent.iter().map(PathBuf::from).collect()
    }
}
