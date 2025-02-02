# Nexa Core Tutorial

This tutorial will guide you through using the Nexa Core system, including its LLM integration capabilities and cluster management features.

## Table of Contents
- [Installation](#installation)
- [Basic Usage](#basic-usage)
- [LLM Integration](#llm-integration)
- [Cluster Management](#cluster-management)
- [System Monitoring](#system-monitoring)

## Installation

```bash
# Install from source
cargo install --path .

# Or install from crates.io
cargo install nexa-core
```

## Basic Usage

Start the Nexa server:

```bash
nexa server start --port 8080
```

## LLM Integration

Nexa Core supports multiple LLM backends:

### LM Studio Integration

1. Start LM Studio server on port 1234 (default)
2. Configure Nexa to use LM Studio:

```rust
use nexa_core::llm::{LLMConfig, LLMClient};

let config = LLMConfig::with_lmstudio_server("http://localhost:1234")
    .with_cors_origins(vec!["http://your-frontend-domain".to_string()]);

let client = LLMClient::new(config)?;
let response = client.complete("Your prompt").await?;
```

### Ollama Integration

1. Install and start Ollama
2. Configure Nexa to use Ollama:

```rust
use nexa_core::llm::{LLMConfig, LLMClient};

let config = LLMConfig::with_ollama_server("model-name")  // e.g., "qwen2.5-coder:7b"
    .with_cors_origins(vec!["http://your-frontend-domain".to_string()]);

let client = LLMClient::new(config)?;
let response = client.complete("Your prompt").await?;
```

### System Helper Features

The system helper provides high-level functionality for task management and system queries:

```rust
use nexa_core::llm::system_helper::{SystemHelper, SystemQuery, SystemTaskRequest};

// Create system helper
let helper = SystemHelper::new(server_control)?;

// Create tasks
let task = helper.create_task(SystemTaskRequest {
    description: "Monitor system resources".to_string(),
    priority: TaskPriority::High,
    required_capabilities: vec!["monitoring".to_string()],
    deadline: None,
}).await?;

// Query system status
let health_status = helper.query_system(SystemQuery::Health).await?;
```

## Cluster Management

Nexa Core supports distributed operation through its cluster management features:

```rust
use nexa_core::mcp::{ClusterConfig, ClusterManager};

// Configure cluster
let config = ClusterConfig {
    node_id: "node-1".to_string(),
    bind_addr: "0.0.0.0:9000".to_string(),
    peers: vec!["node-2:9000".to_string()],
};

// Start cluster manager
let manager = ClusterManager::new(config)?;
manager.start().await?;
```

## System Monitoring

Monitor system health and performance:

```rust
use nexa_core::monitoring::{MonitoringSystem, HealthCheck};

// Initialize monitoring
let monitoring = MonitoringSystem::new()?;

// Configure health checks
monitoring.add_check(HealthCheck {
    name: "memory_usage".to_string(),
    threshold: 80.0,  // 80% threshold
});

// Get metrics
let metrics = monitoring.get_metrics().await?;
```

## Web Integration

Nexa Core supports web integration with CORS configuration:

```rust
use nexa_core::llm::LLMConfig;

let config = LLMConfig::default()
    .with_cors_origins(vec!["http://localhost:3000".to_string()])
    .with_credentials();

// The server will now accept requests from the specified origins
```

## Error Handling

Nexa Core provides robust error handling:

```rust
use nexa_core::error::NexaError;

match result {
    Ok(response) => {
        println!("Success: {}", response);
    }
    Err(NexaError::System(e)) => {
        eprintln!("System error: {}", e);
    }
    Err(NexaError::Cluster(e)) => {
        eprintln!("Cluster error: {}", e);
    }
    // ... handle other error types
}
```

## Version History

- **1.2.1**: Added Ollama support, improved CORS and JSON handling
- **1.2.0**: Added LM Studio integration and system helper
- **1.1.2**: Initial release with core functionality

## Contributing

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details. 