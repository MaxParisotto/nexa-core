use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::types::{Agent, Task, AgentStatus};
use crate::error::NexaError;

/// Registry for managing connected agents
#[derive(Debug, Clone)]
pub struct AgentRegistry {
    agents: Arc<RwLock<HashMap<String, Agent>>>,
    tasks: Arc<RwLock<HashMap<String, Task>>>,
}

impl AgentRegistry {
    pub fn new() -> Self {
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a new agent
    pub async fn register(&self, agent: Agent) -> Result<(), NexaError> {
        let mut agents = self.agents.write().await;
        if agents.contains_key(&agent.id) {
            return Err(NexaError::Agent("Agent already registered".to_string()));
        }
        agents.insert(agent.id.clone(), agent);
        Ok(())
    }

    /// Deregister an agent
    pub async fn deregister(&self, agent_id: &str) -> Result<(), NexaError> {
        let mut agents = self.agents.write().await;
        if agents.remove(agent_id).is_none() {
            return Err(NexaError::Agent("Agent not found".to_string()));
        }
        Ok(())
    }

    /// Get agent by ID
    pub async fn get_agent(&self, agent_id: &str) -> Result<Agent, NexaError> {
        let agents = self.agents.read().await;
        agents
            .get(agent_id)
            .cloned()
            .ok_or_else(|| NexaError::Agent("Agent not found".to_string()))
    }

    /// Update agent status
    pub async fn update_status(&self, agent_id: &str, status: AgentStatus) -> Result<(), NexaError> {
        let mut agents = self.agents.write().await;
        if let Some(agent) = agents.get_mut(agent_id) {
            agent.status = status;
            Ok(())
        } else {
            Err(NexaError::Agent("Agent not found".to_string()))
        }
    }

    /// List all registered agents
    pub async fn list_agents(&self) -> Vec<Agent> {
        let agents = self.agents.read().await;
        agents.values().cloned().collect()
    }

    /// Find agents by capability
    pub async fn find_by_capability(&self, capability: &str) -> Vec<Agent> {
        let agents = self.agents.read().await;
        agents
            .values()
            .filter(|agent| agent.has_capability(capability))
            .cloned()
            .collect()
    }

    pub async fn add_task(&self, task: Task) -> Result<(), NexaError> {
        let mut tasks = self.tasks.write().await;
        tasks.insert(task.id.clone(), task);
        Ok(())
    }

    pub async fn remove_task(&self, id: &str) -> Result<(), NexaError> {
        let mut tasks = self.tasks.write().await;
        tasks.remove(id);
        Ok(())
    }

    pub async fn get_task(&self, id: &str) -> Result<Task, NexaError> {
        let tasks = self.tasks.read().await;
        tasks
            .get(id)
            .cloned()
            .ok_or_else(|| NexaError::System(format!("Task not found: {}", id)))
    }

    pub async fn list_tasks(&self) -> Result<Vec<Task>, NexaError> {
        let tasks = self.tasks.read().await;
        Ok(tasks.values().cloned().collect())
    }

    pub async fn update_task(&self, task: Task) -> Result<(), NexaError> {
        let mut tasks = self.tasks.write().await;
        tasks.insert(task.id.clone(), task);
        Ok(())
    }

    pub async fn assign_task(&self, task_id: &str, agent_id: &str) -> Result<(), NexaError> {
        let mut tasks = self.tasks.write().await;
        let mut agents = self.agents.write().await;

        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| NexaError::System(format!("Task not found: {}", task_id)))?;

        let agent = agents
            .get_mut(agent_id)
            .ok_or_else(|| NexaError::System(format!("Agent not found: {}", agent_id)))?;

        task.assigned_agent = Some(agent_id.to_string());
        agent.current_task = Some(task_id.to_string());

        Ok(())
    }

    pub async fn unassign_task(&self, task_id: &str) -> Result<(), NexaError> {
        let mut tasks = self.tasks.write().await;
        let mut agents = self.agents.write().await;

        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| NexaError::System(format!("Task not found: {}", task_id)))?;

        if let Some(agent_id) = &task.assigned_agent {
            if let Some(agent) = agents.get_mut(agent_id) {
                agent.current_task = None;
            }
        }

        task.assigned_agent = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[tokio::test]
    async fn test_agent_registration() {
        let registry = AgentRegistry::new();
        let agent = Agent {
            id: "test-1".to_string(),
            name: "Test Agent".to_string(),
            capabilities: vec![],
            status: AgentStatus::Idle,
            current_task: None,
            last_heartbeat: Utc::now(),
        };

        assert!(registry.register(agent.clone()).await.is_ok());
        assert!(registry.deregister("test-1").await.is_ok());
    }
}
