use crate::error::NexaError;
use crate::types::Task;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use std::sync::Arc;
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
    Medium,
    High,
    Critical,
}

/// System query types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SystemQuery {
    AgentStatus(String),
    TaskStatus(String),
    SystemHealth,
}

/// System helper for managing tasks and queries
#[derive(Debug, Clone)]
pub struct SystemHelper {
    task_templates: Arc<RwLock<Vec<String>>>,
    tasks: Arc<RwLock<Vec<Task>>>,
}

impl SystemHelper {
    /// Create a new system helper
    pub fn new() -> Result<Self, NexaError> {
        Ok(Self {
            task_templates: Arc::new(RwLock::new(Vec::new())),
            tasks: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// Create a new task from natural language description
    pub async fn create_task(&self, request: SystemTaskRequest) -> Result<Task, NexaError> {
        let task = Task::new(
            "Generated Task".to_string(),
            request.description,
            vec!["Initial step".to_string()],
            request.required_capabilities,
            request.deadline,
            3600, // Default 1 hour duration
            match request.priority {
                TaskPriority::Low => 0,
                TaskPriority::Medium => 1,
                TaskPriority::High => 2,
                TaskPriority::Critical => 3,
            },
        );

        // Store task
        let mut tasks = self.tasks.write().await;
        tasks.push(task.clone());
        info!("Created new task: {} (ID: {})", task.title, task.id);

        Ok(task)
    }

    /// Query system status and information
    pub async fn query_system(&self, query: SystemQuery) -> Result<String, NexaError> {
        match query {
            SystemQuery::AgentStatus(agent_id) => {
                info!("Querying agent status: {}", agent_id);
                Ok("Agent status query not implemented".to_string())
            }
            SystemQuery::TaskStatus(task_id) => {
                info!("Querying task status: {}", task_id);
                Ok("Task status query not implemented".to_string())
            }
            SystemQuery::SystemHealth => {
                info!("Querying system health");
                Ok("System health query not implemented".to_string())
            }
        }
    }

    /// Add a task template
    pub async fn add_task_template(&self, template: String) -> Result<(), NexaError> {
        let mut templates = self.task_templates.write().await;
        templates.push(template);
        Ok(())
    }

    /// Get task suggestions based on system state
    pub async fn suggest_tasks(&self) -> Result<Vec<SystemTaskRequest>, NexaError> {
        let templates = self.task_templates.read().await;
        let tasks = self.tasks.read().await;

        // Simple suggestion logic based on existing tasks and templates
        let mut suggestions = Vec::new();
        
        if tasks.is_empty() && !templates.is_empty() {
            // Suggest first task from template
            if let Some(template) = templates.first() {
                suggestions.push(SystemTaskRequest {
                    description: template.clone(),
                    priority: TaskPriority::Medium,
                    required_capabilities: vec!["basic".to_string()],
                    deadline: None,
                });
            }
        }

        Ok(suggestions)
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

    fn setup_test_helper() -> SystemHelper {
        SystemHelper::new().unwrap()
    }

    #[tokio::test]
    async fn test_system_query() {
        let helper = setup_test_helper();
        let response = helper.query_system(SystemQuery::SystemHealth).await;
        assert!(response.is_ok());
    }

    #[tokio::test]
    async fn test_create_task() {
        let helper = setup_test_helper();
        let request = SystemTaskRequest {
            description: "Test task".to_string(),
            priority: TaskPriority::Medium,
            required_capabilities: vec![],
            deadline: None,
        };
        let response = helper.create_task(request).await;
        assert!(response.is_ok());
    }

    #[tokio::test]
    async fn test_task_suggestions() {
        let helper = setup_test_helper();
        
        // Add some test templates
        helper.add_task_template("Monitor system resources".to_string()).await.unwrap();
        
        let suggestions = helper.suggest_tasks().await.unwrap();
        assert!(!suggestions.is_empty(), "Should suggest at least one task");
        
        for task in suggestions {
            assert!(!task.description.is_empty(), "Task description should not be empty");
            assert!(!task.required_capabilities.is_empty(), "Task should have required capabilities");
        }
    }
}