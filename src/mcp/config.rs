use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use crate::error::NexaError;
use std::fs;
use tracing::debug;
use uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterConfig {
    #[serde(default = "default_cluster_enabled")]
    pub enabled: bool,

    #[serde(default = "default_node_id")]
    pub node_id: String,

    #[serde(default = "default_peers")]
    pub peers: Vec<String>,

    #[serde(default = "default_heartbeat_interval_ms")]
    pub heartbeat_interval_ms: u64,

    #[serde(default = "default_election_timeout_ms")]
    pub election_timeout_ms: u64,

    #[serde(default = "default_quorum_size")]
    pub quorum_size: usize,
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            node_id: uuid::Uuid::new_v4().to_string(),
            peers: Vec::new(),
            heartbeat_interval_ms: 1000,
            election_timeout_ms: 5000,
            quorum_size: 2,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadBalancerConfig {
    #[serde(default = "default_max_pool_size")]
    pub max_pool_size: usize,

    #[serde(default = "default_min_pool_size")]
    pub min_pool_size: usize,

    #[serde(default = "default_connection_timeout_ms")]
    pub connection_timeout_ms: u64,

    #[serde(default = "default_max_retries")]
    pub max_retries: usize,

    #[serde(default = "default_retry_delay_ms")]
    pub retry_delay_ms: u64,

    #[serde(default = "default_health_check_interval_ms")]
    pub health_check_interval_ms: u64,

    #[serde(default = "default_max_connection_lifetime_secs")]
    pub max_connection_lifetime_secs: u64,
}

impl Default for LoadBalancerConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            retry_delay_ms: 100,
            health_check_interval_ms: 1000,
            connection_timeout_ms: 5000,
            max_connection_lifetime_secs: 3600,
            max_pool_size: 100,
            min_pool_size: 10,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_bind_addr")]
    pub bind_addr: String,
    
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
    
    #[serde(default = "default_connection_timeout_secs")]
    pub connection_timeout_secs: u64,
    
    #[serde(default = "default_health_check_interval_secs")]
    pub health_check_interval_secs: u64,
    
    #[serde(default = "default_shutdown_timeout_secs")]
    pub shutdown_timeout_secs: u64,
    
    #[serde(default = "default_runtime_dir")]
    pub runtime_dir: PathBuf,
    
    #[serde(default = "default_log_level")]
    pub log_level: String,
    
    #[serde(default = "default_enable_metrics")]
    pub enable_metrics: bool,

    #[serde(default)]
    pub cluster: ClusterConfig,

    #[serde(default)]
    pub load_balancer: LoadBalancerConfig,
}

fn default_bind_addr() -> String {
    "127.0.0.1:0".to_string()
}

fn default_max_connections() -> u32 {
    1000
}

fn default_connection_timeout_secs() -> u64 {
    30
}

fn default_health_check_interval_secs() -> u64 {
    30
}

fn default_shutdown_timeout_secs() -> u64 {
    5
}

fn default_runtime_dir() -> PathBuf {
    std::env::temp_dir()
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_enable_metrics() -> bool {
    true
}

fn default_cluster_enabled() -> bool {
    false
}

fn default_node_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn default_peers() -> Vec<String> {
    Vec::new()
}

fn default_heartbeat_interval_ms() -> u64 {
    500
}

fn default_election_timeout_ms() -> u64 {
    2000
}

fn default_quorum_size() -> usize {
    2
}

fn default_max_pool_size() -> usize {
    100
}

fn default_min_pool_size() -> usize {
    10
}

fn default_connection_timeout_ms() -> u64 {
    5000
}

fn default_max_retries() -> usize {
    3
}

fn default_retry_delay_ms() -> u64 {
    100
}

fn default_health_check_interval_ms() -> u64 {
    5000
}

fn default_max_connection_lifetime_secs() -> u64 {
    300
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: default_bind_addr(),
            max_connections: default_max_connections(),
            connection_timeout_secs: default_connection_timeout_secs(),
            health_check_interval_secs: default_health_check_interval_secs(),
            shutdown_timeout_secs: default_shutdown_timeout_secs(),
            runtime_dir: default_runtime_dir(),
            log_level: default_log_level(),
            enable_metrics: default_enable_metrics(),
            cluster: ClusterConfig {
                enabled: default_cluster_enabled(),
                node_id: default_node_id(),
                peers: default_peers(),
                heartbeat_interval_ms: default_heartbeat_interval_ms(),
                election_timeout_ms: default_election_timeout_ms(),
                quorum_size: default_quorum_size(),
            },
            load_balancer: LoadBalancerConfig {
                max_pool_size: default_max_pool_size(),
                min_pool_size: default_min_pool_size(),
                connection_timeout_ms: default_connection_timeout_ms(),
                max_retries: default_max_retries(),
                retry_delay_ms: default_retry_delay_ms(),
                health_check_interval_ms: default_health_check_interval_ms(),
                max_connection_lifetime_secs: default_max_connection_lifetime_secs(),
            },
        }
    }
}

