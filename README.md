# Nexa Core

## Overview

Nexa Core is a comprehensive Rust-based utility library providing essential tools and functionalities for multi-agent systems, resource management, and monitoring. The project emphasizes reliability, performance, and safety through robust resource tracking and health monitoring.

## Key Features

### Memory Management

- Resource allocation tracking
- Memory usage monitoring
- Cache management
- Resource pooling

### Token Management

- Token usage tracking per model
- Cost calculation and analytics
- Rate limiting
- Usage optimization

### Multi-agent Control Protocol (MCP)

- WebSocket-based communication with automatic reconnection
- Protocol validation and message handling
- Secure connection management
- Real-time status updates and metrics

### Monitoring System

- Real-time metrics collection (CPU, Memory, Network)
- Health checks with configurable thresholds
- Alert system with multiple severity levels
- Resource tracking and usage optimization

## Project Structure

src/
├── agent/          # Agent management
├── cli/            # Command-line interface
├── error/          # Error handling
├── mcp/            # Multi-agent Control Protocol
│   ├── mod.rs
│   ├── protocol.rs
│   ├── registry.rs
│   └── server.rs
├── memory/         # Memory management
├── monitoring/     # System monitoring
├── tokens/         # Token management
└── utils/          # Utility functions

## Setup Instructions

### Prerequisites

- Rust (>=1.70)
- Cargo

### Installation

```bash
# Clone the repository
git clone https://github.com/actualusername/nexa-utils.git
cd nexa-utils

# Build the project
cargo build --release

# Run tests
cargo test
```

## Usage Examples

### Starting the MCP Server

```rust
use nexa_utils::mcp::ServerControl;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = ServerControl::new();
    server.start(Some("127.0.0.1:8080")).await?;
    Ok(())
}
```

### Connecting via WebSocket

```rust
use tokio_tungstenite::connect_async;
use futures::{SinkExt, StreamExt};

async fn connect_to_server() -> Result<(), Box<dyn std::error::Error>> {
    let url = "ws://127.0.0.1:8080";
    let (mut ws_stream, _) = connect_async(url).await?;
    
    // Send a message
    let message = serde_json::json!({
        "type": "status",
        "agent_id": "test-agent",
        "status": "Running"
    });
    ws_stream.send(Message::Text(message.to_string())).await?;
    
    // Receive response
    if let Some(msg) = ws_stream.next().await {
        println!("Received: {}", msg?);
    }
    
    Ok(())
}
```

### Monitoring System Resources

```rust
use nexa_utils::monitoring::MonitoringSystem;
use std::sync::Arc;

async fn monitor_resources() -> Result<(), Box<dyn std::error::Error>> {
    let monitoring = Arc::new(MonitoringSystem::new(
        memory_manager.clone(),
        token_manager.clone()
    ));
    
    let metrics = monitoring.collect_metrics(0).await?;
    println!("CPU Usage: {:.2}%", metrics.cpu_usage * 100.0);
    println!("Memory Usage: {:?}", metrics.memory_usage);
    
    Ok(())
}
```

### Managing Token Usage

```rust
use nexa_utils::tokens::{TokenManager, ModelType};

async fn track_tokens() -> Result<(), Box<dyn std::error::Error>> {
    let token_manager = TokenManager::new(memory_manager.clone());
    
    // Define metadata
    let metadata = "example_metadata"; // Replace with actual metadata as needed
    
    token_manager.track_usage(
        ModelType::GPT4,
        100, // prompt tokens
        50,  // completion tokens
        metadata
    ).await?;
    
    Ok(())
}
```

## Configuration

The system can be configured through:

1. Command-line arguments
2. Environment variables
3. Configuration files

### Environment Variables

- `NEXA_LOG_LEVEL`: Set logging level (default: INFO)
- `NEXA_MAX_CONNECTIONS`: Maximum concurrent connections (default: 1000)
- `NEXA_HEALTH_CHECK_INTERVAL`: Health check interval in seconds (default: 30)
- `NEXA_CONNECTION_TIMEOUT`: Connection timeout in seconds (default: 30)

### Configuration File Example

```yaml
server:
  host: "127.0.0.1"
  port: 8080
  max_connections: 1000
  health_check_interval: 30
  connection_timeout: 30

monitoring:
  cpu_threshold: 80
  memory_threshold: 90
  alert_interval: 60

logging:
  level: "debug"
  file: "/var/log/nexa/server.log"
```

## Building and Testing

The project includes a script that automatically detects your operating system and architecture, then builds and runs the tests accordingly.

### Quick Start

To build and run tests:

```bash
./scripts/build_and_test.sh
```

### First Time Setup

If this is your first time building the project, run:

```bash
./scripts/build_and_test.sh --setup
```

This will install necessary dependencies based on your operating system:

- On Linux: Installs build-essential, pkg-config, and libssl-dev (via apt) or Development Tools and openssl-devel (via yum)
- On macOS: Installs openssl and pkg-config via Homebrew
- On Windows: Not currently supported

### Supported Platforms

The build script supports the following platforms:

- Linux (x86_64, aarch64)
- macOS (x86_64, arm64/M1)

### Build Output

The script will:

1. Detect your system architecture
2. Clean any previous builds
3. Build the project in release mode
4. Run all tests with detailed output

If any step fails, the script will exit with a non-zero status code and display an error message.

## Development

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_name

# Run with logging
RUST_LOG=debug cargo test
```

### Code Style

- Follow Rust standard practices
- Use `cargo fmt` for formatting
- Run `cargo clippy` for linting

## Contributing

Please read CONTRIBUTING.md for details on our code of conduct and the process for submitting pull requests.

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Contact

For questions or support:

- GitHub Issues: [Project Issues](https://github.com/yourusername/nexa-utils/issues)
- Email: <support@nexa-utils.com>

A command-line interface for managing AI agents and workflows.

# Installation

```bash
cargo install --path .
```

## Usage

```bash
# Start the server
nexa-core start [--port <PORT>]

# Stop the server
nexa-core stop

# Check server status
nexa-core status

# List agents
nexa-core agents [--status <STATUS>]

# Create a new agent
nexa-core create-agent --name <NAME> [--model <MODEL>] [--provider <PROVIDER>]

# Stop an agent
nexa-core stop-agent --id <ID>

# List available models
nexa-core models --provider <PROVIDER>

# Add LLM server
nexa-core add-server --provider <PROVIDER> --url <URL>

# Remove LLM server
nexa-core remove-server --provider <PROVIDER>

# Create a task
nexa-core create-task --description <DESC> [--priority <PRIORITY>] [--agent-id <ID>]

# List tasks
nexa-core tasks

# List workflows
nexa-core workflows

# Create a workflow
nexa-core create-workflow --name <NAME> --steps <STEPS>

# Execute a workflow
nexa-core execute-workflow --id <ID>
```

## Environment Variables

- `RUST_LOG`: Set logging level (e.g., `info`, `debug`, `trace`)
- `NEXA_PORT`: Default port for the server (default: 8080)

```bash
# Build the project
cargo build

# Run tests
cargo test

# Run with logging
RUST_LOG=info cargo run -- [COMMAND]
```
