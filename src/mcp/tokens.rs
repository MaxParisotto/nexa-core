//! Token Management Module
//! 
//! Provides token usage tracking and management:
//! - Token usage monitoring
//! - Cost calculation
//! - Usage limits enforcement

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use tracing::debug;
use crate::memory::MemoryManager;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ModelType {
    GPT4,
    GPT35Turbo,
    Claude2,
    Claude3,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub total_tokens: usize,
    pub cost: f64,
}

#[derive(Debug)]
pub struct TokenManager {
    memory_manager: Arc<MemoryManager>,
    usage_history: Arc<RwLock<Vec<(DateTime<Utc>, ModelType, usize, usize, f64)>>>,
}

impl TokenManager {
    pub fn new(memory_manager: Arc<MemoryManager>) -> Self {
        Self {
            memory_manager,
            usage_history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn track_usage(
        &self,
        model: ModelType,
        prompt_tokens: usize,
        completion_tokens: usize,
        metadata: HashMap<String, String>,
    ) -> Result<(), crate::error::NexaError> {
        let total_tokens = prompt_tokens + completion_tokens;
        
        // Calculate cost based on model type
        let cost = match model {
            ModelType::GPT4 => (prompt_tokens as f64 * 0.03 + completion_tokens as f64 * 0.06) / 1000.0,
            ModelType::GPT35Turbo => (prompt_tokens as f64 * 0.001 + completion_tokens as f64 * 0.002) / 1000.0,
            ModelType::Claude2 => (total_tokens as f64 * 0.01) / 1000.0,
            ModelType::Claude3 => (total_tokens as f64 * 0.02) / 1000.0,
        };

        // Track memory usage
        self.memory_manager.allocate(
            format!("token-usage-{}", Utc::now().timestamp()),
            crate::memory::ResourceType::TokenBuffer,
            total_tokens * std::mem::size_of::<char>(),
            metadata,
        ).await?;

        // Record usage
        let mut history = self.usage_history.write().await;
        history.push((Utc::now(), model, prompt_tokens, completion_tokens, cost));

        debug!(
            "Tracked token usage - Model: {:?}, Prompt: {}, Completion: {}, Cost: ${:.4}",
            model, prompt_tokens, completion_tokens, cost
        );

        Ok(())
    }

    pub async fn get_usage_since(&self, since: DateTime<Utc>) -> TokenUsage {
        let history = self.usage_history.read().await;
        let mut total_prompt = 0;
        let mut total_completion = 0;
        let mut total_cost = 0.0;

        for (timestamp, _, prompt, completion, cost) in history.iter() {
            if *timestamp >= since {
                total_prompt += prompt;
                total_completion += completion;
                total_cost += cost;
            }
        }

        TokenUsage {
            prompt_tokens: total_prompt,
            completion_tokens: total_completion,
            total_tokens: total_prompt + total_completion,
            cost: total_cost,
        }
    }
} 