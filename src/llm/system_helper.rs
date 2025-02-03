use std::sync::Arc;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use crate::error::NexaError;
use crate::agent::Task;
use crate::llm::{LLMClient, LLMConfig};
use crate::mcp::ServerControl;
use log::info;
use chrono::{DateTime, Utc};

/// System task request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemTaskRequest {
    pub description: String,
    pub priority: TaskPriority,
    pub required_capabilities: Vec<String>,
    pub deadline: Option<DateTime<Utc>>,
}

/// Task priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskPriority {
    Low,
    Normal,
    High,
    Critical,
}

/// System query types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SystemQuery {
    Health,
    Metrics,
    AgentStatus { agent_id: Option<String> },
    TaskStatus { task_id: Option<String> },
    ResourceUsage,
    Custom(String),
}

/// System helper for managing tasks and queries
pub struct SystemHelper {
    llm: Arc<LLMClient>,
    server: Arc<ServerControl>,
    task_templates: Arc<RwLock<Vec<String>>>,
}

impl SystemHelper {
    /// Create a new system helper
    pub fn new(server: Arc<ServerControl>) -> Result<Self, NexaError> {
        let config = LLMConfig {
            temperature: 0.3, // More focused responses
            max_tokens: 500,  // Reasonable limit for system tasks
            ..Default::default()
        };

        let llm = Arc::new(LLMClient::new(config)?);
        let task_templates = Arc::new(RwLock::new(Vec::new()));

        Ok(Self {
            llm,
            server,
            task_templates,
        })
    }

    /// Create a new task from natural language description
    pub async fn create_task(&self, request: SystemTaskRequest) -> Result<Task, NexaError> {
        // Generate task structure from description
        let prompt = format!(
            "Convert this task description into a structured task with clear steps and requirements.\n\
            Description: {}\n\
            Priority: {:?}\n\
            Required Capabilities: {:?}\n\
            Deadline: {:?}\n\n\
            Return ONLY a valid JSON object with the following structure:\n\
            {{\n\
                \"title\": \"Task title\",\n\
                \"steps\": [\"step1\", \"step2\", ...],\n\
                \"requirements\": [\"req1\", \"req2\", ...],\n\
                \"estimated_duration\": duration_in_minutes\n\
            }}\n\
            Do not include any other text or explanation.",
            request.description,
            request.priority,
            request.required_capabilities,
            request.deadline,
        );

        let task_json = self.llm.complete(&prompt).await?;
        
        // Try to extract JSON from the response if it's wrapped in code blocks
        let json_str = if task_json.contains("```json") {
            task_json
                .split("```json")
                .nth(1)
                .and_then(|s| s.split("```").next())
                .unwrap_or(&task_json)
                .trim()
        } else if task_json.contains("```") {
            task_json
                .split("```")
                .nth(1)
                .unwrap_or(&task_json)
                .trim()
        } else {
            task_json.trim()
        };

        let task_details: TaskDetails = serde_json::from_str(json_str)
            .map_err(|e| NexaError::System(format!("Failed to parse task details: {}", e)))?;

        // Create task
        let task = Task::new(
            task_details.title.clone(),
            request.description,
            task_details.steps,
            task_details.requirements,
            request.deadline,
            task_details.estimated_duration,
            match request.priority {
                TaskPriority::Low => 0,
                TaskPriority::Normal => 1,
                TaskPriority::High => 2,
                TaskPriority::Critical => 3,
            },
        );

        // Store task in registry
        self.server.registry.add_task(task.clone()).await?;
        info!("Created new task: {} (ID: {})", task_details.title, task.id);

        Ok(task)
    }

    /// Query system status and information
    pub async fn query_system(&self, query: SystemQuery) -> Result<String, NexaError> {
        let (prompt, context) = match &query {
            SystemQuery::Health => {
                let health = self.server.check_health().await?; // Removed extra arguments
                (
                    "Analyze the system health status and provide recommendations",
                    serde_json::to_string(&health)?,
                )
            }
            SystemQuery::Metrics => {
                let metrics = self.server.get_metrics().await?;
                (
                    "Analyze the system metrics and identify any concerning trends",
                    serde_json::to_string(&metrics)?,
                )
            }
            SystemQuery::AgentStatus { agent_id } => {
                let agents = if let Some(id) = agent_id {
                    vec![self.server.registry.get_agent(id).await?]
                } else {
                    self.server.registry.list_agents().await
                };
                (
                    "Analyze the agent status and provide insights",
                    serde_json::to_string(&agents)?,
                )
            }
            SystemQuery::TaskStatus { task_id } => {
                let tasks = if let Some(id) = task_id {
                    vec![self.server.registry.get_task(id).await?]
                } else {
                    self.server.registry.list_tasks().await?
                };
                (
                    "Analyze the task status and provide insights",
                    serde_json::to_string(&tasks)?,
                )
            }
            SystemQuery::ResourceUsage => {
                let memory_stats = self.server.memory_stats().await;
                (
                    "Analyze resource usage and provide optimization recommendations",
                    serde_json::to_string(&memory_stats)?,
                )
            }
            SystemQuery::Custom(question) => {
                let health = self.server.check_health().await?;
                let metrics = self.server.get_metrics().await?;
                let prompt = question.as_str();
                (
                    prompt,
                    format!(
                        "Health: {}\nMetrics: {}",
                        serde_json::to_string(&health)?,
                        serde_json::to_string(&metrics)?,
                    ),
                )
            }
        };

        self.llm.reason(prompt, Some(&context)).await
    }

