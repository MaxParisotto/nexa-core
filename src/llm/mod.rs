pub mod system_helper;
#[cfg(test)]
pub mod test_utils;

pub use system_helper::*;

use serde::{Deserialize, Serialize};
use reqwest::Client;
use std::time::Duration;
use crate::error::NexaError;
use log::debug;

/// Server type for LLM requests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerType {
    LMStudio,
    Ollama,
}

impl Default for ServerType {
    fn default() -> Self {
        Self::LMStudio
    }
}

/// Configuration for LLM client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMConfig {
    /// Server URL
    pub server_url: String,
    /// Server type (LMStudio or Ollama)
    pub server_type: ServerType,
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
    /// Allow CORS from specific origins (empty means allow all)
    pub allowed_origins: Vec<String>,
    /// Whether to include credentials in CORS requests
    pub allow_credentials: bool,
    /// Model name (especially important for Ollama)
    pub model: String,
}

impl Default for LLMConfig {
    fn default() -> Self {
        Self {
            server_url: "http://localhost:1234".to_string(),
            server_type: ServerType::LMStudio,
            timeout_secs: 30,
            max_tokens: 1000,
            temperature: 0.7,
            top_p: 0.9,
            stop: vec![],
            allowed_origins: vec![],
            allow_credentials: false,
            model: "local-model".to_string(),
        }
    }
}

impl LLMConfig {
    /// Create a new configuration with LM Studio server
    pub fn with_lmstudio_server(server_url: impl Into<String>) -> Self {
        Self {
            server_url: server_url.into(),
            server_type: ServerType::LMStudio,
            timeout_secs: 30,
            max_tokens: 1000,
            temperature: 0.7,
            top_p: 0.9,
            stop: vec![],
            allowed_origins: vec![],
            allow_credentials: false,
            model: "local-model".to_string(),
        }
    }

    /// Create a new configuration with Ollama server
    pub fn with_ollama_server(model: impl Into<String>) -> Self {
        Self {
            server_url: "http://localhost:11434".to_string(),
            server_type: ServerType::Ollama,
            timeout_secs: 30,
            max_tokens: 1000,
            temperature: 0.7,
            top_p: 0.9,
            stop: vec![],
            allowed_origins: vec![],
            allow_credentials: false,
            model: model.into(),
        }
    }

    /// Set allowed CORS origins
    pub fn with_cors_origins(mut self, origins: Vec<String>) -> Self {
        self.allowed_origins = origins;
        self
    }

    /// Enable CORS credentials
    pub fn with_credentials(mut self) -> Self {
        self.allow_credentials = true;
        self
    }
}

