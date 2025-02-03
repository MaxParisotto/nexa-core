//! Configuration Management
//! 
//! Provides functionality for:
//! - Loading/saving configuration
//! - Configuration validation
//! - Hot reload support
//! - Default configuration

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use crate::error::NexaError;
use std::fs;
use log::debug;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Server listening address
    pub host: String,
    /// Server listening port
    pub port: u16,
    /// Maximum number of concurrent connections
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
    /// Connection timeout in seconds
    #[serde(default = "default_connection_timeout")]
    pub connection_timeout: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// CPU usage threshold percentage
    #[serde(default = "default_cpu_threshold")]
    pub cpu_threshold: f64,
    /// Memory usage threshold percentage
    #[serde(default = "default_memory_threshold")]
    pub memory_threshold: f64,
    /// Health check interval in seconds
    #[serde(default = "default_health_check_interval")]
    pub health_check_interval: u64,
    /// Enable detailed metrics collection
    #[serde(default = "default_detailed_metrics")]
    pub detailed_metrics: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level (trace, debug, info, warn, error)
    #[serde(default = "default_log_level")]
    pub level: String,
    /// Log file path
    #[serde(default = "default_log_file")]
    pub file: String,
    /// Maximum log file size in MB
    #[serde(default = "default_max_log_size")]
    pub max_size: u64,
    /// Number of log files to keep
    #[serde(default = "default_log_files")]
    pub files_to_keep: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub monitoring: MonitoringConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
}

// Default implementations
impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8080,
            max_connections: default_max_connections(),
            connection_timeout: default_connection_timeout(),
        }
    }
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            cpu_threshold: default_cpu_threshold(),
            memory_threshold: default_memory_threshold(),
            health_check_interval: default_health_check_interval(),
            detailed_metrics: default_detailed_metrics(),
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            file: default_log_file(),
            max_size: default_max_log_size(),
            files_to_keep: default_log_files(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            monitoring: MonitoringConfig::default(),
            logging: LoggingConfig::default(),
        }
    }
}

// Default value functions
fn default_max_connections() -> u32 { 1000 }
fn default_connection_timeout() -> u64 { 30 }
fn default_cpu_threshold() -> f64 { 80.0 }
fn default_memory_threshold() -> f64 { 90.0 }
fn default_health_check_interval() -> u64 { 30 }
fn default_detailed_metrics() -> bool { false }
fn default_log_level() -> String { "info".to_string() }
fn default_log_file() -> String { "nexa.log".to_string() }
fn default_max_log_size() -> u64 { 100 }
fn default_log_files() -> u32 { 5 }

impl Config {
    /// Load configuration from file
    pub fn load(path: &PathBuf) -> Result<Self, NexaError> {
        if !path.exists() {
            debug!("Configuration file not found at {:?}, creating default", path);
            let config = Config::default();
            config.save(path)?;
            return Ok(config);
        }

        let contents = fs::read_to_string(path)
            .map_err(|e| NexaError::Config(format!("Failed to read config file: {}", e)))?;

        serde_yaml::from_str(&contents)
            .map_err(|e| NexaError::Config(format!("Failed to parse config file: {}", e)))
    }

    /// Save configuration to file
    pub fn save(&self, path: &PathBuf) -> Result<(), NexaError> {
        // Create parent directories if they don't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| NexaError::Config(format!("Failed to create config directory: {}", e)))?;
        }
        let contents = serde_yaml::to_string(&self)
            .map_err(|e| NexaError::Config(format!("Failed to serialize config: {}", e)))?;
        fs::write(path, contents)
            .map_err(|e| NexaError::Config(format!("Failed to write config file: {}", e)))?;
        Ok(())
    }

    /// Get configuration file path
    pub fn get_config_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        PathBuf::from(home).join(".config").join("nexa").join("config.yml")
    }

    /// Reset configuration to defaults
    pub fn reset() -> Self {
        Self::default()
    }
}