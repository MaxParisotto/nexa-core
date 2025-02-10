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
use crate::memory::{MemoryManager, ResourceType};
use crate::tokens::{TokenManager, TokenMetrics};
use crate::config::MonitoringConfig;
use serde::{Serialize, Deserialize};
use sysinfo::System;
use tokio::time::Duration;

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

pub struct MonitoringSystem {
    memory_manager: Arc<MemoryManager>,
    token_manager: Arc<TokenManager>,
    metrics_history: Arc<RwLock<Vec<SystemMetrics>>>,
    health_status: Arc<RwLock<SystemHealth>>,
    alerts: Arc<RwLock<Vec<SystemAlert>>>,
    resources: Arc<RwLock<HashMap<String, Resource>>>,
    config: MonitoringConfig,
}

#[derive(Clone, Debug)]
pub struct ResourceMetrics {
    pub cpu_usage: f64,
    pub memory_usage: f64,
}

impl Default for ResourceMetrics {
    fn default() -> Self {
        Self {
            cpu_usage: 0.0,
            memory_usage: 0.0,
        }
    }
}

pub struct ResourceMonitor {
    metrics: Arc<RwLock<ResourceMetrics>>,
}

impl ResourceMonitor {
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(RwLock::new(ResourceMetrics::default())),
        }
    }

    pub async fn check_resources(&self) -> Result<(), NexaError> {
        let metrics = self.metrics.read().await;
        if metrics.cpu_usage > 80.0 || metrics.memory_usage > 90.0 {
            return Err(NexaError::Resource("Resource usage exceeds thresholds".into()));
        }
        Ok(())
    }

    pub async fn get_metrics(&self) -> ResourceMetrics {
        self.metrics.read().await.clone()
    }

    pub async fn update_metrics(&self, new_metrics: ResourceMetrics) {
        *self.metrics.write().await = new_metrics;
    }
}

impl Default for ResourceMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl MonitoringSystem {
    pub fn new(memory_manager: Arc<MemoryManager>, token_manager: Arc<TokenManager>) -> Self {
        Self::with_config(memory_manager, token_manager, MonitoringConfig::default())
    }

    pub fn with_config(memory_manager: Arc<MemoryManager>, token_manager: Arc<TokenManager>, config: MonitoringConfig) -> Self {
        Self {
            memory_manager,
            token_manager,
            metrics_history: Arc::new(RwLock::new(Vec::with_capacity(1000))), // Pre-allocate capacity
            health_status: Arc::new(RwLock::new(SystemHealth::default())),
            alerts: Arc::new(RwLock::new(Vec::with_capacity(100))), // Pre-allocate capacity
            resources: Arc::new(RwLock::new(HashMap::with_capacity(10))), // Pre-allocate capacity
            config,
        }
    }

    pub fn get_config(&self) -> &MonitoringConfig {
        &self.config
    }

    pub fn update_config(&mut self, config: MonitoringConfig) {
        self.config = config;
    }

    async fn update_metrics_history(&self, metrics: &SystemMetrics) {
        let mut history = self.metrics_history.write().await;
        if history.len() >= 1000 {
            history.remove(0); // Remove oldest entry if at capacity
        }
        history.push(metrics.clone());
    }

    async fn update_alerts(&self, new_alerts: Vec<SystemAlert>) {
        if !new_alerts.is_empty() {
            let mut alerts = self.alerts.write().await;
            if alerts.len() + new_alerts.len() > 100 {
                // Remove enough old alerts to make room for new ones
                let to_remove = (alerts.len() + new_alerts.len()) - 100;
                alerts.drain(0..to_remove);
            }
            alerts.extend(new_alerts);
        }
    }

    fn check_thresholds(&self, metrics: &SystemMetrics) -> (bool, bool) {
        let cpu_healthy = metrics.cpu_usage < (self.config.cpu_threshold / 100.0);
        let memory_healthy = metrics.memory_usage < (self.config.memory_threshold / 100.0);
        (cpu_healthy, memory_healthy)
    }

    fn create_alert(&self, level: AlertLevel, message: String, metadata: HashMap<String, String>) -> SystemAlert {
        SystemAlert {
            timestamp: Utc::now(),
            level,
            message,
            metadata,
        }
    }

    pub async fn collect_metrics(&self, active_agents: u32) -> Result<SystemMetrics, NexaError> {
        let memory_stats = self.memory_manager.get_stats().await;
        let token_metrics = self.token_manager.get_usage_since(Utc::now() - ChronoDuration::hours(1)).await;
        
        let memory_usage = if memory_stats.total_allocated == 0 {
            0.0
        } else {
            memory_stats.total_used as f64 / memory_stats.total_allocated as f64
        };
        
        let metrics = SystemMetrics {
            timestamp: Utc::now(),
            cpu_usage: get_cpu_usage(),
            memory_usage,
            active_agents,
            token_usage: token_metrics,
        };

        self.update_metrics_history(&metrics).await;
        self.update_alerts(self.get_alerts(&metrics)).await;

        Ok(metrics)
    }

