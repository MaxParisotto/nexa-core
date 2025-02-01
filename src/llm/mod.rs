pub mod system_helper;
#[cfg(test)]
pub mod test_utils;

pub use system_helper::*;

use serde::{Deserialize, Serialize};
use reqwest::Client;
use std::time::Duration;
use crate::error::NexaError;
use tracing::{debug, error, info};

/// Configuration for LLM client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMConfig {
    /// Server URL
    pub server_url: String,
    /// Request timeout in seconds
    pub timeout_secs: u64,
    /// Maximum tokens to generate
    pub max_tokens: usize,
    /// Temperature for generation (0.0 - 1.0)
    pub temperature: f32,
    /// Top-p sampling
    pub top_p: f32,
    /// Stop sequences
    pub stop: Vec<String>,
}

impl Default for LLMConfig {
    fn default() -> Self {
        Self {
            server_url: "http://localhost:1234".to_string(),
            timeout_secs: 30,
            max_tokens: 1000,
            temperature: 0.7,
            top_p: 0.9,
            stop: vec![],
        }
    }
}

/// Request body for LLM API
#[derive(Debug, Serialize)]
struct LLMRequest {
    prompt: String,
    max_tokens: usize,
    temperature: f32,
    top_p: f32,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    stop: Vec<String>,
}

/// Response from LLM API
#[derive(Debug, Deserialize)]
struct LLMResponse {
    text: String,
    usage: Option<TokenUsage>,
}

/// Token usage information
#[derive(Debug, Deserialize)]
struct TokenUsage {
    prompt_tokens: usize,
    completion_tokens: usize,
    total_tokens: usize,
}

/// Client for interacting with LLM server
#[derive(Debug, Clone)]
pub struct LLMClient {
    config: LLMConfig,
    client: Client,
}

impl LLMClient {
    /// Create a new LLM client
    pub fn new(config: LLMConfig) -> Result<Self, NexaError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .map_err(|e| NexaError::system(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self { config, client })
    }

    /// Generate text completion
    pub async fn complete(&self, prompt: &str) -> Result<String, NexaError> {
        let request = LLMRequest {
            prompt: prompt.to_string(),
            max_tokens: self.config.max_tokens,
            temperature: self.config.temperature,
            top_p: self.config.top_p,
            stop: self.config.stop.clone(),
        };

        let response = self.client
            .post(&self.config.server_url)
            .json(&request)
            .send()
            .await
            .map_err(|e| NexaError::system(format!("Failed to send request: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await
                .unwrap_or_else(|_| "Failed to get error response".to_string());
            return Err(NexaError::system(format!("LLM request failed ({}): {}", status, text)));
        }

        let llm_response: LLMResponse = response.json()
            .await
            .map_err(|e| NexaError::system(format!("Failed to parse response: {}", e)))?;

        if let Some(usage) = llm_response.usage {
            debug!(
                "LLM usage - Prompt: {}, Completion: {}, Total: {}",
                usage.prompt_tokens, usage.completion_tokens, usage.total_tokens
            );
        }

        Ok(llm_response.text)
    }

    /// Generate function call
    pub async fn call_function<T: Serialize, R: for<'de> Deserialize<'de>>(
        &self,
        function_name: &str,
        args: &T,
    ) -> Result<R, NexaError> {
        let prompt = format!(
            "Call function '{}' with arguments: {}",
            function_name,
            serde_json::to_string(args)
                .map_err(|e| NexaError::system(format!("Failed to serialize arguments: {}", e)))?
        );

        let response = self.complete(&prompt).await?;

        serde_json::from_str(&response)
            .map_err(|e| NexaError::system(format!("Failed to parse function response: {}", e)))
    }

    /// Generate reasoning about a topic
    pub async fn reason(&self, topic: &str, context: Option<&str>) -> Result<String, NexaError> {
        let prompt = match context {
            Some(ctx) => format!(
                "Given this context:\n{}\n\nPlease reason about: {}",
                ctx, topic
            ),
            None => format!("Please reason about: {}", topic),
        };

        self.complete(&prompt).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::timeout;
    use std::time::Duration;

    #[tokio::test]
    async fn test_llm_completion() {
        let config = LLMConfig {
            server_url: "http://localhost:1234".to_string(),
            timeout_secs: 5,
            max_tokens: 50,
            temperature: 0.5,
            top_p: 0.9,
            stop: vec![],
        };

        let client = LLMClient::new(config).unwrap();
        
        // Test with a simple prompt
        let result = timeout(
            Duration::from_secs(6),
            client.complete("What is 2+2?")
        ).await;

        match result {
            Ok(Ok(response)) => {
                assert!(!response.is_empty());
                assert!(response.len() < 1000); // Reasonable length check
                println!("LLM Response: {}", response);
            }
            Ok(Err(e)) => {
                if e.to_string().contains("connection refused") {
                    println!("Skipping test: LLM server not available");
                    return;
                }
                panic!("LLM request failed: {}", e);
            }
            Err(_) => panic!("LLM request timed out"),
        }
    }

    #[tokio::test]
    async fn test_function_call() {
        let config = LLMConfig::default();
        let client = LLMClient::new(config).unwrap();

        #[derive(Debug, Serialize)]
        struct CalcArgs {
            x: i32,
            y: i32,
        }

        #[derive(Debug, Deserialize, PartialEq)]
        struct CalcResult {
            sum: i32,
        }

        let result = timeout(
            Duration::from_secs(6),
            client.call_function::<CalcArgs, CalcResult>(
                "add_numbers",
                &CalcArgs { x: 5, y: 3 }
            )
        ).await;

        match result {
            Ok(Ok(response)) => {
                assert_eq!(response.sum, 8);
            }
            Ok(Err(e)) => {
                if e.to_string().contains("connection refused") {
                    println!("Skipping test: LLM server not available");
                    return;
                }
                panic!("Function call failed: {}", e);
            }
            Err(_) => panic!("Function call timed out"),
        }
    }

    #[tokio::test]
    async fn test_reasoning() {
        let config = LLMConfig::default();
        let client = LLMClient::new(config).unwrap();

        let result = timeout(
            Duration::from_secs(6),
            client.reason(
                "What are the implications of using async/await in Rust?",
                Some("Consider performance, error handling, and code complexity.")
            )
        ).await;

        match result {
            Ok(Ok(response)) => {
                assert!(!response.is_empty());
                assert!(response.contains("async") || response.contains("await"));
                println!("Reasoning Response: {}", response);
            }
            Ok(Err(e)) => {
                if e.to_string().contains("connection refused") {
                    println!("Skipping test: LLM server not available");
                    return;
                }
                panic!("Reasoning request failed: {}", e);
            }
            Err(_) => panic!("Reasoning request timed out"),
        }
    }
} 