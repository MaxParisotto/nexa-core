use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::agent::{Agent, AgentStatus};
use crate::error::NexaError;

/// Registry for managing connected agents
#[derive(Debug, Clone)]
pub struct AgentRegistry {
    agents: Arc<RwLock<HashMap<String, Agent>>>,
}

impl AgentRegistry {
    pub fn new() -> Self {
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a new agent
    pub async fn register(&self, agent: Agent) -> Result<(), NexaError> {
        let mut agents = self.agents.write().await;
        if agents.contains_key(&agent.id) {
            return Err(NexaError::agent("Agent already registered"));
        }
        agents.insert(agent.id.clone(), agent);
        Ok(())
    }

    /// Deregister an agent
    pub async fn deregister(&self, agent_id: &str) -> Result<(), NexaError> {
        let mut agents = self.agents.write().await;
        if agents.remove(agent_id).is_none() {
            return Err(NexaError::agent("Agent not found"));
        }
        Ok(())
    }

    /// Get agent by ID
    pub async fn get_agent(&self, agent_id: &str) -> Result<Agent, NexaError> {
        let agents = self.agents.read().await;
        agents
            .get(agent_id)
            .cloned()
            .ok_or_else(|| NexaError::agent("Agent not found"))
    }

    /// Update agent status
    pub async fn update_status(&self, agent_id: &str, status: AgentStatus) -> Result<(), NexaError> {
        let mut agents = self.agents.write().await;
        if let Some(agent) = agents.get_mut(agent_id) {
            agent.status = status;
            Ok(())
        } else {
            Err(NexaError::agent("Agent not found"))
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
            .filter(|agent| agent.capabilities.iter().any(|cap| cap.matches_type(capability)))
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentStatus;

    #[tokio::test]
    async fn test_agent_registration() {
        let registry = AgentRegistry::new();
        let agent = Agent {
            id: "test-1".to_string(),
            name: "Test Agent".to_string(),
            capabilities: vec![],
            status: AgentStatus::Idle,
            current_tasks: vec![],
        };

        assert!(registry.register(agent.clone()).await.is_ok());
        assert!(registry.deregister("test-1").await.is_ok());
    }
}
