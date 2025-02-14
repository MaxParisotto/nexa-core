#![allow(dead_code, unused_imports, unused_variables)]

//! Token Usage Tracking System
//! 
//! This module provides token usage tracking and management:
//! - Token consumption monitoring
//! - Rate limiting
//! - Cost tracking
//! - Usage analytics

use std::sync::Arc;
use tokio::sync::RwLock;
use crate::error::NexaError;
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModelType {
    GPT35Turbo,
    GPT4,
    Claude2,
    Ollama,
    LMStudio,
}

#[derive(Debug, Clone)]
pub struct TokenMetrics {
    pub total_tokens: u64,
    pub max_tokens: u64,
}

impl Default for TokenMetrics {
    fn default() -> Self {
        Self {
            total_tokens: 0,
            max_tokens: 0,
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

pub struct TokenManager {
    metrics: Arc<RwLock<TokenMetrics>>,
    usage_records: Arc<RwLock<Vec<UsageRecord>>>,
    model_limits: HashMap<ModelType, u64>,
}

impl std::fmt::Debug for TokenManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenManager")
            .field("model_limits", &self.model_limits)
            .finish()
    }
}

impl TokenManager {
    pub fn new() -> Self {
        let mut model_limits = HashMap::new();
        model_limits.insert(ModelType::GPT35Turbo, 4096);
        model_limits.insert(ModelType::GPT4, 8192);
        model_limits.insert(ModelType::Claude2, 100_000);
        model_limits.insert(ModelType::Ollama, 4096);
        model_limits.insert(ModelType::LMStudio, 4096);

        Self {
            metrics: Arc::new(RwLock::new(TokenMetrics::default())),
            usage_records: Arc::new(RwLock::new(Vec::new())),
            model_limits,
        }
    }

    /// Track token usage for a model interaction
    pub async fn track_usage(
        &self,
        model: ModelType,
        prompt_tokens: usize,
        completion_tokens: usize,
        metadata: HashMap<String, String>,
    ) -> Result<(), NexaError> {
        let total = prompt_tokens + completion_tokens;
        let max = total as u64;

        // Check model limits
        if let Some(limit) = self.model_limits.get(&model) {
            if total > *limit as usize {
                return Err(NexaError::System(format!(
                    "Token limit exceeded: {} > {}",
                    total,
                    limit
                )));
            }
        }

        // Calculate cost (example rates - local models are free)
        let cost = match model {
            ModelType::GPT4 => (prompt_tokens as f64 * 0.03 + completion_tokens as f64 * 0.06) / 1000.0,
            ModelType::GPT35Turbo => (prompt_tokens as f64 * 0.001 + completion_tokens as f64 * 0.002) / 1000.0,
            ModelType::Ollama | ModelType::LMStudio => 0.0, // Local models are free
            _ => 0.0,
        };

        let usage = TokenMetrics {
            total_tokens: total as u64,
            max_tokens: max,
        };

        // Record usage with all fields
        let mut records = self.usage_records.write().await;
        records.push(UsageRecord {
            model,
            usage,
            timestamp: Utc::now(),
            metadata,
        });

        self.update_metrics(total as u64, max).await?;
        Ok(())
    }

    /// Get total usage for a time period
    pub async fn get_usage_since(&self, since: DateTime<Utc>) -> TokenMetrics {
        let records = self.usage_records.read().await;
        records
            .iter()
            .filter(|r| r.timestamp >= since)
            .fold(
                TokenMetrics::default(),
                |mut acc, r| {
                    acc.total_tokens += r.usage.total_tokens;
                    acc.max_tokens = acc.max_tokens.max(r.usage.max_tokens);
                    acc
                },
            )
    }

    /// Get usage by model type
    pub async fn get_usage_by_model(&self, model: ModelType) -> TokenMetrics {
        let records = self.usage_records.read().await;
        let model_records: Vec<_> = records.iter()
            .filter(|r| r.model == model)
            .collect();

        if model_records.is_empty() {
            return TokenMetrics::default();
        }

        let total_tokens = model_records.iter()
            .map(|r| r.usage.total_tokens)
            .sum();

        let max_tokens = self.model_limits.get(&model).copied().unwrap_or(0);

        TokenMetrics {
            total_tokens,
            max_tokens,
        }
    }

    /// Clear old usage records
    pub async fn cleanup_old_records(&self, before: DateTime<Utc>) -> Result<(), NexaError> {
        let mut records = self.usage_records.write().await;
        records.retain(|r| r.timestamp >= before);
        Ok(())
    }

    pub async fn get_usage(&self) -> Result<u64, NexaError> {
        Ok(self.metrics.read().await.total_tokens)
    }

    pub async fn get_max_tokens(&self) -> Result<u64, NexaError> {
        Ok(self.metrics.read().await.max_tokens)
    }

    pub async fn get_metrics(&self) -> Result<TokenMetrics, NexaError> {
        Ok(self.metrics.read().await.clone())
    }

    pub async fn update_metrics(&self, total: u64, max: u64) -> Result<(), NexaError> {
        let mut metrics = self.metrics.write().await;
        metrics.total_tokens = total;
        metrics.max_tokens = max;
        Ok(())
    }

    pub async fn record_usage(&self, model: ModelType, total_tokens: u64) -> Result<(), NexaError> {
        let max_tokens = *self.model_limits.get(&model).unwrap_or(&0);
        let usage = TokenMetrics {
            total_tokens,
            max_tokens,
        };

        let record = UsageRecord {
            model,
            usage,
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        };

        let mut records = self.usage_records.write().await;
        records.push(record);

        self.update_metrics(total_tokens, max_tokens).await?;
        Ok(())
    }
}

impl Default for TokenManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_token_usage() {
        let token_manager = TokenManager::new();
        token_manager.record_usage(ModelType::GPT4, 150).await.unwrap();

        let usage = token_manager.get_usage_by_model(ModelType::GPT4).await;
        assert_eq!(usage.total_tokens, 150);
        assert_eq!(usage.max_tokens, 8192);
    }
} 