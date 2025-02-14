use nexa_core::{
    startup::StartupManager,
    config::ServerConfig,
    monitoring::MonitoringSystem,
    mcp::cluster::ClusterManager,
    error::NexaError,
    tokens::TokenManager,
};
use std::sync::Arc;
use tokio;
use std::net::SocketAddr;

async fn create_test_components() -> (ServerConfig, Arc<MonitoringSystem>, Arc<ClusterManager>) {
    let config = ServerConfig::default();
    let token_manager = Arc::new(TokenManager::new());
    let monitoring = Arc::new(MonitoringSystem::new(token_manager));
    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    let cluster = Arc::new(ClusterManager::new(addr, None));
    (config, monitoring, cluster)
}

#[tokio::test]
async fn test_startup_sequence() -> Result<(), NexaError> {
    let (config, monitoring, cluster) = create_test_components().await;

    // Create startup manager
    let startup_manager = StartupManager::new(
        config,
        monitoring,
        cluster,
    );

    // Run startup sequence
    startup_manager.run_startup_sequence().await?;

    Ok(())
}

#[tokio::test]
async fn test_system_requirements() -> Result<(), NexaError> {
    let (config, monitoring, cluster) = create_test_components().await;
    let startup_manager = StartupManager::new(config, monitoring, cluster);

    // Check system requirements
    startup_manager.check_system_requirements().await?;

    Ok(())
}

#[tokio::test]
async fn test_configuration_check() -> Result<(), NexaError> {
    let (config, monitoring, cluster) = create_test_components().await;
    let startup_manager = StartupManager::new(config, monitoring, cluster);

    // Check configuration
    startup_manager.check_configuration().await?;

    Ok(())
}

#[tokio::test]
async fn test_api_endpoints() -> Result<(), NexaError> {
    let (config, monitoring, cluster) = create_test_components().await;
    let startup_manager = StartupManager::new(config, monitoring, cluster);

    // Check API endpoints
    startup_manager.check_api_endpoints().await?;

    Ok(())
}

#[tokio::test]
async fn test_cluster_connectivity() -> Result<(), NexaError> {
    let (config, monitoring, cluster) = create_test_components().await;
    let startup_manager = StartupManager::new(config, monitoring, cluster);

    // Check cluster connectivity
    startup_manager.check_cluster_connectivity().await?;

    Ok(())
}

#[tokio::test]
async fn test_resource_availability() -> Result<(), NexaError> {
    let (config, monitoring, cluster) = create_test_components().await;
    let startup_manager = StartupManager::new(config, monitoring, cluster);

    // Check resource availability
    startup_manager.check_resource_availability().await?;

    Ok(())
} 