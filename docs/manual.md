# Nexa Utils Manual

## Table of Contents

1. [Introduction](#introduction)
2. [Installation](#installation)
3. [Getting Started](#getting-started)
4. [Core Features](#core-features)
5. [API Reference](#api-reference)
6. [Configuration](#configuration)
7. [Troubleshooting](#troubleshooting)
8. [Best Practices](#best-practices)

## Introduction

Nexa Utils is a powerful Multi-agent Control Protocol (MCP) implementation that provides robust agent management, resource monitoring, and task distribution capabilities. This manual covers all aspects of using and integrating with Nexa Utils.

## Installation

### Prerequisites

- Rust (>=1.70)
- Cargo

### Building from Source

```bash
git clone https://github.com/actualusername/nexa-utils.git
cd nexa-utils
cargo build --release
```

## Getting Started

### Basic Usage

1. Start the server:

   ```bash
   ./target/release/nexa start --config 127.0.0.1:8080
   ```

2. Check server status:

   ```bash
   ./target/release/nexa status
   ```

3. Stop the server:

   ```bash
   ./target/release/nexa stop
   ```

### Configuration Options

- Port configuration (1024-65535)
- Memory limits
- Token usage thresholds
- Logging levels

## Core Features

### 1. Agent Management

```rust
// Register a new agent
MCPMessage::RegisterAgent {
    agent: Agent {
        id: "agent-1",
        capabilities: ["code_generation", "test_generation"],
        status: AgentStatus::Idle
    }
}

// Query agents by capability
MCPMessage::AgentQuery {
    capability: "code_generation"
}
```

### 2. Task Management

- Code Generation Tasks
- Code Review Tasks
- Test Generation Tasks
- Custom Task Types

### 3. Resource Monitoring

- Real-time CPU usage
- Memory allocation tracking
- Token usage monitoring
- Health checks

### 4. WebSocket Communication

```python
# Python example using websockets
import websockets
import json

async def connect_agent():
    uri = "ws://localhost:8080"
    async with websockets.connect(uri) as websocket:
        # Register agent
        registration = {
            "type": "RegisterAgent",
            "agent": {
                "id": "agent-1",
                "capabilities": ["code_generation"]
            }
        }
        await websocket.send(json.dumps(registration))
```

## API Reference

### WebSocket Messages

#### Registration

```json
{
    "type": "RegisterAgent",
    "agent": {
        "id": "string",
        "name": "string",
        "capabilities": ["string"],
        "status": "Idle|Running|Error"
    }
}
```

#### Task Assignment

```json
{
    "type": "TaskAssignment",
    "task": {
        "id": "string",
        "type": "string",
        "data": {},
        "deadline": "ISO8601 timestamp"
    }
}
```

#### Status Updates

```json
{
    "type": "StatusUpdate",
    "agent_id": "string",
    "status": "Idle|Running|Error",
    "metrics": {
        "cpu_usage": "float",
        "memory_usage": "float"
    }
}
```

### CLI Commands

| Command | Description | Options |
|---------|-------------|----------|
| start   | Start server | --config <addr:port> |
| stop    | Stop server | None |
| status  | Show status | None |

## Configuration

### Server Configuration

```toml
# config.toml
[server]
host = "127.0.0.1"
port = 8080
max_connections = 1000

[memory]
max_usage_mb = 4096
cache_size_mb = 512

[tokens]
rate_limit = 100000
cost_threshold = 10.0
```

### Logging Configuration

```toml
[logging]
level = "info"
file = "nexa.log"
format = "json"
```

## Troubleshooting

### Common Issues

1. Connection Refused

   ```
   Error: Connection refused (os error 61)
   Solution: Check if the server is running and the port is available
   ```

2. Memory Allocation Failed

   ```
   Error: Failed to allocate memory
   Solution: Check system resources and memory limits
   ```

3. Token Rate Limit Exceeded

   ```
   Error: Rate limit exceeded
   Solution: Adjust rate limits or wait for reset
   ```

### Debugging

1. Enable Debug Logging

   ```bash
   RUST_LOG=debug ./target/release/nexa start
   ```

2. Check System Metrics

   ```bash
   ./target/release/nexa status
   ```

3. Monitor WebSocket Connections

   ```bash
   # Using websocat
   websocat ws://localhost:8080
   ```

## Best Practices

### Security

1. Use secure WebSocket (WSS) in production
2. Implement authentication
3. Regular security audits
4. Keep dependencies updated

### Performance

1. Monitor resource usage
2. Implement rate limiting
3. Use connection pooling
4. Regular performance testing

### Development

1. Write comprehensive tests
2. Follow Rust best practices
3. Document code changes
4. Use semantic versioning

### Deployment

1. Use containerization
2. Implement health checks
3. Set up monitoring
4. Regular backups

## Contributing

Please read CONTRIBUTING.md for details on our code of conduct and the process for submitting pull requests.

## License

This project is licensed under the MIT License - see the LICENSE file for details.
