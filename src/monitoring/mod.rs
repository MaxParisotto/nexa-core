#![allow(dead_code, unused_imports, unused_variables)]

//! Monitoring System
//! 
//! This module provides real-time monitoring and metrics tracking:
//! - Resource utilization monitoring
//! - Performance metrics
//! - Health checks
//! - Alert system
//! - Metrics aggregation

use log::debug;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;
use chrono::{DateTime, Utc, Duration as ChronoDuration};
use crate::error::NexaError;
use crate::tokens::{TokenManager, TokenMetrics};
use crate::config::MonitoringConfig;
use serde::{Serialize, Deserialize};
use sysinfo::{System, Cpu};
use tokio::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceType {
    Memory,
    CPU,
    Tokens,
    Network,
    Storage,
}

#[derive(Clone, Debug)]
pub struct SystemMetrics {
    pub timestamp: DateTime<Utc>,
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub active_agents: u32,
    pub token_usage: TokenMetrics,
}

impl Default for SystemMetrics {
    fn default() -> Self {
        Self {
            timestamp: Utc::now(),
            cpu_usage: 0.0,
            memory_usage: 0.0,
            active_agents: 0,
            token_usage: TokenMetrics::default(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct SystemHealth {
    pub cpu_healthy: bool,
    pub memory_healthy: bool,
    pub overall_healthy: bool,
}

impl Default for SystemHealth {
    fn default() -> Self {
        Self {
            cpu_healthy: true,
            memory_healthy: true,
            overall_healthy: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SystemAlert {
    pub timestamp: DateTime<Utc>,
    pub level: AlertLevel,
    pub message: String,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    pub name: String,
    pub value: f64,
    pub unit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AlertLevel {
    Info,
    Warning,
    Error,
    Critical,
}

impl std::fmt::Display for AlertLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AlertLevel::Info => write!(f, "INFO"),
            AlertLevel::Warning => write!(f, "WARNING"),
            AlertLevel::Error => write!(f, "ERROR"),
            AlertLevel::Critical => write!(f, "CRITICAL"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ResourceMetrics {
    pub usage: f64,
    pub limit: f64,
    pub available: f64,
}

pub struct MonitoringSystem {
    config: Arc<RwLock<MonitoringConfig>>,
    token_manager: Arc<TokenManager>,
    system: Arc<RwLock<System>>,
    metrics_history: Arc<RwLock<Vec<SystemMetrics>>>,
    health_status: Arc<RwLock<SystemHealth>>,
    alerts: Arc<RwLock<Vec<SystemAlert>>>,
    resources: Arc<RwLock<HashMap<String, Resource>>>,
}

impl MonitoringSystem {
    pub fn new(token_manager: Arc<TokenManager>) -> Self {
        let config = MonitoringConfig::default();
        Self {
            config: Arc::new(RwLock::new(config)),
            token_manager,
            system: Arc::new(RwLock::new(System::new_all())),
            metrics_history: Arc::new(RwLock::new(Vec::new())),
            health_status: Arc::new(RwLock::new(SystemHealth::default())),
            alerts: Arc::new(RwLock::new(Vec::with_capacity(100))),
            resources: Arc::new(RwLock::new(HashMap::with_capacity(10))),
        }
    }

    pub fn with_config(token_manager: Arc<TokenManager>, config: MonitoringConfig) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            token_manager,
            system: Arc::new(RwLock::new(System::new_all())),
            metrics_history: Arc::new(RwLock::new(Vec::new())),
            health_status: Arc::new(RwLock::new(SystemHealth::default())),
            alerts: Arc::new(RwLock::new(Vec::with_capacity(100))),
            resources: Arc::new(RwLock::new(HashMap::with_capacity(10))),
        }
    }

    pub async fn start_monitoring(&self) -> Result<(), NexaError> {
        let config = self.config.read().await;
        if !config.enabled {
            debug!("Monitoring is disabled");
            return Ok(());
        }

        let interval = Duration::from_secs(config.metrics_interval);
        drop(config);

        loop {
            self.collect_metrics(0).await?;
            tokio::time::sleep(interval).await;
        }
    }

    pub async fn collect_metrics(&self, _agent_id: u32) -> Result<SystemMetrics, NexaError> {
        let mut system = self.system.write().await;
        system.refresh_all();

        let cpu_usage = system.cpus().iter().map(|cpu| cpu.cpu_usage()).sum::<f32>() / system.cpus().len() as f32;
        let total_memory = system.total_memory() as f64;
        let used_memory = system.used_memory() as f64;
        let memory_usage = used_memory / total_memory;

        let metrics = SystemMetrics {
            timestamp: Utc::now(),
            cpu_usage: cpu_usage as f64 / 100.0,
            memory_usage,
            active_agents: 0, // TODO: Implement agent tracking
            token_usage: TokenMetrics::default(),
        };

        // Store metrics in history if detailed metrics are enabled
        let config = self.config.read().await;
        if config.detailed_metrics {
            let mut history = self.metrics_history.write().await;
            if history.len() >= config.history_size {
                history.remove(0); // Remove oldest entry if at capacity
            }
            history.push(metrics.clone());
        }

        // Check thresholds
        if metrics.cpu_usage > config.cpu_threshold / 100.0 {
            debug!("CPU usage above threshold: {:.1}%", metrics.cpu_usage * 100.0);
        }
        if metrics.memory_usage > config.memory_threshold / 100.0 {
            debug!("Memory usage above threshold: {:.1}%", metrics.memory_usage * 100.0);
        }

        Ok(metrics)
    }

    pub async fn get_resource_metrics(&self, resource_type: ResourceType) -> Result<ResourceMetrics, NexaError> {
        let metrics = match resource_type {
            ResourceType::Memory => {
                let mut system = self.system.write().await;
                system.refresh_all();
                let total = system.total_memory() as f64;
                let used = system.used_memory() as f64;
                ResourceMetrics {
                    usage: used / total,
                    limit: total,
                    available: total - used,
                }
            }
            ResourceType::CPU => {
                let mut system = self.system.write().await;
                system.refresh_all();
                let usage = system.cpus().iter().map(|cpu| cpu.cpu_usage()).sum::<f32>() / system.cpus().len() as f32;
                ResourceMetrics {
                    usage: usage as f64 / 100.0,
                    limit: 100.0,
                    available: 100.0 - usage as f64,
                }
            }
            ResourceType::Tokens => {
                let token_usage = self.token_manager.get_usage().await?;
                let token_limit = self.token_manager.get_max_tokens().await?;
                ResourceMetrics {
                    usage: token_usage as f64,
                    limit: token_limit as f64,
                    available: (token_limit - token_usage) as f64,
                }
            }
            _ => return Err(NexaError::Resource(format!("Unsupported resource type: {:?}", resource_type))),
        };

        Ok(metrics)
    }

    pub async fn check_health(&self) -> Result<SystemHealth, NexaError> {
        let metrics = self.collect_metrics(0).await?;
        let config = self.config.read().await;

        let cpu_healthy = metrics.cpu_usage <= config.cpu_threshold / 100.0;
        let memory_healthy = metrics.memory_usage <= config.memory_threshold / 100.0;
        let overall_healthy = cpu_healthy && memory_healthy;

        let health = SystemHealth {
            cpu_healthy,
            memory_healthy,
            overall_healthy,
        };

        *self.health_status.write().await = health.clone();
        Ok(health)
    }

    pub async fn get_config(&self) -> MonitoringConfig {
        self.config.read().await.clone()
    }

    pub async fn update_config(&self, new_config: MonitoringConfig) {
        *self.config.write().await = new_config;
    }

    pub async fn raise_alert(&self, level: AlertLevel, message: String, metadata: HashMap<String, String>) {
        let alert = SystemAlert {
            timestamp: Utc::now(),
            level,
            message,
            metadata,
        };
        self.alerts.write().await.push(alert);
    }

    pub async fn get_recent_alerts(&self, since: DateTime<Utc>) -> Vec<SystemAlert> {
        self.alerts.read().await
            .iter()
            .filter(|alert| alert.timestamp >= since)
            .cloned()
            .collect()
    }

    pub async fn get_metrics(&self, since: DateTime<Utc>) -> Vec<SystemMetrics> {
        self.metrics_history.read().await
            .iter()
            .filter(|metrics| metrics.timestamp >= since)
            .cloned()
            .collect()
    }

    pub async fn allocate(&self, name: String, resource_type: ResourceType, size: usize, metadata: HashMap<String, String>) -> Result<(), NexaError> {
        // Record resource allocation
        let mut resources = self.resources.write().await;
        resources.insert(name.clone(), Resource {
            name,
            value: size as f64,
            unit: match resource_type {
                ResourceType::Memory => "bytes",
                ResourceType::CPU => "cores",
                ResourceType::Tokens => "tokens",
                ResourceType::Network => "bytes/s",
                ResourceType::Storage => "bytes",
            }.to_string(),
        });

        Ok(())
    }
}

impl Default for MonitoringSystem {
    fn default() -> Self {
        let token_manager = Arc::new(TokenManager::new());
        MonitoringSystem::new(token_manager)
    }
}

#[derive(Clone, Debug)]
pub struct SystemStatus {
    pub metrics: SystemMetrics,
    pub health: SystemHealth,
    pub token_usage: TokenMetrics,
    pub alerts: Vec<SystemAlert>,
}

fn get_cpu_usage() -> f64 {
    let mut sys = System::new();
    sys.refresh_cpu_usage();
    sys.cpus().iter().map(|cpu| cpu.cpu_usage() as f64).sum::<f64>() / (sys.cpus().len() as f64 * 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_metrics_collection() {
        let token_manager = Arc::new(TokenManager::new());
        let monitoring = MonitoringSystem::new(token_manager);

        let metrics = monitoring.collect_metrics(1).await.unwrap();
        assert!(metrics.cpu_usage >= 0.0 && metrics.cpu_usage <= 1.0);
        assert!(metrics.memory_usage >= 0.0 && metrics.memory_usage <= 1.0);
    }

    #[tokio::test]
    async fn test_alert_system() {
        let token_manager = Arc::new(TokenManager::new());
        let monitoring = MonitoringSystem::new(token_manager);

        let mut metadata = HashMap::new();
        metadata.insert("test".to_string(), "value".to_string());

        monitoring.raise_alert(
            AlertLevel::Warning,
            "Test alert".to_string(),
            metadata,
        ).await;

        let alerts = monitoring.get_recent_alerts(Utc::now() - ChronoDuration::hours(1)).await;
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].level, AlertLevel::Warning);
    }

    #[tokio::test]
    async fn test_health_check() {
        let token_manager = Arc::new(TokenManager::new());
        
        // Create custom config with high thresholds to ensure test passes
        let config = MonitoringConfig {
            enabled: true,
            detailed_metrics: true,
            metrics_interval: 60,
            history_size: 1000,
            health_check_interval: 30,
            cpu_threshold: 95.0,    // High threshold
            memory_threshold: 95.0,  // High threshold
        };
        
        let monitoring = MonitoringSystem::with_config(token_manager, config);
        let status = monitoring.check_health().await.unwrap();
        assert!(status.overall_healthy, "System should be healthy with high thresholds");
    }

    #[tokio::test]
    async fn test_monitoring_loop() {
        let token_manager = Arc::new(TokenManager::new());
        let monitoring = MonitoringSystem::new(token_manager);

        // Start monitoring in background
        let monitoring_handle = monitoring.clone();
        tokio::spawn(async move {
            monitoring_handle.start_monitoring().await.unwrap();
        });

        // Wait a bit for metrics collection
        tokio::time::sleep(Duration::from_millis(250)).await;

        let metrics = monitoring.get_metrics(Utc::now() - ChronoDuration::minutes(1)).await;
        assert!(!metrics.is_empty());
    }

    #[tokio::test]
    async fn test_resource_allocation() {
        let token_manager = Arc::new(TokenManager::new());
        let monitoring = MonitoringSystem::new(token_manager);

        let mut metadata = HashMap::new();
        metadata.insert("test".to_string(), "value".to_string());

        monitoring.allocate(
            "test_resource".to_string(),
            ResourceType::Memory,
            1024,
            metadata,
        ).await.unwrap();

        let resources = monitoring.resources.read().await;
        let resource = resources.get("test_resource").unwrap();
        assert_eq!(resource.name, "test_resource");
        assert_eq!(resource.value, 1024.0);
        assert_eq!(resource.unit, "bytes");
    }

    #[tokio::test]
    async fn test_config_update() {
        let token_manager = Arc::new(TokenManager::new());
        let monitoring = MonitoringSystem::new(token_manager);

        let new_config = MonitoringConfig {
            enabled: false,
            detailed_metrics: false,
            metrics_interval: 120,
            history_size: 500,
            health_check_interval: 60,
            cpu_threshold: 70.0,
            memory_threshold: 80.0,
        };

        monitoring.update_config(new_config.clone()).await;
        let updated_config = monitoring.get_config().await;
        assert_eq!(updated_config.metrics_interval, new_config.metrics_interval);
        assert_eq!(updated_config.cpu_threshold, new_config.cpu_threshold);
    }
}

impl Clone for MonitoringSystem {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            token_manager: self.token_manager.clone(),
            system: self.system.clone(),
            metrics_history: self.metrics_history.clone(),
            health_status: self.health_status.clone(),
            alerts: self.alerts.clone(),
            resources: self.resources.clone(),
        }
    }
}
