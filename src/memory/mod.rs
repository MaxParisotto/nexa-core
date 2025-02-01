//! Memory Management System
//! 
//! This module provides memory tracking and management capabilities:
//! - Memory usage monitoring
//! - Resource allocation tracking
//! - Memory limits enforcement
//! - Cache management
//! - Resource pooling

use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;
use crate::error::NexaError;
use serde::{Serialize, Deserialize};

/// Memory usage statistics
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct MemoryStats {
    pub total_used: usize,
    pub total_allocated: usize,
    pub peak_usage: usize,
    pub allocation_count: usize,
    pub available: usize,
}

impl Default for MemoryStats {
    fn default() -> Self {
        Self {
            total_used: 0,
            total_allocated: 0,
            peak_usage: 0,
            allocation_count: 0,
            available: 0,
        }
    }
}

/// Memory resource types that can be tracked
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum ResourceType {
    TokenBuffer,
    Cache,
    Context,
    Model,
    Custom(String),
}

/// Memory allocation record
#[derive(Debug, Clone)]
pub struct AllocationRecord {
    pub resource_type: ResourceType,
    pub size: usize,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub metadata: HashMap<String, String>,
}

/// Memory Manager for tracking and controlling memory usage
#[derive(Debug)]
pub struct MemoryManager {
    stats: Arc<RwLock<MemoryStats>>,
    allocations: Arc<RwLock<HashMap<String, AllocationRecord>>>,
    limits: HashMap<ResourceType, usize>,
}

impl MemoryManager {
    pub fn new() -> Self {
        Self {
            stats: Arc::new(RwLock::new(MemoryStats::default())),
            allocations: Arc::new(RwLock::new(HashMap::new())),
            limits: HashMap::new(),
        }
    }

    /// Set memory limit for a resource type
    pub fn set_limit(&mut self, resource_type: ResourceType, limit: usize) {
        self.limits.insert(resource_type, limit);
    }

    /// Request memory allocation
    pub async fn allocate(
        &self,
        id: String,
        resource_type: ResourceType,
        size: usize,
        metadata: HashMap<String, String>,
    ) -> Result<(), NexaError> {
        // Check resource limits
        if let Some(limit) = self.limits.get(&resource_type) {
            if size > *limit {
                return Err(NexaError::system(format!(
                    "Memory allocation exceeds limit for {:?}: {} > {}",
                    resource_type, size, limit
                )));
            }
        }

        let mut stats = self.stats.write().await;
        let mut allocations = self.allocations.write().await;

        // Add 20% overhead for memory management
        let allocation_size = size + (size / 5);

        // Update stats
        stats.total_allocated += allocation_size;
        stats.total_used += size;
        stats.allocation_count += 1;
        stats.peak_usage = stats.peak_usage.max(stats.total_used);
        stats.available = stats.total_allocated.saturating_sub(stats.total_used);

        // Record allocation
        allocations.insert(id, AllocationRecord {
            resource_type,
            size,
            timestamp: chrono::Utc::now(),
            metadata,
        });

        Ok(())
    }

    /// Release allocated memory
    pub async fn deallocate(&self, id: &str) -> Result<(), NexaError> {
        let mut stats = self.stats.write().await;
        let mut allocations = self.allocations.write().await;

        if let Some(record) = allocations.remove(id) {
            let allocation_size = record.size + (record.size / 5);
            stats.total_allocated -= allocation_size;
            stats.total_used -= record.size;
            stats.available = stats.total_allocated.saturating_sub(stats.total_used);
            Ok(())
        } else {
            Err(NexaError::system(format!("No allocation found for id: {}", id)))
        }
    }

    /// Get current memory statistics
    pub async fn get_stats(&self) -> MemoryStats {
        let stats = self.stats.read().await;
        stats.clone()
    }

    /// Get allocation records
    pub async fn get_allocations(&self) -> HashMap<String, AllocationRecord> {
        self.allocations.read().await.clone()
    }

    pub async fn update_stats(&self, used: usize, allocated: usize) {
        let mut stats = self.stats.write().await;
        stats.total_used = used;
        stats.total_allocated = allocated;
        stats.peak_usage = stats.peak_usage.max(used);
        stats.allocation_count += 1;
        stats.available = allocated.saturating_sub(used);
    }
}

impl Default for MemoryManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_allocation() {
        let manager = MemoryManager::new();
        let metadata = HashMap::new();
        let size = 1024;
        let overhead = size / 5; // 20% overhead

        assert!(manager
            .allocate(
                "test-1".to_string(),
                ResourceType::TokenBuffer,
                size,
                metadata.clone()
            )
            .await
            .is_ok());

        let stats = manager.get_stats().await;
        assert_eq!(stats.total_allocated, size + overhead);
        assert_eq!(stats.total_used, size);
        assert_eq!(stats.allocation_count, 1);
        assert_eq!(stats.peak_usage, size);

        assert!(manager.deallocate("test-1").await.is_ok());

        // Verify cleanup
        let final_stats = manager.get_stats().await;
        assert_eq!(final_stats.total_allocated, 0);
        assert_eq!(final_stats.total_used, 0);
        assert_eq!(final_stats.allocation_count, 1);
    }
} 