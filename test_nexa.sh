#!/bin/bash

# Enable error handling
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test counters
TESTS_TOTAL=0
TESTS_PASSED=0
TESTS_FAILED=0

# Port management
get_random_port() {
    local port
    while true; do
        # Get a random port number between 1024 and 65535
        port=$(( ((RANDOM<<15)|RANDOM) % 64512 + 1024 ))
        # Check if the port is available
        if ! nc -z 127.0.0.1 $port 2>/dev/null; then
            echo $port
            return 0
        fi
    done
}

cleanup_server() {
    local port=$1
    if [ -n "$port" ]; then
        local pid=$(lsof -i :$port -t 2>/dev/null)
        if [ -n "$pid" ]; then
            kill -9 $pid 2>/dev/null || true
        fi
    fi
    rm -f /tmp/nexa.pid /tmp/nexa.state /tmp/nexa.sock 2>/dev/null || true
}

# Wait for server to be in a specific state
wait_for_server_state() {
    local expected_state=$1
    local timeout=$2
    local start_time=$(date +%s)
    local current_time
    local elapsed_time

    while true; do
        current_time=$(date +%s)
        elapsed_time=$((current_time - start_time))
        
        if [ $elapsed_time -ge $timeout ]; then
            return 1
        fi

        local state=$($BIN_PATH status 2>/dev/null | grep "State:" || true)
        if [[ $state == *"$expected_state"* ]]; then
            return 0
        fi
        sleep 0.1
    done
}

# Logging function
log() {
    local level=$1
    shift
    local message=$@
    local timestamp=$(date '+%Y-%m-%d %H:%M:%S')
    case $level in
        "INFO")
            echo -e "${BLUE}[INFO]${NC} ${timestamp} - $message"
            ;;
        "SUCCESS")
            echo -e "${GREEN}[SUCCESS]${NC} ${timestamp} - $message"
            ;;
        "WARNING")
            echo -e "${YELLOW}[WARNING]${NC} ${timestamp} - $message"
            ;;
        "ERROR")
            echo -e "${RED}[ERROR]${NC} ${timestamp} - $message"
            ;;
    esac
}

# Function to run a test and check its result
run_test() {
    local test_name=$1
    local command=$2
    local expected_status=${3:-0}  # Default expected status is 0
    local retry_count=${4:-1}      # Default retry count is 1
    local retry_delay=${5:-1}      # Default retry delay is 1 second

    ((TESTS_TOTAL++))
    
    log "INFO" "Running test: ${test_name}"
    log "INFO" "Executing command: ${command}"
    
    local attempt=1
    local success=false
    
    while [ $attempt -le $retry_count ]; do
        if [ $attempt -gt 1 ]; then
            log "INFO" "Retry attempt $attempt of $retry_count"
            sleep $retry_delay
        fi
        
        # Run the command and capture both stdout and stderr
        local output
        if output=$($command 2>&1); then
            local status=$?
            if [ $status -eq $expected_status ]; then
                log "SUCCESS" "Test '${test_name}' passed"
                log "INFO" "Command output:\n${output}"
                ((TESTS_PASSED++))
                success=true
                break
            fi
        else
            local status=$?
            if [ $status -eq $expected_status ]; then
                log "SUCCESS" "Test '${test_name}' passed (expected failure)"
                log "INFO" "Command output:\n${output}"
                ((TESTS_PASSED++))
                success=true
                break
            fi
        fi
        ((attempt++))
    done
    
    if [ "$success" = false ]; then
        log "ERROR" "Test '${test_name}' failed - Expected status ${expected_status}, got ${status}"
        log "INFO" "Command output:\n${output}"
        ((TESTS_FAILED++))
        return 1
    fi
}

# Determine the target binary
if [ -f "./target/aarch64-apple-darwin/release/nexa" ]; then
    BIN_PATH=./target/aarch64-apple-darwin/release/nexa
elif [ -f "./target/aarch64-apple-darwin/debug/nexa" ]; then
    BIN_PATH=./target/aarch64-apple-darwin/debug/nexa
else
    echo "[ERROR] Nexa binary not found in release or debug builds."
    exit 1
fi

# Run the nex binary with appropriate parameters (for example, show version)
$BIN_PATH --version

log "INFO" "Starting Nexa binary tests"
log "INFO" "Binary location: ${BIN_PATH}"
log "INFO" "Binary version info:"
$BIN_PATH --version

# Test Suite 1: Basic Commands
log "INFO" "Running Basic Command Tests"
run_test "Help Command" "$BIN_PATH --help"
run_test "Version Command" "$BIN_PATH --version"
run_test "Invalid Command" "$BIN_PATH invalid_command" 2

# Test Suite 2: Server Lifecycle
log "INFO" "Running Server Lifecycle Tests"
run_test "Initial Status" "$BIN_PATH status"

# Get a random port for testing
TEST_PORT=$(get_random_port)
log "INFO" "Using port ${TEST_PORT} for tests"

# Clean up any existing server instances
cleanup_server $TEST_PORT

# Start the server and verify it's running
log "INFO" "Starting server..."
run_test "Start Server" "$BIN_PATH start --config 127.0.0.1:${TEST_PORT}" 0 3 2

# Wait for server to fully start
sleep 5

# Test Suite 3: Server Operations
log "INFO" "Running Server Operation Tests"
run_test "Status After Start" "$BIN_PATH status" 0 3 1

# Stop the server and verify it's stopped
log "INFO" "Stopping server..."
run_test "Stop Server" "$BIN_PATH stop" 0 3 2
sleep 2

# Clean up after stop
cleanup_server $TEST_PORT

# Test Suite 4: Error Handling
log "INFO" "Running Error Handling Tests"
run_test "Stop Non-Running Server" "$BIN_PATH stop" 1

# Test double start by running two separate commands
log "INFO" "Testing double start..."
# Get a new random port
TEST_PORT=$(get_random_port)
log "INFO" "Using port ${TEST_PORT} for double start test"

cleanup_server $TEST_PORT
$BIN_PATH start --config 127.0.0.1:${TEST_PORT} > /dev/null 2>&1
sleep 2
run_test "Double Start" "$BIN_PATH start --config 127.0.0.1:${TEST_PORT}" 1
$BIN_PATH stop > /dev/null 2>&1
sleep 2

# Clean up after tests
cleanup_server $TEST_PORT

# Test Suite 5: Final Status Check
log "INFO" "Running Final Status Check"
run_test "Final Status" "$BIN_PATH status"

# Summary
log "INFO" "Test suite completed"
log "INFO" "Results:"
log "INFO" "Total tests: ${TESTS_TOTAL}"
log "SUCCESS" "Tests passed: ${TESTS_PASSED}"
if [ $TESTS_FAILED -gt 0 ]; then
    log "ERROR" "Tests failed: ${TESTS_FAILED}"
else
    log "SUCCESS" "All tests passed!"
fi

# Print binary information
log "INFO" "Binary details:"
file "$BIN_PATH"
log "INFO" "Binary size: $(ls -lh "$BIN_PATH" | awk '{print $5}')"
log "INFO" "Binary permissions: $(ls -l "$BIN_PATH" | awk '{print $1}')"

# Check if binary is stripped
if nm "$BIN_PATH" 2>&1 | grep -q "no symbols"; then
    log "INFO" "Binary is stripped of debug symbols"
else
    log "WARNING" "Binary may contain debug symbols"
fi

# Exit with overall status
if [ $TESTS_FAILED -gt 0 ]; then
    exit 1
fi
exit 0 