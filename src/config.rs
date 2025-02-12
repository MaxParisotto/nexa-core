use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};
use crate::error::NexaError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_max_connections")]
    pub max_connections: usize,
    #[serde(default)]
    pub llm: LLMConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub monitoring: MonitoringConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LLMConfig {
    pub providers: Vec<LLMProvider>,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMProvider {
    pub name: String,
    pub url: String,
    pub models: Vec<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
    #[serde(default = "default_log_file")]
    pub file: PathBuf,
    #[serde(default = "default_true")]
    pub console: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub detailed_metrics: bool,
    #[serde(default = "default_metrics_interval")]
    pub metrics_interval_secs: u64,
    #[serde(default = "default_history_size")]
    pub history_size: usize,
    #[serde(default = "default_cpu_threshold")]
    pub cpu_threshold: f64,
    #[serde(default = "default_memory_threshold")]
    pub memory_threshold: f64,
    #[serde(default = "default_health_check_interval")]
    pub health_check_interval: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            max_connections: default_max_connections(),
            llm: LLMConfig::default(),
            logging: LoggingConfig {
                level: default_log_level(),
                file: default_log_file(),
                console: default_true(),
            },
            monitoring: MonitoringConfig::default(),
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            file: default_log_file(),
            console: default_true(),
        }
    }
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            detailed_metrics: default_true(),
            metrics_interval_secs: default_metrics_interval(),
            history_size: default_history_size(),
            cpu_threshold: default_cpu_threshold(),
            memory_threshold: default_memory_threshold(),
            health_check_interval: default_health_check_interval(),
        }
    }
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    3000
}

fn default_max_connections() -> usize {
    1000
}

fn default_timeout() -> u64 {
    30
}

fn default_max_tokens() -> usize {
    2000
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_log_file() -> PathBuf {
    PathBuf::from("logs/nexa.log")
}

fn default_true() -> bool {
    true
}

fn default_metrics_interval() -> u64 {
    60
}

fn default_history_size() -> usize {
    1000
}

fn default_cpu_threshold() -> f64 {
    80.0 // 80% CPU threshold
}

fn default_memory_threshold() -> f64 {
    90.0 // 90% memory threshold
}

fn default_health_check_interval() -> u64 {
    30 // 30 seconds
}

impl ServerConfig {
    pub fn load() -> Result<Self, NexaError> {
        // Try loading from different locations in order
        let config_paths = [
            // Current directory
            PathBuf::from("config.yml"),
            // User's config directory
            dirs::config_dir()
                .map(|p| p.join("nexa/config.yml"))
                .unwrap_or_default(),
            // System-wide config
            PathBuf::from("/etc/nexa/config.yml"),
        ];

        for path in &config_paths {
            if path.exists() {
                return Self::load_from_file(path);
            }
        }

        // If no config file found, create default one in current directory
        let config = Self::default();
        let yaml = serde_yaml::to_string(&config)
            .map_err(|e| NexaError::Config(format!("Failed to serialize default config: {}", e)))?;
        
        fs::write("config.yml", yaml)
            .map_err(|e| NexaError::Config(format!("Failed to write default config: {}", e)))?;

        Ok(config)
    }

    pub fn load_from_file(path: &PathBuf) -> Result<Self, NexaError> {
        let content = fs::read_to_string(path)
            .map_err(|e| NexaError::Config(format!("Failed to read config file: {}", e)))?;

        serde_yaml::from_str(&content)
            .map_err(|e| NexaError::Config(format!("Failed to parse config file: {}", e)))
    }

    #[allow(dead_code)]
    pub fn save(&self, path: &PathBuf) -> Result<(), NexaError> {
        let yaml = serde_yaml::to_string(self)
            .map_err(|e| NexaError::Config(format!("Failed to serialize config: {}", e)))?;

        // Create parent directories if they don't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| NexaError::Config(format!("Failed to create config directory: {}", e)))?;
        }

        // Write atomically using a temporary file
        let temp_path = path.with_extension("tmp");
        fs::write(&temp_path, yaml)
            .map_err(|e| NexaError::Config(format!("Failed to write config: {}", e)))?;
        
        fs::rename(&temp_path, path)
            .map_err(|e| NexaError::Config(format!("Failed to save config: {}", e)))?;

        Ok(())
    }
} 