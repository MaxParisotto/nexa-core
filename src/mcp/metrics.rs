use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;
use std::time::{Duration, SystemTime, Instant};
use crate::mcp::buffer::Priority;
use serde::Serialize;
use crate::error::NexaError;

/// Message processing metrics
#[derive(Debug, Clone, Serialize)]
pub struct MessageMetrics {
    /// Total messages processed
    pub total_processed: u64,
    /// Messages processed per priority level
    pub processed_by_priority: HashMap<Priority, u64>,
    /// Average processing time per priority
    pub avg_processing_time: HashMap<Priority, Duration>,
    /// Failed message count
    pub failed_count: u64,
    /// Retry count
    pub retry_count: u64,
    /// Current queue sizes by priority
    pub queue_sizes: HashMap<Priority, usize>,
    /// Messages processed per second
    pub throughput: f64,
    /// Last update timestamp
    pub last_updated: SystemTime,
}

impl Default for MessageMetrics {
    fn default() -> Self {
        let mut processed_by_priority = HashMap::new();
        let mut avg_processing_time = HashMap::new();
        let mut queue_sizes = HashMap::new();

        // Initialize maps for all priority levels
        for priority in [Priority::Low, Priority::Normal, Priority::High, Priority::Critical] {
            processed_by_priority.insert(priority, 0);
            avg_processing_time.insert(priority, Duration::from_secs(0));
            queue_sizes.insert(priority, 0);
        }

        Self {
            total_processed: 0,
            processed_by_priority,
            avg_processing_time,
            failed_count: 0,
            retry_count: 0,
            queue_sizes,
            throughput: 0.0,
            last_updated: SystemTime::now(),
        }
    }
}

/// Metrics collector for message processing
#[derive(Debug)]
pub struct MetricsCollector {
    /// Current metrics
    metrics: Arc<RwLock<MessageMetrics>>,
    /// Processing time samples
    processing_times: Arc<RwLock<HashMap<Priority, Vec<Duration>>>>,
    /// Last throughput calculation time
    last_throughput_calc: Arc<RwLock<Instant>>,
    /// Messages processed since last calculation
    messages_since_last_calc: Arc<RwLock<u64>>,
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(RwLock::new(MessageMetrics::default())),
            processing_times: Arc::new(RwLock::new(HashMap::new())),
            last_throughput_calc: Arc::new(RwLock::new(Instant::now())),
            messages_since_last_calc: Arc::new(RwLock::new(0)),
        }
    }

    /// Record a successful message processing
    pub async fn record_success(&self, priority: Priority, processing_time: Duration) {
        let mut metrics = self.metrics.write().await;
        let mut times = self.processing_times.write().await;
        
        // Update total count
        metrics.total_processed += 1;
        
        // Update priority-specific count
        if let Some(count) = metrics.processed_by_priority.get_mut(&priority) {
            *count += 1;
        }
        
        // Update processing time samples
        times.entry(priority)
            .or_insert_with(Vec::new)
            .push(processing_time);
            
        // Keep only last 100 samples
        if let Some(samples) = times.get_mut(&priority) {
            if samples.len() > 100 {
                samples.remove(0);
            }
            
            // Update average processing time
            let avg = samples.iter().sum::<Duration>() / samples.len() as u32;
            metrics.avg_processing_time.insert(priority, avg);
        }
        
        // Update throughput calculation
        *self.messages_since_last_calc.write().await += 1;
        self.update_throughput().await;
        
        metrics.last_updated = SystemTime::now();
    }

    /// Record a failed message processing
    pub async fn record_failure(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.failed_count += 1;
        metrics.last_updated = SystemTime::now();
    }

    /// Record a message retry
    pub async fn record_retry(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.retry_count += 1;
        metrics.last_updated = SystemTime::now();
    }

    /// Update queue sizes
    pub async fn update_queue_sizes(&self, sizes: HashMap<Priority, usize>) {
        let mut metrics = self.metrics.write().await;
        metrics.queue_sizes = sizes;
        metrics.last_updated = SystemTime::now();
    }

    /// Update throughput calculation
    async fn update_throughput(&self) {
        let mut last_calc = self.last_throughput_calc.write().await;
        let elapsed = last_calc.elapsed();
        
        // Update throughput every second
        if elapsed >= Duration::from_secs(1) {
            let messages = *self.messages_since_last_calc.read().await;
            let mut metrics = self.metrics.write().await;
            
            metrics.throughput = messages as f64 / elapsed.as_secs_f64();
            
            // Reset counters
            *last_calc = Instant::now();
            *self.messages_since_last_calc.write().await = 0;
        }
    }

    /// Get current metrics
    pub async fn get_metrics(&self) -> MessageMetrics {
        self.metrics.read().await.clone()
    }
}

/// Alert thresholds for message processing
#[derive(Debug, Clone)]
pub struct AlertThresholds {
    /// Maximum queue size before warning
    pub queue_size_warning: usize,
    /// Maximum queue size before critical
    pub queue_size_critical: usize,
    /// Maximum processing time before warning (ms)
    pub processing_time_warning_ms: u64,
    /// Maximum processing time before critical (ms)
    pub processing_time_critical_ms: u64,
    /// Minimum throughput before warning
    pub min_throughput_warning: f64,
    /// Maximum error rate before warning (%)
    pub error_rate_warning: f64,
}

impl Default for AlertThresholds {
    fn default() -> Self {
        Self {
            queue_size_warning: 1000,
            queue_size_critical: 5000,
            processing_time_warning_ms: 1000,
            processing_time_critical_ms: 5000,
            min_throughput_warning: 100.0,
            error_rate_warning: 5.0,
        }
    }
}

