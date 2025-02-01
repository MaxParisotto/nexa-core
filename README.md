# Nexa Utils

## Overview

Nexa Utils is a comprehensive Rust-based utility library providing essential tools and functionalities for multi-agent systems, resource management, and monitoring. The project emphasizes reliability, performance, and safety through robust resource tracking and health monitoring.

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

- WebSocket-based communication
- Protocol validation
- Message handling
- Connection management

### Monitoring System

- Real-time metrics collection
- Health checks
- Alert system
- Resource tracking

## Project Structure

```
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
```

## Setup Instructions

### Prerequisites

- Rust (>=1.70)
- Cargo

### Installation

```bash
# Clone the repository
git clone https://github.com/yourusername/nexa-utils.git
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

Example configuration:

```toml
[server]
host = "127.0.0.1"
port = 8080

[monitoring]
interval = 1000  # milliseconds
cpu_threshold = 80.0  # percentage
memory_threshold = 90.0  # percentage

[tokens]
rate_limit = 100000  # tokens per minute
cost_tracking = true
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

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Submit a pull request

## License

MIT License - See LICENSE file for details

## Contact

For questions or support:

- GitHub Issues: [Project Issues](https://github.com/yourusername/nexa-utils/issues)
- Email: <support@nexa-utils.com>
