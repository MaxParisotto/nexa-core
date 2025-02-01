#!/bin/bash

# Exit on any error
set -e

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Add musl-cross to PATH if on macOS
if [[ "$(uname)" == "Darwin" ]]; then
    export PATH="/opt/homebrew/Cellar/musl-cross/0.9.9_2/bin:$PATH"
    # Set cross-compilation environment variables
    export CC_x86_64_unknown_linux_musl="/opt/homebrew/Cellar/musl-cross/0.9.9_2/bin/x86_64-linux-musl-gcc"
    export CXX_x86_64_unknown_linux_musl="/opt/homebrew/Cellar/musl-cross/0.9.9_2/bin/x86_64-linux-musl-g++"
    export AR_x86_64_unknown_linux_musl="/opt/homebrew/Cellar/musl-cross/0.9.9_2/bin/x86_64-linux-musl-ar"
    export CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER="/opt/homebrew/Cellar/musl-cross/0.9.9_2/bin/x86_64-linux-musl-gcc"
fi

# Check for required tools
check_dependencies() {
    local missing_deps=0

    # Check for musl cross-compiler on macOS
    if [[ "$(uname)" == "Darwin" ]] && ! command -v x86_64-linux-musl-gcc &> /dev/null; then
        echo -e "${YELLOW}Warning: Linux musl cross-compiler not found${NC}"
        echo -e "To install cross-compiler on macOS, run:"
        echo -e "    brew install FiloSottile/musl-cross/musl-cross"
        echo -e "If already installed, ensure it's in your PATH:"
        echo -e "    export PATH=\"/opt/homebrew/Cellar/musl-cross/0.9.9_2/bin:\$PATH\""
        missing_deps=1
    fi

    # Check for lipo on macOS
    if [[ "$(uname)" == "Darwin" ]] && ! command -v lipo &> /dev/null; then
        echo -e "${RED}Error: 'lipo' command not found${NC}"
        echo -e "This is required for creating universal macOS binaries"
        missing_deps=1
    fi

    if [ $missing_deps -eq 1 ]; then
        echo -e "\n${YELLOW}Please install missing dependencies and try again${NC}"
        exit 1
    fi
}

# Create release directory if it doesn't exist
mkdir -p release

echo -e "${GREEN}Checking dependencies...${NC}"
check_dependencies

echo -e "${GREEN}Building Nexa Utils for multiple platforms...${NC}"

# Add Rust target if not already added
echo -e "\n${GREEN}Ensuring Rust targets are installed...${NC}"
rustup target add x86_64-unknown-linux-musl
rustup target add x86_64-apple-darwin
rustup target add aarch64-apple-darwin

# Build for x86_64 Linux (musl)
echo -e "\n${GREEN}Building for x86_64 Linux (musl)...${NC}"
if [[ "$(uname)" == "Darwin" ]]; then
    CROSS_COMPILE=1 cargo build --release --target x86_64-unknown-linux-musl
else
    cargo build --release --target x86_64-unknown-linux-musl
fi
cp target/x86_64-unknown-linux-musl/release/nexa release/nexa-x86_64-linux

# Build for x86_64 macOS
echo -e "\n${GREEN}Building for x86_64 macOS...${NC}"
cargo build --release --target x86_64-apple-darwin
cp target/x86_64-apple-darwin/release/nexa release/nexa-x86_64-darwin

# Build for Apple Silicon
echo -e "\n${GREEN}Building for Apple Silicon...${NC}"
cargo build --release --target aarch64-apple-darwin
cp target/aarch64-apple-darwin/release/nexa release/nexa-aarch64-darwin

# Create universal binary for macOS
if [[ "$(uname)" == "Darwin" ]]; then
    echo -e "\n${GREEN}Creating universal macOS binary...${NC}"
    lipo -create \
        target/x86_64-apple-darwin/release/nexa \
        target/aarch64-apple-darwin/release/nexa \
        -output release/nexa-universal-darwin
fi

# Make binaries executable
chmod +x release/nexa-*

echo -e "\n${GREEN}Build complete! Binaries are in the release directory:${NC}"
ls -lh release/

echo -e "\n${GREEN}Build artifacts:${NC}"
echo -e "- release/nexa-x86_64-linux    (Linux x86_64 - statically linked with musl)"
echo -e "- release/nexa-x86_64-darwin   (macOS Intel)"
echo -e "- release/nexa-aarch64-darwin  (macOS Apple Silicon)"
if [[ "$(uname)" == "Darwin" ]]; then
    echo -e "- release/nexa-universal-darwin (macOS Universal Binary)"
fi 
