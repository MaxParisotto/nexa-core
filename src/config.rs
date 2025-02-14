use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};
use crate::error::NexaError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub server: ServerSettings,
    pub monitoring: MonitoringConfig,
    pub logging: LoggingConfig,
    pub llm: LLMConfig,
    pub api: ApiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerSettings {
    pub host: String,
    pub port: u16,
    pub max_connections: usize,
    pub connection_timeout: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub endpoints: Vec<String>,
    pub timeout: u64,
    pub retry_attempts: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMConfig {
    pub default_model: String,
    pub providers: Vec<String>,
    pub timeout: u64,
    pub max_tokens: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub file: PathBuf,
    pub console: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    pub enabled: bool,
    pub detailed_metrics: bool,
    pub metrics_interval: u64,
    pub history_size: usize,
    pub health_check_interval: u64,
    pub cpu_threshold: f64,
    pub memory_threshold: f64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            server: ServerSettings {
                host: "127.0.0.1".to_string(),
                port: 8080,
                max_connections: 1000,
                connection_timeout: 30,
            },
            monitoring: MonitoringConfig::default(),
            logging: LoggingConfig::default(),
            llm: LLMConfig::default(),
            api: ApiConfig {
                endpoints: vec![
                    "http://localhost:8080/health".to_string(),
                    "http://localhost:8080/metrics".to_string(),
                ],
                timeout: 30,
                retry_attempts: 3,
            },
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            file: PathBuf::from("/var/log/nexa/server.log"),
            console: true,
        }
    }
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            detailed_metrics: true,
            metrics_interval: 60,  // 1 minute
            history_size: 1000,    // Store last 1000 metrics
            health_check_interval: 30,  // 30 seconds
            cpu_threshold: 80.0,   // 80%
            memory_threshold: 90.0, // 90%
        }
    }
}

impl Default for LLMConfig {
    fn default() -> Self {
        Self {
            default_model: "gpt-3.5-turbo".to_string(),
            providers: vec!["openai".to_string()],
            timeout: 30,
            max_tokens: 2048,
        }
    }
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

    pub fn validate(&self) -> Result<(), NexaError> {
        // Validate server settings
        if self.server.port == 0 {
            return Err(NexaError::Config("Invalid server port".to_string()));
        }
        if self.server.max_connections == 0 {
            return Err(NexaError::Config("Invalid max connections".to_string()));
        }
        if self.server.connection_timeout == 0 {
            return Err(NexaError::Config("Invalid connection timeout".to_string()));
        }

        // Validate monitoring config
        if self.monitoring.metrics_interval == 0 {
            return Err(NexaError::Config("Invalid metrics interval".to_string()));
        }

        // Validate logging config
        if self.logging.level.is_empty() {
            return Err(NexaError::Config("Invalid log level".to_string()));
        }

        // Validate LLM config
        if self.llm.default_model.is_empty() {
            return Err(NexaError::Config("Invalid default model".to_string()));
        }
        if self.llm.providers.is_empty() {
            return Err(NexaError::Config("No LLM providers configured".to_string()));
        }
        if self.llm.timeout == 0 {
            return Err(NexaError::Config("Invalid LLM timeout".to_string()));
        }
        if self.llm.max_tokens == 0 {
            return Err(NexaError::Config("Invalid max tokens".to_string()));
        }

        // Validate API config
        if self.api.timeout == 0 {
            return Err(NexaError::Config("Invalid API timeout".to_string()));
        }
        if self.api.retry_attempts == 0 {
            return Err(NexaError::Config("Invalid retry attempts".to_string()));
        }

        Ok(())
    }

    pub fn get_api_endpoints(&self) -> Vec<String> {
        self.api.endpoints.clone()
    }
} 