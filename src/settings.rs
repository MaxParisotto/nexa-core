use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use log::{debug, error};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub theme: Theme,
    pub llm_servers: Vec<LLMServerConfig>,
    pub window_size: WindowSize,
    pub max_logs: usize,
    pub refresh_interval: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Theme {
    Light,
    Dark,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowSize {
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMServerConfig {
    pub provider: String,
    pub url: String,
    pub models: Vec<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: Theme::Dark,
            llm_servers: Vec::new(),
            window_size: WindowSize {
                width: 1920.0,
                height: 1080.0,
            },
            max_logs: 1000,
            refresh_interval: 5,
        }
    }
}

#[derive(Clone)]
pub struct SettingsManager {
    settings: AppSettings,
    file_path: PathBuf,
}

impl SettingsManager {
    pub fn new() -> Self {
        let file_path = Self::get_settings_path();
        let settings = Self::load_settings(&file_path).unwrap_or_default();
        
        Self {
            settings,
            file_path,
        }
    }

    fn get_settings_path() -> PathBuf {
        let mut path = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("nexa");
        
        fs::create_dir_all(&path).unwrap_or_else(|e| {
            error!("Failed to create config directory: {}", e);
        });
        
        path.push("settings.json");
        path
    }

    fn load_settings(path: &PathBuf) -> Result<AppSettings, String> {
        match fs::read_to_string(path) {
            Ok(contents) => {
                serde_json::from_str(&contents)
                    .map_err(|e| format!("Failed to parse settings: {}", e))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                debug!("Settings file not found, using defaults");
                Ok(AppSettings::default())
            }
            Err(e) => Err(format!("Failed to read settings file: {}", e)),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let contents = serde_json::to_string_pretty(&self.settings)
            .map_err(|e| format!("Failed to serialize settings: {}", e))?;
        
        fs::write(&self.file_path, contents)
            .map_err(|e| format!("Failed to write settings file: {}", e))
    }

    pub fn get(&self) -> &AppSettings {
        &self.settings
    }

    pub fn get_mut(&mut self) -> &mut AppSettings {
        &mut self.settings
    }

    pub fn update<F>(&mut self, f: F) -> Result<(), String>
    where
        F: FnOnce(&mut AppSettings),
    {
        f(&mut self.settings);
        self.save()
    }
} 