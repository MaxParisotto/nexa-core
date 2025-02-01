#!/bin/bash

# Function to detect OS and architecture
detect_system() {
    local os
    local arch
    
    # Detect OS
    case "$(uname -s)" in
        Linux*)     os="linux";;
        Darwin*)    os="darwin";;
        MINGW*)     os="windows";;
        *)          os="unknown";;
    esac
    
    # Detect architecture
    case "$(uname -m)" in
        x86_64*)    arch="x86_64";;
        aarch64*)   arch="aarch64";;
        arm64*)     arch="aarch64";;
        *)          arch="unknown";;
    esac
    
    echo "${os}-${arch}"
}

# Function to set up build environment
setup_environment() {
    local system=$1
    
    echo "Setting up build environment for $system..."
    
    case $system in
        linux-*)
            # Install dependencies for Linux
            if command -v apt-get &> /dev/null; then
                sudo apt-get update
                sudo apt-get install -y build-essential pkg-config libssl-dev
            elif command -v yum &> /dev/null; then
                sudo yum groupinstall -y "Development Tools"
                sudo yum install -y openssl-devel
            fi
            ;;
        darwin-*)
            # Install dependencies for macOS
            if ! command -v brew &> /dev/null; then
                echo "Homebrew not found. Please install Homebrew first."
                exit 1
            fi
            brew install openssl pkg-config
            ;;
        windows-*)
            echo "Windows build environment setup not implemented"
            exit 1
            ;;
        *)
            echo "Unsupported system: $system"
            exit 1
            ;;
    esac
}

# Function to run build and tests
run_build_and_test() {
    local system=$1
    local target
    
    echo "Building for $system..."
    
    # Determine target based on system
    case $system in
        linux-x86_64)    target="x86_64-unknown-linux-gnu";;
        linux-aarch64)   target="aarch64-unknown-linux-gnu";;
        darwin-x86_64)   target="x86_64-apple-darwin";;
        darwin-aarch64)  target="aarch64-apple-darwin";;
        *)
            echo "Unsupported system for build: $system"
            exit 1
            ;;
    esac
    
    # Clean previous build
    cargo clean
    
    # Build with release profile
    echo "Building with target: $target"
    cargo build --target $target --release
    
    if [ $? -ne 0 ]; then
        echo "Build failed"
        exit 1
    fi
    
    # Run tests
    echo "Running tests..."
    cargo test --target $target -- --nocapture
    
    if [ $? -ne 0 ]; then
        echo "Tests failed"
        exit 1
    fi
}

# Main script execution
main() {
    # Detect system
    system=$(detect_system)
    echo "Detected system: $system"
    
    # Setup environment if needed
    if [ "$1" = "--setup" ]; then
        setup_environment "$system"
    fi
    
    # Run build and tests
    run_build_and_test "$system"
}

# Execute main function with all script arguments
main "$@" 