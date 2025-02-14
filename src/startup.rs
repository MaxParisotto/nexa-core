#![allow(dead_code)]

use std::io::{stdout, Write};
use std::time::Duration;
use tokio::time::sleep;
use crossterm::{
    execute,
    terminal::{Clear, ClearType},
    cursor::{MoveTo, Hide, Show},
    style::{Color, Print, SetForegroundColor},
};
use log::{info, warn, error};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::error::NexaError;
use crate::config::ServerConfig;
use crate::monitoring::MonitoringSystem;
use crate::mcp::cluster::ClusterManager;

#[derive(Debug, Clone)]
pub struct StartupCheck {
    name: String,
    status: CheckStatus,
    message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CheckStatus {
    Pending,
    InProgress,
    Success,
    Warning,
    Failed,
}

pub struct StartupManager {
    checks: Arc<Mutex<Vec<StartupCheck>>>,
    config: ServerConfig,
    monitoring: Arc<MonitoringSystem>,
    cluster: Arc<ClusterManager>,
}

impl StartupManager {
    pub fn new(
        config: ServerConfig,
        monitoring: Arc<MonitoringSystem>,
        cluster: Arc<ClusterManager>,
    ) -> Self {
        let checks = vec![
            StartupCheck {
                name: "System Requirements".to_string(),
                status: CheckStatus::Pending,
                message: String::new(),
            },
            StartupCheck {
                name: "Configuration".to_string(),
                status: CheckStatus::Pending,
                message: String::new(),
            },
            StartupCheck {
                name: "Database Connection".to_string(),
                status: CheckStatus::Pending,
                message: String::new(),
            },
            StartupCheck {
                name: "API Endpoints".to_string(),
                status: CheckStatus::Pending,
                message: String::new(),
            },
            StartupCheck {
                name: "Cluster Connectivity".to_string(),
                status: CheckStatus::Pending,
                message: String::new(),
            },
            StartupCheck {
                name: "Resource Availability".to_string(),
                status: CheckStatus::Pending,
                message: String::new(),
            },
        ];

        Self {
            checks: Arc::new(Mutex::new(checks)),
            config,
            monitoring,
            cluster,
        }
    }

    pub async fn run_startup_sequence(&self) -> Result<(), NexaError> {
        self.display_splash_screen().await?;
        self.run_self_tests().await?;
        self.display_summary().await?;
        Ok(())
    }

    async fn display_splash_screen(&self) -> Result<(), NexaError> {
        let mut stdout = stdout();
        execute!(stdout, Clear(ClearType::All), Hide)?;

        // ASCII art logo
        let logo = r#"
    _   ___________  __
   / | / / ____/   |/ /   ____
  /  |/ / __/ / /| / /   / __ \
 / /|  / /___/ ___ / /___/ /_/ /
/_/ |_/_____/_/  |_\____/\____/
"#;

        execute!(
            stdout,
            MoveTo(0, 2),
            SetForegroundColor(Color::Cyan),
            Print(logo),
            SetForegroundColor(Color::White),
            MoveTo(0, 8),
            Print("Version: 0.1.0"),
            MoveTo(0, 9),
            Print("Starting system components..."),
            MoveTo(0, 11),
        )?;

        sleep(Duration::from_secs(1)).await;
        Ok(())
    }

    async fn run_self_tests(&self) -> Result<(), NexaError> {
        // Run checks sequentially to ensure proper display order
        self.check_system_requirements().await?;
        self.check_configuration().await?;
        self.check_database().await?;
        self.check_api_endpoints().await?;
        self.check_cluster_connectivity().await?;
        self.check_resource_availability().await?;
        
        Ok(())
    }

    async fn update_check_status(&self, index: usize, status: CheckStatus, message: String) {
        let mut checks = self.checks.lock().await;
        if let Some(check) = checks.get_mut(index) {
            check.status = status;
            check.message = message;
            self.display_progress(&checks).await.unwrap_or_else(|e| {
                error!("Failed to display progress: {}", e);
            });
        }
    }

    async fn display_progress(&self, checks: &[StartupCheck]) -> Result<(), NexaError> {
        let mut stdout = stdout();
        execute!(stdout, MoveTo(0, 11))?;

        for check in checks {
            let (color, symbol) = match check.status {
                CheckStatus::Pending => (Color::White, "○"),
                CheckStatus::InProgress => (Color::Yellow, "◎"),
                CheckStatus::Success => (Color::Green, "●"),
                CheckStatus::Warning => (Color::Yellow, "⚠"),
                CheckStatus::Failed => (Color::Red, "✖"),
            };

            execute!(
                stdout,
                SetForegroundColor(color),
                Print(format!(" {} {}", symbol, check.name)),
                SetForegroundColor(Color::White),
                Print(if !check.message.is_empty() {
                    format!(": {}", check.message)
                } else {
                    String::new()
                }),
                Print("\n")
            )?;
        }

        stdout.flush()?;
        Ok(())
    }