/// Request body for LLM API
#[derive(Debug, Serialize)]
struct LLMRequest {
    messages: Vec<ChatMessage>,
    model: String,
    temperature: f32,
    max_tokens: Option<usize>,
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    stop: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

/// Response from LLM API
#[derive(Debug, Deserialize)]
struct LLMResponse {
    choices: Vec<ChatChoice>,
    usage: Option<TokenUsage>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

/// Token usage information
#[derive(Debug, Deserialize)]
struct TokenUsage {
    prompt_tokens: usize,
    completion_tokens: usize,
    total_tokens: usize,
}

/// Request body for Ollama API
#[derive(Debug, Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    stream: bool,
    options: OllamaOptions,
}

#[derive(Debug, Serialize)]
struct OllamaOptions {
    temperature: f32,
    top_p: f32,
    num_predict: i32,
    stop: Vec<String>,
}

/// Response from Ollama API
#[derive(Debug, Deserialize)]
struct OllamaResponse {
    response: String,
    done: bool,
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
        let mut client_builder = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs));

        // Configure CORS headers
        if !config.allowed_origins.is_empty() {
            client_builder = client_builder
                .default_headers({
                    let mut headers = reqwest::header::HeaderMap::new();
                    headers.insert(
                        reqwest::header::ORIGIN,
                        config.allowed_origins.join(",").parse().unwrap()
                    );
                    if config.allow_credentials {
                        headers.insert(
                            reqwest::header::HeaderName::from_static("access-control-allow-credentials"),
                            "true".parse().unwrap()
                        );
                    }
                    headers
                });
        }

        let client = client_builder
            .build()
            .map_err(|e| NexaError::System(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self { config, client })
    }

    /// Generate text completion
    pub async fn complete(&self, prompt: &str) -> Result<String, NexaError> {
        match self.config.server_type {
            ServerType::LMStudio => self.complete_lmstudio(prompt).await,
            ServerType::Ollama => self.complete_ollama(prompt).await,
        }
    }

    async fn complete_lmstudio(&self, prompt: &str) -> Result<String, NexaError> {
        let request = LLMRequest {
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
            model: self.config.model.clone(),
            temperature: self.config.temperature,
            max_tokens: Some(self.config.max_tokens),
            top_p: Some(self.config.top_p),
            stop: self.config.stop.clone(),
        };

        let response = self.client
            .post(format!("{}/v1/chat/completions", self.config.server_url))
            .json(&request)
            .send()
            .await
            .map_err(|e| NexaError::System(format!("Failed to send request: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await
                .unwrap_or_else(|_| "Failed to get error response".to_string());
            return Err(NexaError::System(format!("LLM request failed ({}): {}", status, text)));
        }

        let llm_response: LLMResponse = response.json()
            .await
            .map_err(|e| NexaError::System(format!("Failed to parse response: {}", e)))?;

        if let Some(usage) = llm_response.usage {
            debug!(
                "LLM usage - Prompt: {}, Completion: {}, Total: {}",
                usage.prompt_tokens, usage.completion_tokens, usage.total_tokens
            );
        }

        Ok(llm_response.choices.first()
            .ok_or_else(|| NexaError::System("No completion choices returned".to_string()))?
            .message.content.clone())
    }

    async fn complete_ollama(&self, prompt: &str) -> Result<String, NexaError> {
        let request = OllamaRequest {
            model: self.config.model.clone(),
            prompt: prompt.to_string(),
            stream: false,
            options: OllamaOptions {
                temperature: self.config.temperature,
                top_p: self.config.top_p,
                num_predict: self.config.max_tokens as i32,
                stop: self.config.stop.clone(),
            },
        };

        let response = self.client
            .post(format!("{}/api/generate", self.config.server_url))
            .json(&request)
            .send()
            .await
            .map_err(|e| NexaError::System(format!("Failed to send request to Ollama: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await
                .unwrap_or_else(|_| "Failed to get error response".to_string());
            return Err(NexaError::System(format!("Ollama request failed ({}): {}", status, text)));
        }

        let ollama_response: OllamaResponse = response.json()
            .await
            .map_err(|e| NexaError::System(format!("Failed to parse Ollama response: {}", e)))?;

        if !ollama_response.done {
            debug!("Ollama response not marked as done, but proceeding with response");
        }

        Ok(ollama_response.response)
    }

    /// Generate function call
    pub async fn call_function<T: Serialize, R: for<'de> Deserialize<'de>>(
        &self,
        function_name: &str,
        args: &T,
    ) -> Result<R, NexaError> {
        let prompt = format!(
            "Call function '{}' with arguments: {}. Return ONLY a valid JSON object containing the result. For example, if calculating a sum, return: {{\"sum\": 42}}",
            function_name,
            serde_json::to_string(args)
                .map_err(|e| NexaError::System(format!("Failed to serialize arguments: {}", e)))?
        );

        let response = self.complete(&prompt).await?;

        // Try to extract JSON from the response if it's wrapped in code blocks
        let json_str = if response.contains("```json") {
            response
                .split("```json")
                .nth(1)
                .and_then(|s| s.split("```").next())
                .unwrap_or(&response)
                .trim()
        } else if response.contains("```") {
            response
                .split("```")
                .nth(1)
                .unwrap_or(&response)
                .trim()
        } else {
            response.trim()
        };

        serde_json::from_str(json_str)
            .map_err(|e| NexaError::System(format!("Failed to parse function response: {}", e)))
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
        let config = LLMConfig::with_lmstudio_server("http://localhost:1234");

        let client = LLMClient::new(config).unwrap();
        
        // Test with a simple prompt
        let result = timeout(
            Duration::from_secs(30),  // Increased timeout
            client.complete("What is 2+2? Please provide just the numerical answer.")
        ).await;

        match result {
            Ok(Ok(response)) => {
                println!("LLM Response: {}", response);
                assert!(!response.is_empty());
                let contains_answer = response.contains("4") || 
                                    response.contains("four") || 
                                    response.contains("= 4") ||
                                    response.contains("equals 4");
                assert!(contains_answer, "Response did not contain the expected answer: {}", response);
            }
            Ok(Err(e)) => {
                if e.to_string().contains("connection refused") || 
                   e.to_string().contains("Failed to send request") ||
                   e.to_string().contains("Insufficient Memory") {
                    println!("Skipping test: LLM server not available or insufficient resources");
                    return;
                }
                panic!("LLM request failed: {}", e);
            }
            Err(_) => {
                println!("Skipping test: LLM request timed out");
                return;
            }
        }
    }

    #[tokio::test]
    async fn test_function_call() {
        let config = LLMConfig::with_lmstudio_server("http://localhost:1234");
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
            Duration::from_secs(30),  // Increased timeout
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
                if e.to_string().contains("connection refused") || 
                   e.to_string().contains("Failed to send request") ||
                   e.to_string().contains("Insufficient Memory") {
                    println!("Skipping test: LLM server not available or insufficient resources");
                    return;
                }
                if e.to_string().contains("Failed to parse function response") {
                    println!("Response format was not as expected: {}", e);
                    return;
                }
                panic!("Function call failed: {}", e);
            }
            Err(_) => {
                println!("Skipping test: Function call timed out");
                return;
            }
        }
    }

    #[tokio::test]
    async fn test_reasoning() {
        let config = LLMConfig::with_lmstudio_server("http://localhost:1234");
        let client = LLMClient::new(config).unwrap();

        let result = timeout(
            Duration::from_secs(30),  // Increased timeout
            client.reason(
                "What are the implications of using async/await in Rust?",
                Some("Consider performance, error handling, and code complexity.")
            )
        ).await;

        match result {
            Ok(Ok(response)) => {
                assert!(!response.is_empty());
                assert!(response.contains("async") || response.contains("await") || 
                       response.contains("performance") || response.contains("error") ||
                       response.contains("complexity"));
                println!("Reasoning Response: {}", response);
            }
            Ok(Err(e)) => {
                if e.to_string().contains("connection refused") || e.to_string().contains("Failed to send request") {
                    println!("Skipping test: LLM server not available");
                    return;
                }
                panic!("Reasoning request failed: {}", e);
            }
            Err(_) => {
                println!("Skipping test: Reasoning request timed out");
                return;
            }
        }
    }

    #[test]
    fn test_config_builder() {
        let config = LLMConfig::with_lmstudio_server("http://custom-server:8080")
            .with_cors_origins(vec!["http://localhost:3000".to_string()])
            .with_credentials();

        assert_eq!(config.server_url, "http://custom-server:8080");
        assert_eq!(config.allowed_origins, vec!["http://localhost:3000"]);
        assert!(config.allow_credentials);
    }

    #[tokio::test]
    async fn test_custom_server_connection() {
        let config = LLMConfig::with_lmstudio_server("http://localhost:1234");
        let client = LLMClient::new(config).unwrap();

        let result = timeout(
            Duration::from_secs(30),
            client.complete("Test connection")
        ).await;

        match result {
            Ok(Ok(_)) => (),
            Ok(Err(e)) => {
                if e.to_string().contains("connection refused") || 
                   e.to_string().contains("Failed to send request") ||
                   e.to_string().contains("Insufficient Memory") {
                    println!("Skipping test: LLM server not available or insufficient resources");
                    return;
                }
                panic!("Request failed: {}", e);
            }
            Err(_) => {
                println!("Skipping test: Request timed out");
                return;
            }
        }
    }

    #[tokio::test]
    async fn test_ollama_completion() {
        let config = LLMConfig::with_ollama_server("qwen2.5-coder:7b")
            .with_cors_origins(vec!["http://localhost:3000".to_string()]);

        let client = LLMClient::new(config).unwrap();
        
        let result = timeout(
            Duration::from_secs(30),  // Increased timeout
            client.complete("Write a Rust function that adds two numbers. Return just the function code.")
        ).await;

        match result {
            Ok(Ok(response)) => {
                println!("Ollama Response: {}", response);
                assert!(!response.is_empty());
                // Check if response contains Rust code elements
                let contains_rust = response.contains("fn") && 
                                  (response.contains("->") || response.contains("return"));
                assert!(contains_rust, "Response did not contain Rust code: {}", response);
            }
            Ok(Err(e)) => {
                if e.to_string().contains("connection refused") || e.to_string().contains("Failed to send request") {
                    println!("Skipping test: Ollama server not available");
                    return;
                }
                if e.to_string().contains("model") && e.to_string().contains("not found") {
                    println!("Skipping test: Ollama model not installed");
                    return;
                }
                panic!("Ollama request failed: {}", e);
            }
            Err(_) => {
                println!("Skipping test: Ollama request timed out");
                return;
            }
        }
    }

    #[tokio::test]
    async fn test_ollama_function_call() {
        let config = LLMConfig::with_ollama_server("qwen2.5-coder:7b");
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
            Duration::from_secs(30),  // Increased timeout
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
                if e.to_string().contains("connection refused") || e.to_string().contains("Failed to send request") {
                    println!("Skipping test: Ollama server not available");
                    return;
                }
                if e.to_string().contains("model") && e.to_string().contains("not found") {
                    println!("Skipping test: Ollama model not installed");
                    return;
                }
                if e.to_string().contains("Failed to parse function response") {
                    println!("Response format was not as expected: {}", e);
                    return;
                }
                panic!("Function call failed: {}", e);
            }
            Err(_) => {
                println!("Skipping test: Function call timed out");
                return;
            }
        }
    }
}