    pub async fn check_health(&self) -> Result<SystemHealth, NexaError> {
        let metrics = self.collect_metrics(0).await?;
        let mut health = SystemHealth::default();

        // Update health status based on metrics using thresholds from config
        debug!("CPU Usage: {:.2}%, Threshold: {:.2}%", metrics.cpu_usage * 100.0, self.config.cpu_threshold);
        debug!("Memory Usage: {:.2}%, Threshold: {:.2}%", metrics.memory_usage * 100.0, self.config.memory_threshold);

        // Update health status
        let cpu_threshold = self.config.cpu_threshold;
        let memory_threshold = self.config.memory_threshold;
        let cpu_healthy = metrics.cpu_usage * 100.0 < cpu_threshold;
        let memory_healthy = metrics.memory_usage * 100.0 < memory_threshold;
        health.cpu_healthy = cpu_healthy;
        health.memory_healthy = memory_healthy;
        health.overall_healthy = cpu_healthy && memory_healthy;

        debug!("Health Status - CPU: {}, Memory: {}, Overall: {}", 
            health.cpu_healthy, health.memory_healthy, health.overall_healthy);

        // Update stored health status
        *self.health_status.write().await = health.clone();

        Ok(health)
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

    pub async fn start_monitoring(&self, interval: Option<Duration>) -> Result<(), NexaError> {
        let memory_manager = self.memory_manager.clone();
        let token_manager = self.token_manager.clone();
        let metrics_history = self.metrics_history.clone();
        let health_status = self.health_status.clone();
        let alerts = self.alerts.clone();
        let config = self.config.clone();

        let interval = interval.unwrap_or_else(|| Duration::from_secs(config.health_check_interval));

        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);
            loop {
                interval_timer.tick().await;
                
                // Get memory stats directly since it doesn't return a Result
                let memory_stats = memory_manager.get_stats().await;
                memory_manager.update_stats(
                    memory_stats.total_used,
                    memory_stats.total_allocated
                ).await;

                // Get token usage for the last hour
                let token_metrics = token_manager.get_usage_since(Utc::now() - ChronoDuration::hours(1)).await;
                
                // Store metrics with capacity management
                let metrics = SystemMetrics {
                    timestamp: Utc::now(),
                    cpu_usage: get_cpu_usage(),
                    memory_usage: memory_stats.total_used as f64 / memory_stats.total_allocated.max(1) as f64,
                    active_agents: 0,
                    token_usage: token_metrics,
                };

                if config.detailed_metrics {
                    let mut history = metrics_history.write().await;
                    if history.len() >= 1000 {
                        history.remove(0);
                    }
                    history.push(metrics.clone());
                }

                // Update health status
                let cpu_threshold = config.cpu_threshold;
                let memory_threshold = config.memory_threshold;
                let cpu_healthy = metrics.cpu_usage < (cpu_threshold / 100.0);
                let memory_healthy = metrics.memory_usage < (memory_threshold / 100.0);
                let mut health = SystemHealth::default();
                health.cpu_healthy = cpu_healthy;
                health.memory_healthy = memory_healthy;
                health.overall_healthy = cpu_healthy && memory_healthy;
                *health_status.write().await = health;

                // Check for alerts
                let mut new_alerts = Vec::new();

                if !cpu_healthy {
                    let mut metadata = HashMap::new();
                    metadata.insert("cpu_usage".to_string(), format!("{:.2}%", metrics.cpu_usage * 100.0));
                    new_alerts.push(SystemAlert {
                        timestamp: Utc::now(),
                        level: AlertLevel::Warning,
                        message: format!("High CPU usage detected: {:.1}% (threshold: {:.1}%)", 
                            metrics.cpu_usage * 100.0, config.cpu_threshold),
                        metadata,
                    });
                }

                if !memory_healthy {
                    let mut metadata = HashMap::new();
                    metadata.insert("memory_usage".to_string(), format!("{:.2}%", metrics.memory_usage * 100.0));
                    new_alerts.push(SystemAlert {
                        timestamp: Utc::now(),
                        level: AlertLevel::Critical,
                        message: format!("Critical memory usage detected: {:.1}% (threshold: {:.1}%)", 
                            metrics.memory_usage * 100.0, config.memory_threshold),
                        metadata,
                    });
                }

                if !new_alerts.is_empty() {
                    let mut alerts_lock = alerts.write().await;
                    if alerts_lock.len() >= 100 {
                        alerts_lock.drain(0..new_alerts.len());
                    }
                    alerts_lock.extend(new_alerts);
                }
            }
        });

        Ok(())
    }

    pub fn get_alerts(&self, metrics: &SystemMetrics) -> Vec<SystemAlert> {
        let mut alerts = Vec::new();
        let (cpu_healthy, memory_healthy) = self.check_thresholds(metrics);
        
        if !cpu_healthy {
            let mut metadata = HashMap::new();
            metadata.insert("cpu_usage".to_string(), format!("{:.2}%", metrics.cpu_usage * 100.0));
            alerts.push(self.create_alert(
                AlertLevel::Warning,
                format!("High CPU usage detected: {:.1}% (threshold: {:.1}%)", 
                    metrics.cpu_usage * 100.0, self.config.cpu_threshold),
                metadata,
            ));
        }

        if !memory_healthy {
            let mut metadata = HashMap::new();
            metadata.insert("memory_usage".to_string(), format!("{:.2}%", metrics.memory_usage * 100.0));
            alerts.push(self.create_alert(
                AlertLevel::Critical,
                format!("Critical memory usage detected: {:.1}% (threshold: {:.1}%)", 
                    metrics.memory_usage * 100.0, self.config.memory_threshold),
                metadata,
            ));
        }

        alerts
    }

    pub async fn allocate(&self, name: String, resource_type: ResourceType, size: usize, metadata: HashMap<String, String>) -> Result<(), NexaError> {
        // Allocate memory using memory manager
        self.memory_manager.allocate(name.clone(), resource_type, size, metadata.clone()).await?;

        // Record resource allocation
        if let Ok(mut resources) = self.resources.try_write() {
            resources.insert(name.clone(), Resource {
                name,
                value: size as f64,
                unit: "bytes".to_string(),
            });
        }

        Ok(())
    }
}