/// Message processing alert
#[derive(Debug, Clone, Serialize)]
pub struct ProcessingAlert {
    /// Alert message
    pub message: String,
    /// Alert severity
    pub severity: AlertSeverity,
    /// Timestamp
    pub timestamp: SystemTime,
}

/// Alert severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

/// Alert checker for message processing
#[derive(Debug)]
pub struct AlertChecker {
    /// Alert thresholds
    thresholds: AlertThresholds,
    /// Metrics collector
    metrics: Arc<MetricsCollector>,
}

impl AlertChecker {
    pub fn new(thresholds: AlertThresholds, metrics: Arc<MetricsCollector>) -> Self {
        Self {
            thresholds,
            metrics,
        }
    }

    /// Check for alerts based on current metrics
    pub async fn check_alerts(&self) -> Vec<ProcessingAlert> {
        let mut alerts = Vec::new();
        let metrics = self.metrics.get_metrics().await;
        
        // Check queue sizes
        for (priority, size) in metrics.queue_sizes.iter() {
            if *size >= self.thresholds.queue_size_critical {
                alerts.push(ProcessingAlert {
                    message: format!("{:?} priority queue is critically full ({} messages)", priority, size),
                    severity: AlertSeverity::Critical,
                    timestamp: SystemTime::now(),
                });
            } else if *size >= self.thresholds.queue_size_warning {
                alerts.push(ProcessingAlert {
                    message: format!("{:?} priority queue is near capacity ({} messages)", priority, size),
                    severity: AlertSeverity::Warning,
                    timestamp: SystemTime::now(),
                });
            }
        }
        
        // Check processing times
        for (priority, time) in metrics.avg_processing_time.iter() {
            let time_ms = time.as_millis() as u64;
            if time_ms >= self.thresholds.processing_time_critical_ms {
                alerts.push(ProcessingAlert {
                    message: format!("{:?} priority messages are processing very slowly ({} ms)", priority, time_ms),
                    severity: AlertSeverity::Critical,
                    timestamp: SystemTime::now(),
                });
            } else if time_ms >= self.thresholds.processing_time_warning_ms {
                alerts.push(ProcessingAlert {
                    message: format!("{:?} priority messages are processing slowly ({} ms)", priority, time_ms),
                    severity: AlertSeverity::Warning,
                    timestamp: SystemTime::now(),
                });
            }
        }
        
        // Check throughput
        if metrics.throughput < self.thresholds.min_throughput_warning {
            alerts.push(ProcessingAlert {
                message: format!("Message throughput is low ({:.1} msg/s)", metrics.throughput),
                severity: AlertSeverity::Warning,
                timestamp: SystemTime::now(),
            });
        }
        
        // Check error rate
        if metrics.total_processed > 0 {
            let error_rate = (metrics.failed_count as f64 / metrics.total_processed as f64) * 100.0;
            if error_rate > self.thresholds.error_rate_warning {
                alerts.push(ProcessingAlert {
                    message: format!("High message processing error rate ({:.1}%)", error_rate),
                    severity: AlertSeverity::Warning,
                    timestamp: SystemTime::now(),
                });
            }
        }
        
        alerts
    }
}

#[derive(Debug, Clone)]
pub struct MCPMetrics {
    pub active_agents: u32,
    pub total_messages: u64,
    pub processed_messages: u64,
    pub failed_messages: u64,
    pub average_processing_time: f64,
    pub buffer_size: usize,
    pub buffer_capacity: usize,
    pub high_priority_count: usize,
    pub medium_priority_count: usize,
    pub low_priority_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_metrics_collection() {
        let collector = MetricsCollector::new();
        
        // Record some successful processing
        collector.record_success(Priority::High, Duration::from_millis(100)).await;
        collector.record_success(Priority::High, Duration::from_millis(150)).await;
        collector.record_success(Priority::Normal, Duration::from_millis(200)).await;
        
        // Record a failure and retry
        collector.record_failure().await;
        collector.record_retry().await;
        
        // Update queue sizes
        let mut sizes = HashMap::new();
        sizes.insert(Priority::High, 10);
        sizes.insert(Priority::Normal, 20);
        collector.update_queue_sizes(sizes).await;
        
        // Wait for throughput calculation
        sleep(Duration::from_secs(1)).await;
        
        let metrics = collector.get_metrics().await;
        assert_eq!(metrics.total_processed, 3);
        assert_eq!(*metrics.processed_by_priority.get(&Priority::High).unwrap(), 2);
        assert_eq!(metrics.failed_count, 1);
        assert_eq!(metrics.retry_count, 1);
        assert_eq!(*metrics.queue_sizes.get(&Priority::High).unwrap(), 10);
    }

    #[tokio::test]
    async fn test_alert_generation() {
        let collector = Arc::new(MetricsCollector::new());
        let thresholds = AlertThresholds {
            queue_size_warning: 5,
            queue_size_critical: 10,
            processing_time_warning_ms: 50,
            processing_time_critical_ms: 100,
            min_throughput_warning: 10.0,
            error_rate_warning: 1.0,
        };
        
        let checker = AlertChecker::new(thresholds, collector.clone());
        
        // Generate some concerning metrics
        let mut sizes = HashMap::new();
        sizes.insert(Priority::High, 15); // Should trigger critical alert
        collector.update_queue_sizes(sizes).await;
        
        collector.record_success(Priority::High, Duration::from_millis(150)).await; // Should trigger critical alert
        collector.record_failure().await;
        collector.record_failure().await; // Should trigger error rate alert
        
        let alerts = checker.check_alerts().await;
        assert!(!alerts.is_empty());
        
        // Verify we got the expected alerts
        let critical_alerts: Vec<_> = alerts.iter()
            .filter(|a| a.severity == AlertSeverity::Critical)
            .collect();
        assert!(!critical_alerts.is_empty());
    }
} 