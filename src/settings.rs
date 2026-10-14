//! Launcher settings that live outside the server config: theme, server location,
//! the mod library, known scenarios, startup parameters and the window layout.

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::Mod;
use crate::launch::LaunchOptions;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ThemeChoice {
    #[default]
    Dark,
    /// Follow the operating system's light or dark setting.
    System,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub theme: ThemeChoice,
    /// The server install folder, or the server executable itself.
    pub server_path: Option<PathBuf>,
    /// The config file that was open when the launcher last ran.
    pub last_config: Option<PathBuf>,
    /// Mods the user has added to the library ("Mods Library").
    pub available_mods: Vec<Mod>,
    /// Scenario IDs found by the last "Reload Scenarios".
    pub scenarios: Vec<String>,
    pub launch: LaunchOptions,
    /// Saved window layout. Kept as raw JSON so an unreadable layout never costs the other settings.
    pub dock: Option<Value>,
}

pub fn settings_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "Arsenal")
        .map(|dirs| dirs.config_dir().join("settings.json"))
}

impl AppSettings {
    /// Loads the saved settings, or defaults when there are none or they cannot be read.
    pub fn load() -> Self {
        settings_path()
            .and_then(|path| fs::read_to_string(path).ok())
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        let path = settings_path().context("No settings folder is available on this system")?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self)?;
        fs::write(&path, text).with_context(|| format!("Could not write {}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let s: AppSettings = serde_json::from_str(r#"{ "theme": "System" }"#).unwrap();
        assert_eq!(s.theme, ThemeChoice::System);
        assert!(s.available_mods.is_empty());
        assert_eq!(s.launch.auto_restart_time, "04:00");
    }

    #[test]
    fn dark_is_the_default_theme() {
        assert_eq!(AppSettings::default().theme, ThemeChoice::Dark);
    }

    #[test]
    fn settings_round_trip() {
        let mut s = AppSettings::default();
        s.theme = ThemeChoice::System;
        s.scenarios.push("{ECC61978EDCC2B5A}Missions/23_Campaign.conf".into());
        let text = serde_json::to_string(&s).unwrap();
        let back: AppSettings = serde_json::from_str(&text).unwrap();
        assert_eq!(back.theme, ThemeChoice::System);
        assert_eq!(back.scenarios, s.scenarios);
    }
}
