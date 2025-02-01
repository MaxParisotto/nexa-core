# Nexa Core v1.0.0 Tutorial

This tutorial will guide you through testing all features of the Nexa Core WebSocket server using command-line tools.

## Prerequisites

Install these tools before starting:
```bash
# Install websocat for WebSocket testing
curl -L https://github.com/vi/websocat/releases/download/v1.11.0/websocat.x86_64-unknown-linux-musl -o websocat
chmod +x websocat
sudo mv websocat /usr/local/bin/

# Install jq for JSON formatting
sudo apt-get install jq
```

## 1. Basic Server Operations

### Starting the Server
```bash
# Start the server on default port
./target/release/nexa start

# Start on specific port
./target/release/nexa start --bind-addr 127.0.0.1:8080

# Start with custom settings
NEXA_MAX_CONNECTIONS=2000 NEXA_TIMEOUT=60 ./target/release/nexa start
```

### Checking Server Status
```bash
# Get server status
./target/release/nexa status

# Get detailed metrics
./target/release/nexa metrics
```

### Stopping the Server
```bash
# Graceful shutdown
./target/release/nexa stop

# Force stop (if needed)
./target/release/nexa stop --force
```

## 2. Testing WebSocket Connections

### Basic Connection Test
```bash
# Open a WebSocket connection
websocat ws://127.0.0.1:8080
```

### Send Test Messages
```bash
# Send a simple JSON message
echo '{"type":"test","message":"hello"}' | websocat ws://127.0.0.1:8080

# Expected response:
# {"status":"success","code":200}
```

### Testing Concurrent Connections
```bash
# Open multiple connections (replace 8080 with your port)
for i in {1..5}; do
    echo "Connection $i"
    echo '{"type":"test","connection_id":'$i',"message":"hello"}' | \
    websocat ws://127.0.0.1:8080 &
done
```

### Testing Binary Messages
```bash
# Send binary data
echo -n "binary test" | base64 | websocat ws://127.0.0.1:8080 --binary

# Expected response:
# {"status":"success","code":200,"size":10}
```

## 3. Connection Management Tests

### Test Connection Timeout
```bash
# Start server with short timeout
NEXA_TIMEOUT=5 ./target/release/nexa start

# Open connection and wait
websocat ws://127.0.0.1:8080
# Connection should close after 5 seconds of inactivity
```

### Test Maximum Connections
```bash
# Start server with low connection limit
NEXA_MAX_CONNECTIONS=2 ./target/release/nexa start

# Try to open more connections than allowed
for i in {1..3}; do
    echo "Connection $i"
    websocat ws://127.0.0.1:8080 &
done
# Third connection should be rejected
```

## 4. State Management Tests

### Test State Transitions
```bash
# Monitor state changes
watch -n 1 './target/release/nexa status'

# In another terminal, perform operations:
./target/release/nexa start
# Watch state change: Stopped -> Starting -> Running

./target/release/nexa stop
# Watch state change: Running -> Stopping -> Stopped
```

### Test Crash Recovery
```bash
# Start server
./target/release/nexa start

# Simulate crash (replace PID)
kill -9 $(pgrep nexa)

# Check state file
cat /tmp/nexa-*.state

# Restart server
./target/release/nexa start
```

## 5. Monitoring Tests

### Check Metrics
```bash
# Get current metrics
./target/release/nexa metrics

# Monitor metrics in real-time
watch -n 1 './target/release/nexa metrics'
```

### Test Connection Statistics
```bash
# In terminal 1: Start monitoring
watch -n 1 './target/release/nexa metrics'

# In terminal 2: Generate connections
for i in {1..10}; do
    echo "Connection $i"
    echo '{"type":"test"}' | websocat ws://127.0.0.1:8080
    sleep 1
done
```

## 6. Cleanup and Maintenance

### Clean Stale Files
```bash
# Remove PID and state files
rm /tmp/nexa-*.pid /tmp/nexa-*.state

# Verify cleanup
ls -la /tmp/nexa-*
```

### Check Server Logs
```bash
# Run server with debug logging
RUST_LOG=debug ./target/release/nexa start

# Run with trace logging for detailed information
RUST_LOG=trace ./target/release/nexa start
```

## Troubleshooting

### Common Issues

1. **Server Won't Start**
   ```bash
   # Check for existing instances
   ps aux | grep nexa
   
   # Check for stale files
   ls -la /tmp/nexa-*
   
   # Remove stale files if needed
   rm /tmp/nexa-*
   ```

2. **Connection Issues**
   ```bash
   # Check if server is listening
   netstat -tulpn | grep nexa
   
   # Test port availability
   nc -zv 127.0.0.1 8080
   ```

3. **Performance Issues**
   ```bash
   # Check system resources
   top -p $(pgrep nexa)
   
   # Monitor connection count
   watch -n 1 './target/release/nexa metrics | grep connections'
   ```

### Debug Mode
```bash
# Run with full debug output
RUST_LOG=debug,nexa_utils=trace ./target/release/nexa start
```

## Advanced Usage

### Custom Message Formats
```bash
# Send structured data
echo '{
    "type": "custom",
    "data": {
        "action": "test",
        "payload": {"key": "value"}
    }
}' | websocat ws://127.0.0.1:8080
```

### Load Testing
```bash
# Simple load test script
for i in {1..100}; do
    echo "Connection $i"
    echo '{"type":"test","id":'$i'}' | \
    websocat ws://127.0.0.1:8080 --async &
    sleep 0.1
done
```

---
*Note: Replace port numbers (8080) with your actual server port if different.* 