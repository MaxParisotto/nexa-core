//! Monitoring System
//! 
//! This module provides real-time monitoring and metrics tracking:
//! - Resource utilization monitoring
//! - Performance metrics
//! - Health checks
//! - Alert system
//! - Metrics aggregation

use log::{error, info, debug};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use crate::error::NexaError;
use crate::memory::MemoryManager;
use crate::tokens::{TokenManager, TokenMetrics};
use serde::{Serialize, Deserialize};
use sysinfo::System;
use std::time::SystemTime;
use tokio::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub cpu_usage: f64,
    pub memory_used: usize,
    pub memory_allocated: usize,
    pub memory_available: usize,
    pub token_usage: usize,
    pub token_cost: f64,
    pub active_agents: u32,
    pub error_count: usize,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl Default for SystemMetrics {
    fn default() -> Self {
        Self {
            cpu_usage: 0.0,
            memory_used: 0,
            memory_allocated: 0,
            memory_available: 0,
            token_usage: 0,
            token_cost: 0.0,
            active_agents: 0,
            error_count: 0,
            timestamp: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealth {
    pub is_healthy: bool,
    pub message: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemAlert {
    pub level: AlertLevel,
    pub message: String,
    pub timestamp: DateTime<Utc>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResourceType {
    Memory,
    CPU,
    Network,
    Storage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    pub name: String,
    pub resource_type: ResourceType,
    pub size: usize,
    pub allocated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct MonitoringSystem {
    memory_manager: Arc<MemoryManager>,
    token_manager: Arc<TokenManager>,
    cpu_threshold: f64,
    memory_threshold: f64,
    metrics_history: Arc<RwLock<Vec<SystemMetrics>>>,
    health_status: Arc<RwLock<SystemHealth>>,
    alerts: Arc<RwLock<Vec<SystemAlert>>>,
    resources: Arc<RwLock<HashMap<String, Resource>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStatus {
    pub metrics: SystemMetrics,
    pub health: SystemHealth,
    pub token_usage: TokenMetrics,
    pub alerts: Vec<SystemAlert>,
}

impl MonitoringSystem {
    pub fn new(memory_manager: Arc<MemoryManager>, token_manager: Arc<TokenManager>) -> Self {
        let system = Self {
            memory_manager,
            token_manager,
            cpu_threshold: 80.0,
            memory_threshold: 90.0,
            metrics_history: Arc::new(RwLock::new(Vec::new())),
            health_status: Arc::new(RwLock::new(SystemHealth {
                is_healthy: true,
                message: "System initializing".to_string(),
                timestamp: Utc::now(),
            })),
            alerts: Arc::new(RwLock::new(Vec::new())),
            resources: Arc::new(RwLock::new(HashMap::new())),
        };
        
        debug!("Initialized monitoring system with thresholds - CPU: {}, Memory: {}", 
            system.cpu_threshold, system.memory_threshold);
        
        system
    }

    /// Collect current system metrics
    pub async fn collect_metrics(&self, active_agents: u32) -> Result<SystemMetrics, NexaError> {
        let memory_usage = self.memory_manager.get_stats().await;
        let token_usage = self.token_manager.get_usage_since(
            Utc::now() - chrono::Duration::hours(1)
        ).await;

        // Get system metrics using sysinfo
        let mut sys = System::new_all();
        sys.refresh_all();

        // Get CPU usage (average across all cores)
        let cpu_usage = sys.global_cpu_info().cpu_usage();

        let metrics = SystemMetrics {
            cpu_usage: cpu_usage as f64,
            memory_used: memory_usage.total_used,
            memory_allocated: memory_usage.total_allocated,
            memory_available: memory_usage.available,
            token_usage: token_usage.total_tokens,
            token_cost: token_usage.cost,
            active_agents,
            error_count: 0,
            timestamp: Utc::now(),
        };

        // Store metrics
        let mut history = self.metrics_history.write().await;
        history.push(metrics.clone());

        // Cleanup old metrics (keep last 24 hours)
        let day_ago = Utc::now() - chrono::Duration::days(1);
        history.retain(|m| m.timestamp > day_ago);

        Ok(metrics)
    }

    /// Check system health
    pub async fn check_health(&self) -> Result<SystemHealth, NexaError> {
        let metrics = self.collect_metrics(0).await?;
        
        // Calculate memory percentage safely
        let memory_percentage = if metrics.memory_allocated == 0 {
            0.0
        } else {
            metrics.memory_used as f64 / metrics.memory_allocated as f64 * 100.0
        };

        let is_healthy = metrics.cpu_usage <= self.cpu_threshold && 
                        memory_percentage <= self.memory_threshold;
        
        let message = if is_healthy {
            "System healthy".to_string()
        } else {
            format!(
                "System under stress - CPU: {:.1}%, Memory: {:.1}%",
                metrics.cpu_usage,
                memory_percentage
            )
        };

        debug!(
            "Health check metrics - CPU: {:.1}% (threshold: {:.1}%), Memory: {:.1}% (threshold: {:.1}%)",
            metrics.cpu_usage,
            self.cpu_threshold,
            memory_percentage,
            self.memory_threshold
        );

        let health = SystemHealth {
            is_healthy,
            message: message.clone(),
            timestamp: Utc::now(),
        };

        *self.health_status.write().await = health.clone();

        Ok(health)
    }

    /// Raise an alert
    pub async fn raise_alert(&self, level: AlertLevel, message: String, _metadata: HashMap<String, String>) {
        let alert = SystemAlert {
            level,
            message,
            timestamp: chrono::Utc::now(),
        };
        
        let mut alerts = self.alerts.write().await;
        alerts.push(alert);
        
        // Keep only the last 100 alerts
        if alerts.len() > 100 {
            alerts.remove(0);
        }
    }

    /// Get recent alerts
    pub async fn get_recent_alerts(&self, since: DateTime<Utc>) -> Vec<SystemAlert> {
        let alerts = self.alerts.read().await;
        alerts
            .iter()
            .filter(|a| a.timestamp >= since)
            .cloned()
            .collect()
    }

    /// Get metrics for a time period
    pub async fn get_metrics(&self, since: DateTime<Utc>) -> Vec<SystemMetrics> {
        let metrics = self.metrics_history.read().await;
        metrics
            .iter()
            .filter(|m| m.timestamp >= since)
            .cloned()
            .collect()
    }

    /// Start background monitoring
    pub async fn start_monitoring(&self, interval: Duration) -> Result<(), NexaError> {
        let metrics_history = self.metrics_history.clone();
        let health_status = self.health_status.clone();
        let alerts = self.alerts.clone();
        let memory_manager = self.memory_manager.clone();
        let token_manager = self.token_manager.clone();

        tokio::spawn(async move {
            let monitor = MonitoringSystem {
                cpu_threshold: 80.0,
                memory_threshold: 80.0,
                memory_manager,
                token_manager,
                metrics_history,
                health_status,
                alerts,
                resources: Arc::new(RwLock::new(HashMap::new())),
            };

            loop {
                if let Err(e) = monitor.check_health().await {
                    let mut metadata = HashMap::new();
                    metadata.insert("error".to_string(), e.to_string());
                    monitor.raise_alert(
                        AlertLevel::Error,
                        "Health check failed".to_string(),
                        metadata,
                    ).await;
                }
                tokio::time::sleep(interval).await;
            }
        });

        Ok(())
    }

    pub fn get_alerts(&self, metrics: &SystemMetrics) -> Vec<SystemAlert> {
        let mut alerts = Vec::new();

        // Check CPU usage
        if metrics.cpu_usage > self.cpu_threshold {
            alerts.push(SystemAlert {
                level: AlertLevel::Critical,
                message: format!("CPU usage critical: {:.1}%", metrics.cpu_usage),
                timestamp: Utc::now(),
            });
        } else if metrics.cpu_usage > self.cpu_threshold * 0.8 {
            alerts.push(SystemAlert {
                level: AlertLevel::Warning,
                message: format!("CPU usage high: {:.1}%", metrics.cpu_usage),
                timestamp: Utc::now(),
            });
        }

        // Check memory usage
        let memory_usage_percent = (metrics.memory_used as f64 / metrics.memory_allocated as f64) * 100.0;
        if memory_usage_percent > self.memory_threshold {
            alerts.push(SystemAlert {
                level: AlertLevel::Critical,
                message: format!("Memory usage critical: {:.1}%", memory_usage_percent),
                timestamp: Utc::now(),
            });
        } else if memory_usage_percent > self.memory_threshold * 0.8 {
            alerts.push(SystemAlert {
                level: AlertLevel::Warning,
                message: format!("Memory usage high: {:.1}%", memory_usage_percent),
                timestamp: Utc::now(),
            });
        }

        // Check error count
        if metrics.error_count > 0 {
            alerts.push(SystemAlert {
                level: AlertLevel::Error,
                message: format!("System has {} errors", metrics.error_count),
                timestamp: Utc::now(),
            });
        }

        alerts
    }

    /// Set CPU usage threshold (percentage)
    pub fn set_cpu_threshold(&mut self, threshold: f64) {
        debug!("Setting CPU threshold to {}", threshold);
        self.cpu_threshold = threshold;
    }

    /// Set memory usage threshold (percentage)
    pub fn set_memory_threshold(&mut self, threshold: f64) {
        debug!("Setting memory threshold to {}", threshold);
        self.memory_threshold = threshold;
    }

    pub async fn allocate(&self, name: String, resource_type: ResourceType, size: usize, _metadata: HashMap<String, String>) {
        let mut resources = self.resources.write().await;
        resources.insert(name.clone(), Resource {
            name,
            resource_type,
            size,
            allocated_at: Utc::now(),
        });
    }
}

impl Default for MonitoringSystem {
    fn default() -> Self {
        Self::new(Arc::new(MemoryManager::new()), Arc::new(TokenManager::new(Arc::new(MemoryManager::new()))))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceMetrics {
    pub memory_usage_mb: f64,
    pub cpu_usage_percent: f64,
    pub error_rate: f64,
    pub active_connections: u32,
    pub requests_per_second: f64,
}

pub struct ResourceMonitor {
    metrics: Arc<RwLock<ResourceMetrics>>,
}

impl ResourceMonitor {
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(RwLock::new(ResourceMetrics {
                memory_usage_mb: 0.0,
                cpu_usage_percent: 0.0,
                error_rate: 0.0,
                active_connections: 0,
                requests_per_second: 0.0,
            })),
        }
    }

    pub async fn check_resources(&self) -> Result<(), NexaError> {
        let metrics = self.metrics.read().await;
        
        if metrics.memory_usage_mb > 90.0 {
            return Err(NexaError::System("Memory usage too high".to_string()));
        }

        if metrics.cpu_usage_percent > 80.0 {
            return Err(NexaError::System("CPU usage too high".to_string()));
        }

        if metrics.error_rate > 0.1 {
            return Err(NexaError::System("Error rate too high".to_string()));
        }

        Ok(())
    }

    pub async fn get_metrics(&self) -> Vec<String> {
        let metrics = self.metrics.read().await;
        vec![
            format!("Memory Usage: {:.2} MB", metrics.memory_usage_mb),
            format!("CPU Usage: {:.2}%", metrics.cpu_usage_percent),
            format!("Error Rate: {:.4}", metrics.error_rate),
            format!("Active Connections: {}", metrics.active_connections),
            format!("Requests/sec: {:.2}", metrics.requests_per_second),
        ]
    }

    pub async fn update_metrics(&self, new_metrics: ResourceMetrics) {
        let mut metrics = self.metrics.write().await;
        *metrics = new_metrics;
    }
}

impl Default for ResourceMonitor {
    fn default() -> Self {
        Self::new()
    }
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

        let alerts = monitoring.get_recent_alerts(Utc::now() - chrono::Duration::hours(1)).await;
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].level, AlertLevel::Warning);
    }

    #[tokio::test]
    async fn test_health_check() {
        // Enable debug logging
        let _subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
            .with_thread_ids(true)
            .with_file(true)
            .with_line_number(true)
            .try_init();

        let memory_manager = Arc::new(MemoryManager::new());
        let token_manager = Arc::new(TokenManager::new(memory_manager.clone()));
        let monitoring = MonitoringSystem::new(memory_manager, token_manager);

        // Start monitoring
        monitoring.start_monitoring(Duration::from_secs(1)).await.unwrap();

        // Wait for some data collection
        tokio::time::sleep(Duration::from_secs(2)).await;

        let status = monitoring.check_health().await.unwrap();
        assert!(status.is_healthy, "System should be healthy after initialization");
    }

    #[tokio::test]
    async fn test_monitoring_loop() {
        let memory_manager = Arc::new(MemoryManager::new());
        let token_manager = Arc::new(TokenManager::new(memory_manager.clone()));
        let monitoring = MonitoringSystem::new(memory_manager, token_manager);

        assert!(monitoring.start_monitoring(Duration::from_millis(100)).await.is_ok());
        tokio::time::sleep(Duration::from_millis(250)).await;

        let metrics = monitoring.get_metrics(Utc::now() - chrono::Duration::minutes(1)).await;
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
            ResourceType::Memory,
            1024,
            metadata,
        ).await;

        let resources = monitoring.resources.read().await;
        let resource = resources.get("test_resource").unwrap();
        assert_eq!(resource.name, "test_resource");
        assert!(matches!(resource.resource_type, ResourceType::Memory));
        assert_eq!(resource.size, 1024);
    }
}
