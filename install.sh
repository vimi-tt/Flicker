#!/bin/bash
# Flicker Installation Script
# Usage:
#   ./install.sh              # Build & install GUI + CLI
#   ./install.sh --cli-only   # Build & install lightweight CLI without GUI dependencies
#   ./install.sh --uninstall  # Remove Flicker from the system

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

print_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
print_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
print_warning() { echo -e "${YELLOW}[WARNING]${NC} $1"; }
print_error() { echo -e "${RED}[ERROR]${NC} $1"; }

CLI_ONLY=false
UNINSTALL=false

for arg in "$@"; do
    case "$arg" in
        --cli-only) CLI_ONLY=true ;;
        --uninstall) UNINSTALL=true ;;
        -h|--help)
            echo "Usage: ./install.sh [--cli-only | --uninstall]"
            exit 0
            ;;
        *)
            print_error "Unknown option: $arg"
            echo "Usage: ./install.sh [--cli-only | --uninstall]"
            exit 1
            ;;
    esac
done

if [ "$EUID" -eq 0 ]; then
    print_error "Please do not run this script as root"
    print_info "The script will ask for sudo password when needed"
    exit 1
fi

if [ "$UNINSTALL" = true ]; then
    print_info "Uninstalling Flicker..."
    sudo rm -f /usr/local/bin/flicker
    sudo rm -f /usr/share/applications/flicker.desktop
    sudo rm -f /usr/share/icons/hicolor/512x512/apps/flicker.png
    sudo gtk-update-icon-cache -f -t /usr/share/icons/hicolor 2>/dev/null || true
    sudo update-desktop-database /usr/share/applications 2>/dev/null || true
    print_success "Flicker uninstalled."
    exit 0
fi

echo "🔥 Flicker Installation Script"
echo "=============================="
echo ""

# Check Rust
print_info "Checking for Rust..."
if ! command -v cargo &> /dev/null; then
    print_warning "Rust not found. Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    # shellcheck disable=SC1091
    source "$HOME/.cargo/env"
    print_success "Rust installed successfully"
else
    RUST_VERSION=$(rustc --version | cut -d' ' -f2)
    print_success "Rust found: $RUST_VERSION"
fi

# Check Rust version
REQUIRED_VERSION="1.85.0"
CURRENT_VERSION=$(rustc --version | cut -d' ' -f2)
if [ "$(printf '%s\n' "$REQUIRED_VERSION" "$CURRENT_VERSION" | sort -V | head -n1)" != "$REQUIRED_VERSION" ]; then
    print_warning "Rust $REQUIRED_VERSION+ is recommended (found $CURRENT_VERSION). Updating..."
    rustup update
fi

# Check system build dependencies
if ! command -v cc &> /dev/null; then
    print_error "C compiler (cc/gcc) not found. Please install build-essential / base-devel."
    exit 1
fi

if [ "$CLI_ONLY" = false ]; then
    if ! command -v pkg-config &> /dev/null; then
        print_warning "pkg-config not found; GUI build may require libfontconfig, libxkbcommon, and gtk3 development headers."
    fi
fi

# Build Flicker
if [ "$CLI_ONLY" = true ]; then
    print_info "Building Flicker (CLI-only)..."
    cargo build --release --no-default-features
else
    print_info "Building Flicker (GUI + CLI, this may take a few minutes)..."
    cargo build --release
fi

if [ ! -f "target/release/flicker" ]; then
    print_error "Build failed. Binary not found."
    exit 1
fi

print_success "Build completed successfully"
BINARY_SIZE=$(du -h target/release/flicker | cut -f1)
print_info "Binary size: $BINARY_SIZE"

# Install binary
print_info "Installing Flicker to /usr/local/bin..."
sudo install -m 755 target/release/flicker /usr/local/bin/flicker

if [ "$CLI_ONLY" = false ]; then
    print_info "Installing Desktop entry and Icon..."
    sudo mkdir -p /usr/share/icons/hicolor/512x512/apps
    sudo install -m 644 "$SCRIPT_DIR/Flicker.png" /usr/share/icons/hicolor/512x512/apps/flicker.png
    sudo tee /usr/share/applications/flicker.desktop > /dev/null << 'EOF'
[Desktop Entry]
Name=Flicker
Comment=USB Bootable Drive Creator
Exec=/usr/local/bin/flicker
Icon=flicker
Terminal=false
Type=Application
Categories=Utility;System;
EOF
    sudo gtk-update-icon-cache -f -t /usr/share/icons/hicolor 2>/dev/null || true
    sudo update-desktop-database /usr/share/applications 2>/dev/null || true
fi

if command -v flicker &> /dev/null; then
    INSTALLED_VERSION=$(flicker --version 2>/dev/null || echo "unknown")
    print_success "Flicker installed successfully! ($INSTALLED_VERSION)"
else
    print_error "Installation verification failed"
    exit 1
fi

echo ""
echo "Quick Start:"
echo "  $ flicker                  # Launch GUI (if built with GUI support)"
echo "  $ flicker list -v          # List USB devices"
echo "  $ sudo flicker write --iso ubuntu.iso --device /dev/sdb --verify"
echo ""
print_info "To uninstall later, run: ./install.sh --uninstall"
