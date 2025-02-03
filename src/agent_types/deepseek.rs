use std::path::PathBuf;
use tokio::sync::mpsc;
use log::{debug, error, info};

use crate::{
    capabilities::{Capability, CodeGeneration, CodeReview, TestGeneration},
    context::ContextManager,
    errors::AgentError,
    llm::LlamaModel,
    mcp::{McpMessage, WebSocketClient},
};

#[derive(Debug)]
pub struct DeepseekConfig {
    pub model_path: PathBuf,
    pub context_size: usize,
    pub temperature: f32,
    pub max_tokens: usize,
    pub top_p: f32,
    pub presence_penalty: f32,
    pub frequency_penalty: f32,
}

impl Default for DeepseekConfig {
    fn default() -> Self {
        Self {
            model_path: PathBuf::from("models/deepseek-coder-33b.gguf"),
            context_size: 8192,
            temperature: 0.7,
            max_tokens: 2048,
            top_p: 0.95,
            presence_penalty: 0.0,
            frequency_penalty: 0.0,
        }
    }
}

pub struct DeepseekAgent {
    model: LlamaModel,
    context_manager: ContextManager,
    mcp_client: WebSocketClient,
    capabilities: Vec<Capability>,
    config: DeepseekConfig,
}

impl DeepseekAgent {
    pub async fn new(config: DeepseekConfig) -> Result<Self, AgentError> {
        let model = LlamaModel::new(&config.model_path).await?;
        let context_manager = ContextManager::new(config.context_size);
        let mcp_client = WebSocketClient::new().await?;
        
        let capabilities = vec![
            Capability::CodeGeneration(CodeGeneration::new()),
            Capability::CodeReview(CodeReview::new()),
            Capability::TestGeneration(TestGeneration::new()),
        ];

        Ok(Self {
            model,
            context_manager,
            mcp_client,
            capabilities,
            config,
        })
    }

    pub async fn register(&mut self) -> Result<(), AgentError> {
        info!("Registering Deepseek agent with MCP server");
        self.mcp_client.register_agent("deepseek", &self.capabilities).await?;
        Ok(())
    }

    pub async fn handle_task(&mut self, msg: McpMessage) -> Result<(), AgentError> {
        debug!("Handling task: {:?}", msg);
        
        let (response_tx, mut response_rx) = mpsc::channel(32);
        
        match msg.task_type {
            "code_generation" => {
                self.handle_code_generation(msg, response_tx).await?;
            }
            "code_review" => {
                self.handle_code_review(msg, response_tx).await?;
            }
            "test_generation" => {
                self.handle_test_generation(msg, response_tx).await?;
            }
            _ => {
                error!("Unsupported task type: {}", msg.task_type);
                return Err(AgentError::UnsupportedTaskType);
            }
        }

        while let Some(response) = response_rx.recv().await {
            self.mcp_client.send_response(response).await?;
        }

        Ok(())
    }

    async fn handle_code_generation(
        &mut self,
        msg: McpMessage,
        response_tx: mpsc::Sender<String>,
    ) -> Result<(), AgentError> {
        let prompt = self.context_manager.build_code_generation_prompt(&msg.content)?;
        
        self.model
            .stream_generate(
                &prompt,
                self.config.max_tokens,
                self.config.temperature,
                response_tx,
            )
            .await?;

        Ok(())
    }

    async fn handle_code_review(
        &mut self,
        msg: McpMessage,
        response_tx: mpsc::Sender<String>,
    ) -> Result<(), AgentError> {
        let prompt = self.context_manager.build_code_review_prompt(&msg.content)?;
        
        self.model
            .stream_generate(
                &prompt,
                self.config.max_tokens,
                self.config.temperature,
                response_tx,
            )
            .await?;

        Ok(())
    }

    async fn handle_test_generation(
        &mut self,
        msg: McpMessage,
        response_tx: mpsc::Sender<String>,
    ) -> Result<(), AgentError> {
        let prompt = self.context_manager.build_test_generation_prompt(&msg.content)?;
        
        self.model
            .stream_generate(
                &prompt,
                self.config.max_tokens,
                self.config.temperature,
                response_tx,
            )
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_agent_creation() {
        let config = DeepseekConfig::default();
        let agent = DeepseekAgent::new(config).await;
        assert!(agent.is_ok());
    }

    #[tokio::test]
    async fn test_agent_registration() {
        let config = DeepseekConfig::default();
        let mut agent = DeepseekAgent::new(config).await.unwrap();
        let result = agent.register().await;
        assert!(result.is_ok());
    }
}
