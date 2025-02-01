use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Server bind address
    pub bind_addr: String,
    /// Maximum number of connections
    pub max_connections: u32,
    /// Connection timeout in seconds
    pub connection_timeout: Duration,
    /// Health check interval in seconds
    pub health_check_interval: Duration,
    /// Shutdown timeout in seconds
    pub shutdown_timeout: Duration,
    /// Runtime directory
    pub runtime_dir: PathBuf,
    /// Log level
    pub log_level: String,
    /// Enable metrics collection
    pub enable_metrics: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:0".to_string(),
            max_connections: 1000,
            connection_timeout: Duration::from_secs(30),
            health_check_interval: Duration::from_secs(30),
            shutdown_timeout: Duration::from_secs(5),
            runtime_dir: std::env::temp_dir(),
            log_level: "info".to_string(),
            enable_metrics: true,
        }
    }
}

impl ServerConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_bind_addr(mut self, addr: String) -> Self {
        self.bind_addr = addr;
        self
    }

    pub fn with_max_connections(mut self, max: u32) -> Self {
        self.max_connections = max;
        self
    }

    pub fn with_connection_timeout(mut self, timeout: Duration) -> Self {
        self.connection_timeout = timeout;
        self
    }

    pub fn with_health_check_interval(mut self, interval: Duration) -> Self {
        self.health_check_interval = interval;
        self
    }

    pub fn with_shutdown_timeout(mut self, timeout: Duration) -> Self {
        self.shutdown_timeout = timeout;
        self
    }

    pub fn with_runtime_dir(mut self, dir: PathBuf) -> Self {
        self.runtime_dir = dir;
        self
    }

    pub fn with_log_level(mut self, level: String) -> Self {
        self.log_level = level;
        self
    }

    pub fn with_metrics_enabled(mut self, enabled: bool) -> Self {
        self.enable_metrics = enabled;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_default_config() {
        let config = ServerConfig::default();
        assert_eq!(config.bind_addr, "127.0.0.1:0");
        assert_eq!(config.max_connections, 1000);
        assert_eq!(config.connection_timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_config_builder() {
        let config = ServerConfig::new()
            .with_bind_addr("0.0.0.0:8080".to_string())
            .with_max_connections(500)
            .with_connection_timeout(Duration::from_secs(60));

        assert_eq!(config.bind_addr, "0.0.0.0:8080");
        assert_eq!(config.max_connections, 500);
        assert_eq!(config.connection_timeout, Duration::from_secs(60));
    }
} 