impl ServerConfig {
    pub fn load(path: &str) -> Result<Self, NexaError> {
        debug!("Loading configuration from {}", path);
        let content = fs::read_to_string(path)?;
        let config: ServerConfig = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    pub fn save(&self, path: &str) -> Result<(), NexaError> {
        debug!("Saving configuration to {}", path);
        let content = serde_yaml::to_string(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    pub fn connection_timeout(&self) -> Duration {
        Duration::from_secs(self.connection_timeout_secs)
    }

    pub fn health_check_interval(&self) -> Duration {
        Duration::from_secs(self.health_check_interval_secs)
    }

    pub fn shutdown_timeout(&self) -> Duration {
        Duration::from_secs(self.shutdown_timeout_secs)
    }

    pub fn validate(&self) -> Result<(), NexaError> {
        // Validate bind address format
        if self.bind_addr.split(':').count() != 2 {
            return Err(NexaError::system("Invalid bind address format"));
        }

        // Validate timeouts
        if self.connection_timeout_secs == 0 {
            return Err(NexaError::system("Connection timeout cannot be zero"));
        }

        if self.health_check_interval_secs == 0 {
            return Err(NexaError::system("Health check interval cannot be zero"));
        }

        if self.shutdown_timeout_secs == 0 {
            return Err(NexaError::system("Shutdown timeout cannot be zero"));
        }

        // Validate max connections
        if self.max_connections == 0 {
            return Err(NexaError::system("Max connections cannot be zero"));
        }

        // Validate log level
        match self.log_level.to_lowercase().as_str() {
            "error" | "warn" | "info" | "debug" | "trace" => Ok(()),
            _ => Err(NexaError::system("Invalid log level")),
        }?;

        // Validate runtime directory
        if !self.runtime_dir.exists() {
            return Err(NexaError::system("Runtime directory does not exist"));
        }

        // Cluster validation
        if self.cluster.enabled {
            if self.cluster.heartbeat_interval_ms == 0 {
                return Err(NexaError::system("Cluster heartbeat interval cannot be zero"));
            }

            if self.cluster.election_timeout_ms <= self.cluster.heartbeat_interval_ms {
                return Err(NexaError::system("Election timeout must be greater than heartbeat interval"));
            }

            if self.cluster.quorum_size < 2 {
                return Err(NexaError::system("Quorum size must be at least 2"));
            }
        }

        // Load balancer validation
        if self.load_balancer.min_pool_size > self.load_balancer.max_pool_size {
            return Err(NexaError::system("Min pool size cannot be greater than max pool size"));
        }

        if self.load_balancer.connection_timeout_ms == 0 {
            return Err(NexaError::system("Load balancer connection timeout cannot be zero"));
        }

        if self.load_balancer.max_connection_lifetime_secs == 0 {
            return Err(NexaError::system("Max connection lifetime cannot be zero"));
        }

        Ok(())
    }

    pub fn to_cluster_config(&self) -> Result<ClusterConfig, NexaError> {
        if !self.cluster.enabled {
            return Err(NexaError::system("Clustering is not enabled"));
        }
        Ok(self.cluster.clone())
    }

    pub fn to_load_balancer_config(&self) -> LoadBalancerConfig {
        self.load_balancer.clone()
    }

    pub fn load_yaml(&mut self, path: &PathBuf) -> Result<(), NexaError> {
        let contents = fs::read_to_string(path)
            .map_err(|err| NexaError::config(format!("Failed to read config file: {}", err)))?;
            
        let config: ServerConfig = serde_yaml::from_str(&contents)
            .map_err(|err| NexaError::config(format!("YAML error: {}", err)))?;
            
        // Update configuration
        *self = config;
        Ok(())
    }
}

impl From<serde_yaml::Error> for NexaError {
    fn from(err: serde_yaml::Error) -> Self {
        NexaError::config(format!("YAML error: {}", err))
    }
} 