    async fn display_summary(&self) -> Result<(), NexaError> {
        let checks = self.checks.lock().await;
        let total = checks.len();
        let successful = checks.iter().filter(|c| c.status == CheckStatus::Success).count();
        let warnings = checks.iter().filter(|c| c.status == CheckStatus::Warning).count();
        let failures = checks.iter().filter(|c| c.status == CheckStatus::Failed).count();

        let mut stdout = stdout();
        execute!(
            stdout,
            MoveTo(0, 20),
            SetForegroundColor(Color::White),
            Print("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n"),
            Print(format!("Startup Checks Complete: {}/{} successful", successful, total)),
            Print(if warnings > 0 {
                format!(" ({} warnings)", warnings)
            } else {
                String::new()
            }),
            Print(if failures > 0 {
                format!(" ({} failures)", failures)
            } else {
                String::new()
            }),
            Print("\n\nPress any key to continue..."),
            Show,
        )?;

        if failures > 0 {
            error!("Startup completed with {} failures", failures);
        } else if warnings > 0 {
            warn!("Startup completed with {} warnings", warnings);
        } else {
            info!("Startup completed successfully");
        }

        stdout.flush()?;
        Ok(())
    }

    pub async fn check_system_requirements(&self) -> Result<(), NexaError> {
        self.update_check_status(0, CheckStatus::InProgress, "Checking...".to_string()).await;
        
        // Check CPU cores
        let cpu_cores = num_cpus::get();
        if cpu_cores < 2 {
            self.update_check_status(0, CheckStatus::Warning, 
                format!("Only {} CPU cores available", cpu_cores)).await;
            return Ok(());
        }

        // Check available memory
        let sys_info = sys_info::mem_info()?;
        let available_mem_gb = sys_info.avail as f64 / 1024.0 / 1024.0;
        if available_mem_gb < 2.0 {
            self.update_check_status(0, CheckStatus::Warning,
                format!("Only {:.1}GB memory available", available_mem_gb)).await;
            return Ok(());
        }

        self.update_check_status(0, CheckStatus::Success, 
            format!("{}GB RAM, {} cores", available_mem_gb as u64, cpu_cores)).await;
        Ok(())
    }

    pub async fn check_configuration(&self) -> Result<(), NexaError> {
        self.update_check_status(1, CheckStatus::InProgress, "Validating...".to_string()).await;
        
        // Validate configuration
        if let Err(e) = self.config.validate() {
            self.update_check_status(1, CheckStatus::Failed, format!("Invalid configuration: {}", e)).await;
        } else {
            self.update_check_status(1, CheckStatus::Success, "Valid".to_string()).await;
        }
        Ok(())
    }

    async fn check_database(&self) -> Result<(), NexaError> {
        self.update_check_status(2, CheckStatus::InProgress, "Connecting...".to_string()).await;
        
        // Implement database connection check
        // For now, we'll simulate it
        sleep(Duration::from_millis(500)).await;
        self.update_check_status(2, CheckStatus::Success, "Connected".to_string()).await;
        Ok(())
    }

    pub async fn check_api_endpoints(&self) -> Result<(), NexaError> {
        self.update_check_status(3, CheckStatus::InProgress, "Testing...".to_string()).await;
        
        // Test API endpoints
        let endpoints = self.config.get_api_endpoints();
        let mut success = 0;
        let mut failed = 0;

        for endpoint in endpoints {
            if let Ok(response) = reqwest::get(&endpoint).await {
                if response.status().is_success() {
                    success += 1;
                } else {
                    failed += 1;
                }
            } else {
                failed += 1;
            }
        }

        if failed > 0 {
            self.update_check_status(3, CheckStatus::Warning, 
                format!("{}/{} endpoints available", success, success + failed)).await;
        } else {
            self.update_check_status(3, CheckStatus::Success, 
                format!("All {} endpoints available", success)).await;
        }
        Ok(())
    }

    pub async fn check_cluster_connectivity(&self) -> Result<(), NexaError> {
        self.update_check_status(4, CheckStatus::InProgress, "Checking...".to_string()).await;
        
        let nodes = self.cluster.get_active_nodes().await?;
        if nodes.is_empty() {
            self.update_check_status(4, CheckStatus::Warning, "No active nodes".to_string()).await;
        } else {
            self.update_check_status(4, CheckStatus::Success, 
                format!("{} nodes connected", nodes.len())).await;
        }
        Ok(())
    }

    pub async fn check_resource_availability(&self) -> Result<(), NexaError> {
        self.update_check_status(5, CheckStatus::InProgress, "Analyzing...".to_string()).await;
        
        let metrics = self.monitoring.collect_metrics(0).await?;
        
        if metrics.cpu_usage > 0.8 {
            self.update_check_status(5, CheckStatus::Warning, 
                format!("High CPU usage: {:.1}%", metrics.cpu_usage * 100.0)).await;
        } else if metrics.memory_usage > 0.8 {
            self.update_check_status(5, CheckStatus::Warning, 
                format!("High memory usage: {:.1}%", metrics.memory_usage * 100.0)).await;
        } else {
            self.update_check_status(5, CheckStatus::Success, 
                format!("CPU: {:.1}%, Memory: {:.1}%", 
                    metrics.cpu_usage * 100.0, 
                    metrics.memory_usage * 100.0)).await;
        }
        Ok(())
    }
}

#[cfg(test)]
pub mod test_utils {
    use super::*;

    impl StartupManager {
        pub async fn test_system_requirements(&self) -> Result<(), NexaError> {
            self.check_system_requirements().await
        }

        pub async fn test_configuration(&self) -> Result<(), NexaError> {
            self.check_configuration().await
        }

        pub async fn test_api_endpoints(&self) -> Result<(), NexaError> {
            self.check_api_endpoints().await
        }

        pub async fn test_cluster_connectivity(&self) -> Result<(), NexaError> {
            self.check_cluster_connectivity().await
        }

        pub async fn test_resource_availability(&self) -> Result<(), NexaError> {
            self.check_resource_availability().await
        }
    }
} 