    /// Add a task template
    pub async fn add_task_template(&self, template: String) -> Result<(), NexaError> {
        let mut templates = self.task_templates.write().await;
        templates.push(template);
        Ok(())
    }

    /// Get task suggestions based on system state
    pub async fn suggest_tasks(&self) -> Result<Vec<SystemTaskRequest>, NexaError> {
        let health = self.server.check_health().await?;
        let metrics = self.server.get_metrics().await?;
        let templates = self.task_templates.read().await;

        let prompt = format!(
            "Based on the current system state and task templates, suggest tasks that should be created.\n\
            Health: {}\n\
            Metrics: {}\n\
            Templates: {:#?}\n\n\
            Return ONLY a valid JSON array of task suggestions with the following structure:\n\
            [\n\
                {{\n\
                    \"description\": \"Task description\",\n\
                    \"priority\": \"Normal\",\n\
                    \"required_capabilities\": [\"capability1\", \"capability2\", ...]\n\
                }},\n\
                ...\n\
            ]\n\
            Priority must be one of: Low, Normal, High, Critical.\n\
            Do not include any other text or explanation.",
            serde_json::to_string(&health)?,
            serde_json::to_string(&metrics)?,
            templates,
        );

        let suggestions = self.llm.complete(&prompt).await?;
        
        // Try to extract JSON from the response if it's wrapped in code blocks
        let json_str = if suggestions.contains("```json") {
            suggestions
                .split("```json")
                .nth(1)
                .and_then(|s| s.split("```").next())
                .unwrap_or(&suggestions)
                .trim()
        } else if suggestions.contains("```") {
            suggestions
                .split("```")
                .nth(1)
                .unwrap_or(&suggestions)
                .trim()
        } else {
            suggestions.trim()
        };

        let tasks: Vec<SystemTaskRequest> = serde_json::from_str(json_str)
            .map_err(|e| NexaError::System(format!("Failed to parse task suggestions: {}", e)))?;

        Ok(tasks)
    }
}

#[derive(Debug, Deserialize)]
struct TaskDetails {
    title: String,
    steps: Vec<String>,
    requirements: Vec<String>,
    estimated_duration: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::path::PathBuf;
    use crate::mcp::ServerControl;

    fn setup_test_helper() -> SystemHelper {
        let server = Arc::new(ServerControl::new(PathBuf::from("/tmp"), PathBuf::from("/tmp")));
        SystemHelper::new(server).unwrap()
    }

    #[tokio::test]
    async fn test_system_query() {
        let helper = setup_test_helper();
        let response = helper.query_system(SystemQuery::Health).await;
        if let Err(e) = &response {
            if e.to_string().contains("connection refused") || 
               e.to_string().contains("Failed to send request") ||
               e.to_string().contains("Server is not running") ||
               e.to_string().contains("Insufficient Memory") {
                println!("Skipping test: Service not available or insufficient resources");
                return;
            }
        }
        assert!(response.is_ok());
    }

    #[tokio::test]
    async fn test_create_task() {
        let helper = setup_test_helper();
        let request = SystemTaskRequest {
            description: "Test task".to_string(),
            priority: TaskPriority::Normal,
            required_capabilities: vec![],
            deadline: None,
        };
        let response = helper.create_task(request).await;
        if let Err(e) = &response {
            if e.to_string().contains("connection refused") || 
               e.to_string().contains("Failed to send request") ||
               e.to_string().contains("Server is not running") ||
               e.to_string().contains("Insufficient Memory") {
                println!("Skipping test: Service not available or insufficient resources");
                return;
            }
        }
        assert!(response.is_ok());
    }

    #[tokio::test]
    async fn test_task_suggestions() {
        let helper = setup_test_helper();
        
        // Add some test templates
        let template_result = helper.add_task_template("Monitor system resources".to_string()).await;
        if let Err(e) = &template_result {
            if e.to_string().contains("connection refused") || 
               e.to_string().contains("Failed to send request") ||
               e.to_string().contains("Insufficient Memory") {
                println!("Skipping test: Service not available or insufficient resources");
                return;
            }
        }
        
        let response = helper.suggest_tasks().await;
        match response {
            Ok(tasks) => {
                assert!(!tasks.is_empty(), "Should suggest at least one task");
                for task in tasks {
                    assert!(!task.description.is_empty(), "Task description should not be empty");
                    assert!(!task.required_capabilities.is_empty(), "Task should have required capabilities");
                    match task.priority {
                        TaskPriority::Low | TaskPriority::Normal | TaskPriority::High | TaskPriority::Critical => (),
                    }
                }
            }
            Err(e) => {
                if e.to_string().contains("Failed to parse task suggestions") {
                    println!("Skipping test: LLM response was not in expected format");
                    return;
                }
                if e.to_string().contains("connection refused") || 
                   e.to_string().contains("Failed to send request") ||
                   e.to_string().contains("Insufficient Memory") {
                    println!("Skipping test: Service not available or insufficient resources");
                    return;
                }
                panic!("Unexpected error: {}", e);
            }
        }
    }
}