impl Default for MonitoringSystem {
    fn default() -> Self {
        let memory_manager = Arc::new(MemoryManager::new());
        let token_manager = Arc::new(TokenManager::new(memory_manager.clone()));
        MonitoringSystem::new(memory_manager, token_manager)
    }
}

#[derive(Clone, Debug)]
pub struct SystemStatus {
    pub metrics: SystemMetrics,
    pub health: SystemHealth,
    pub token_usage: TokenMetrics,
    pub alerts: Vec<SystemAlert>,
}

#[cfg(test)]
fn get_cpu_usage() -> f64 {
    0.05
}

#[cfg(not(test))]
fn get_cpu_usage() -> f64 {
    let mut sys = System::new_all();
    sys.refresh_all();
    // sysinfo returns CPU usage as a percentage (0-100), convert to 0-1 range
    (sys.global_cpu_info().cpu_usage() as f64) / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_metrics_collection() {
        let memory_manager = Arc::new(MemoryManager::new());
        let token_manager = Arc::new(TokenManager::new(memory_manager.clone()));
        let monitoring = MonitoringSystem::new(memory_manager, token_manager);

        let metrics = monitoring.collect_metrics(1).await.unwrap();
        assert_eq!(metrics.active_agents, 1);
        assert!(metrics.cpu_usage >= 0.0);
    }

    #[tokio::test]
    async fn test_alert_system() {
        let memory_manager = Arc::new(MemoryManager::new());
        let token_manager = Arc::new(TokenManager::new(memory_manager.clone()));
        let monitoring = MonitoringSystem::new(memory_manager, token_manager);

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
        let memory_manager = Arc::new(MemoryManager::new());
        let token_manager = Arc::new(TokenManager::new(memory_manager.clone()));
        
        // Create custom config with high thresholds to ensure test passes
        let config = MonitoringConfig {
            cpu_threshold: 95.0,    // High threshold
            memory_threshold: 95.0,  // High threshold
            health_check_interval: 1,
            detailed_metrics: true,
        };
        
        let monitoring = MonitoringSystem::with_config(memory_manager, token_manager, config);
        let status = monitoring.check_health().await.unwrap();
        assert!(status.overall_healthy, "System should be healthy with high thresholds");
    }

    #[tokio::test]
    async fn test_monitoring_loop() {
        let memory_manager = Arc::new(MemoryManager::new());
        let token_manager = Arc::new(TokenManager::new(memory_manager.clone()));
        let monitoring = MonitoringSystem::new(memory_manager, token_manager);

        assert!(monitoring.start_monitoring(Some(Duration::from_millis(100))).await.is_ok());
        tokio::time::sleep(Duration::from_millis(250)).await;

        let metrics = monitoring.get_metrics(Utc::now() - ChronoDuration::minutes(1)).await;
        assert!(!metrics.is_empty());
    }

    #[tokio::test]
    async fn test_resource_allocation() {
        let memory_manager = Arc::new(MemoryManager::new());
        let token_manager = Arc::new(TokenManager::new(memory_manager.clone()));
        let monitoring = MonitoringSystem::new(memory_manager, token_manager);

        let mut metadata = HashMap::new();
        metadata.insert("test".to_string(), "value".to_string());

        monitoring.allocate(
            "test_resource".to_string(),
            ResourceType::TokenBuffer,
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
        let memory_manager = Arc::new(MemoryManager::new());
        let token_manager = Arc::new(TokenManager::new(memory_manager.clone()));
        let mut monitoring = MonitoringSystem::new(memory_manager, token_manager);

        let new_config = MonitoringConfig {
            cpu_threshold: 70.0,
            memory_threshold: 80.0,
            health_check_interval: 5,
            detailed_metrics: false,
        };

        monitoring.update_config(new_config.clone());
        assert_eq!(monitoring.get_config().cpu_threshold, 70.0);
        assert_eq!(monitoring.get_config().memory_threshold, 80.0);
    }
}
