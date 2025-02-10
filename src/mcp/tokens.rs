#![allow(dead_code, unused_imports, unused_variables)]

//! Token Management Module
//! 
//! Provides token usage tracking and management:
//! - Token usage monitoring
//! - Cost calculation
//! - Usage limits enforcement

use std::collections::HashMap;
use std::sync::Arc;
use chrono::{DateTime, Utc};
use log::debug;
use serde::{Serialize, Deserialize};
use crate::memory::MemoryManager;
use crate::tokens::{self, TokenManager as BaseTokenManager};

pub use crate::tokens::TokenMetrics;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ModelType {
    GPT4,
    GPT35,
    Claude2,
    Claude3,
}

impl From<ModelType> for tokens::ModelType {
    fn from(model: ModelType) -> Self {
        match model {
            ModelType::GPT4 => tokens::ModelType::GPT4,
            ModelType::GPT35 => tokens::ModelType::GPT35,
            ModelType::Claude2 => tokens::ModelType::Claude2,
            ModelType::Claude3 => tokens::ModelType::Claude3,
        }
    }
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
    base_manager: Arc<BaseTokenManager>,
}

impl TokenManager {
    pub fn new(memory_manager: Arc<MemoryManager>) -> Self {
        Self {
            base_manager: Arc::new(BaseTokenManager::new(memory_manager)),
        }
    }

    pub async fn track_usage(
        &self,
        model: ModelType,
        prompt_tokens: usize,
        completion_tokens: usize,
        metadata: HashMap<String, String>,
    ) -> Result<(), crate::error::NexaError> {
        self.base_manager.track_usage(model.into(), prompt_tokens, completion_tokens, metadata).await
    }

    pub async fn get_usage_since(&self, since: DateTime<Utc>) -> tokens::TokenMetrics {
        self.base_manager.get_usage_since(since).await
    }
} 