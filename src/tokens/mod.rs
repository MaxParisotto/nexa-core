//! Token Usage Tracking System
//! 
//! This module provides token usage tracking and management:
//! - Token consumption monitoring
//! - Rate limiting
//! - Cost tracking
//! - Usage analytics

use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use crate::error::NexaError;
use crate::memory::{MemoryManager, ResourceType};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum ModelType {
    GPT4,
    GPT35,
    Claude2,
    Claude3,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenMetrics {
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub total_tokens: usize,
    pub cost: f64,
}

impl Default for TokenMetrics {
    fn default() -> Self {
        Self {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
            cost: 0.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct UsageRecord {
    pub model: ModelType,
    pub usage: TokenMetrics,
    pub timestamp: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug)]
pub struct TokenManager {
    usage_records: Arc<RwLock<Vec<UsageRecord>>>,
    model_limits: HashMap<ModelType, usize>,
    memory_manager: Arc<MemoryManager>,
}

impl TokenManager {
    pub fn new(memory_manager: Arc<MemoryManager>) -> Self {
        Self {
            usage_records: Arc::new(RwLock::new(Vec::new())),
            model_limits: HashMap::new(),
            memory_manager,
        }
    }

    /// Set token limit for a model
    pub fn set_model_limit(&mut self, model: ModelType, limit: usize) {
        self.model_limits.insert(model, limit);
    }

    /// Track token usage for a model interaction
    pub async fn track_usage(
        &self,
        model: ModelType,
        prompt_tokens: usize,
        completion_tokens: usize,
        metadata: HashMap<String, String>,
    ) -> Result<(), NexaError> {
        // Check model limits
        if let Some(limit) = self.model_limits.get(&model) {
            let total = prompt_tokens + completion_tokens;
            if total > *limit {
                return Err(NexaError::System(format!(
                    "Token limit exceeded: {} > {}",
                    total,
                    limit
                )));
            }
        }

        // Calculate cost (example rates)
        let cost = match model {
            ModelType::GPT4 => (prompt_tokens as f64 * 0.03 + completion_tokens as f64 * 0.06) / 1000.0,
            ModelType::GPT35 => (prompt_tokens as f64 * 0.001 + completion_tokens as f64 * 0.002) / 1000.0,
            _ => 0.0,
        };

        let usage = TokenMetrics {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
            cost,
        };

        // Track memory allocation for tokens
        self.memory_manager
            .allocate(
                format!("tokens-{:?}-{}", model, Utc::now().timestamp()),
                ResourceType::TokenBuffer,
                (prompt_tokens + completion_tokens) * 4, // Approximate memory usage
                metadata.clone(),
            )
            .await?;

        // Record usage
        let mut records = self.usage_records.write().await;
        records.push(UsageRecord {
            model,
            usage,
            timestamp: Utc::now(),
            metadata,
        });

        Ok(())
    }

    /// Get total usage for a time period
    pub async fn get_usage_since(&self, since: DateTime<Utc>) -> TokenMetrics {
        let records = self.usage_records.read().await;
        records
            .iter()
            .filter(|r| r.timestamp >= since)
            .fold(
                TokenMetrics {
                    prompt_tokens: 0,
                    completion_tokens: 0,
                    total_tokens: 0,
                    cost: 0.0,
                },
                |mut acc, r| {
                    acc.prompt_tokens += r.usage.prompt_tokens;
                    acc.completion_tokens += r.usage.completion_tokens;
                    acc.total_tokens += r.usage.total_tokens;
                    acc.cost += r.usage.cost;
                    acc
                },
            )
    }

    /// Get usage by model type
    pub async fn get_usage_by_model(&self, model: ModelType) -> TokenMetrics {
        let records = self.usage_records.read().await;
        records
            .iter()
            .filter(|r| r.model == model)
            .fold(
                TokenMetrics {
                    prompt_tokens: 0,
                    completion_tokens: 0,
                    total_tokens: 0,
                    cost: 0.0,
                },
                |mut acc, r| {
                    acc.prompt_tokens += r.usage.prompt_tokens;
                    acc.completion_tokens += r.usage.completion_tokens;
                    acc.total_tokens += r.usage.total_tokens;
                    acc.cost += r.usage.cost;
                    acc
                },
            )
    }

    /// Clear old usage records
    pub async fn cleanup_old_records(&self, before: DateTime<Utc>) -> Result<(), NexaError> {
        let mut records = self.usage_records.write().await;
        records.retain(|r| r.timestamp >= before);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_token_tracking() {
        let memory_manager = Arc::new(MemoryManager::new());
        let token_manager = TokenManager::new(memory_manager);
        let metadata = HashMap::new();

        assert!(token_manager
            .track_usage(ModelType::GPT4, 100, 50, metadata.clone())
            .await
            .is_ok());

        let usage = token_manager.get_usage_by_model(ModelType::GPT4).await;
        assert_eq!(usage.total_tokens, 150);
        assert_eq!(usage.prompt_tokens, 100);
        assert_eq!(usage.completion_tokens, 50);
    }
} 