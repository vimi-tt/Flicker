#!/bin/bash
# Flicker Installation Script
# Usage: bash install.sh

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Functions
print_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if running as root
if [ "$EUID" -eq 0 ]; then 
    print_error "Please do not run this script as root"
    print_info "The script will ask for sudo password when needed"
    exit 1
fi

echo "🔥 Flicker Installation Script"
echo "=============================="
echo ""

# Check if Rust is installed
print_info "Checking for Rust..."
if ! command -v cargo &> /dev/null; then
    print_warning "Rust not found. Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
    print_success "Rust installed successfully"
else
    RUST_VERSION=$(rustc --version | cut -d' ' -f2)
    print_success "Rust found: $RUST_VERSION"
fi

# Check Rust version
print_info "Checking Rust version..."
REQUIRED_VERSION="1.70.0"
CURRENT_VERSION=$(rustc --version | cut -d' ' -f2)

if [ "$(printf '%s\n' "$REQUIRED_VERSION" "$CURRENT_VERSION" | sort -V | head -n1)" != "$REQUIRED_VERSION" ]; then
    print_error "Rust version $REQUIRED_VERSION or higher required"
    print_info "Current version: $CURRENT_VERSION"
    print_info "Updating Rust..."
    rustup update
fi

# Build Flicker
print_info "Building Flicker (this may take a few minutes)..."
cargo build --release

if [ ! -f "target/release/flicker" ]; then
    print_error "Build failed. Binary not found."
    exit 1
fi

print_success "Build completed successfully"

# Show binary info
BINARY_SIZE=$(du -h target/release/flicker | cut -f1)
print_info "Binary size: $BINARY_SIZE"

# Install to system
print_info "Installing Flicker to /usr/local/bin..."
sudo cp target/release/flicker /usr/local/bin/

# Verify installation
if command -v flicker &> /dev/null; then
    INSTALLED_VERSION=$(flicker --version 2>/dev/null || echo "unknown")
    print_success "Flicker installed successfully!"
    print_info "Version: $INSTALLED_VERSION"
else
    print_error "Installation verification failed"
    exit 1
fi

echo ""
echo "=============================="
print_success "Installation Complete! 🎉"
echo "=============================="
echo ""
echo "Quick Start:"
echo "  $ flicker list              # List USB devices"
echo "  $ flicker list -v           # Detailed list"
echo "  $ sudo flicker write --iso ubuntu.iso --device /dev/sdb"
echo ""
print_info "Note: Root privileges (sudo) required for writing to devices"
echo ""
