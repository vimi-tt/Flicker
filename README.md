# 🔥 Flicker

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![Platform](https://img.shields.io/badge/platform-Linux-blue.svg)](https://www.linux.org/)

> A modern, fast, and safe USB bootable drive creator for Linux, written in Rust 🦀

Flicker is a Rufus alternative for Linux that allows you to easily create bootable USB drives from ISO images. Built with Rust for maximum safety, performance, and reliability.

## ✨ Features

- 🔍 **Smart USB Detection** - Automatically detects and lists USB devices
- 🚀 **Fast Writing** - Optimized 4MB chunk writes for maximum performance
- 🛡️ **Safety First** - Multiple validations and confirmations to prevent data loss
- 📊 **Real-time Progress** - Beautiful progress bars with ETA and speed
- ✅ **Data Verification** - Optional byte-by-byte verification after writing
- 🔓 **Auto Unmount** - Automatically unmounts devices before writing
- 💻 **CLI Interface** - Simple and intuitive command-line interface
- ⚡ **Zero Dependencies** - Single binary, no external tools required

## 📦 Installation

### From Source

```bash
# Clone the repository
git clone https://github.com/vimi-tt/Flicker.git
cd Flicker

# Build release version
cargo build --release

# Install to system
sudo cp target/release/flicker /usr/local/bin/

# Verify installation
flicker --version
```

### Requirements

- Rust 1.70 or higher
- Linux kernel 3.0+
- Root privileges (for writing to devices)

## 🚀 Quick Start

### 1. List available USB devices

```bash
flicker list
```

Output:
```
📀 Available USB devices:

  /dev/sdb - SanDisk Ultra (14.92 GB)
  /dev/sdc - Kingston DataTraveler (29.84 GB)
```

### 2. Write ISO to USB

```bash
sudo flicker write --iso ubuntu-24.04.iso --device /dev/sdb
```

### 3. Write with verification (recommended)

```bash
sudo flicker write --iso ubuntu-24.04.iso --device /dev/sdb --verify
```

## 📖 Usage

### Commands

#### `list` - List USB devices

```bash
# Simple list
flicker list

# Detailed information
flicker list --verbose
flicker list -v
```

#### `write` - Write ISO to USB

```bash
sudo flicker write --iso <ISO_FILE> --device <DEVICE> [DEVICE...] [OPTIONS]
```

**Options:**
- `--iso, -i <FILE>` - Path to ISO file (required)
- `--device, -d <DEVICE>...` - Target device path(s) (required, supports multiple devices)
- `--verify, -v` - Verify data after writing
- `--resume, -r` - Resume an interrupted write without starting over
- `--yes, -y` - Skip confirmation prompts

**Examples:**

```bash
# Basic write
sudo flicker write --iso debian.iso --device /dev/sdb

# Write to multiple devices simultaneously
sudo flicker write --iso ubuntu.iso --device /dev/sdb /dev/sdc /dev/sdd

# Resume an interrupted write
sudo flicker write --iso ubuntu.iso --device /dev/sdb --resume

# With verification (recommended for important data)
sudo flicker write --iso ubuntu.iso --device /dev/sdb --verify

# Skip confirmation (use with caution!)
sudo flicker write --iso arch.iso --device /dev/sdb --yes

# Short flags
sudo flicker write -i ubuntu.iso -d /dev/sdb -v
```

#### `verify` - Verify ISO checksum

```bash
flicker verify --iso <FILE> [--checksum <HASH>] [--algorithm <ALGO>]
```

**Options:**
- `--iso, -i <FILE>` - Path to ISO file (required)
- `--checksum, -c <HASH>` - Expected checksum to verify against
- `--algorithm, -a <ALGO>` - Checksum algorithm to use (sha256 or md5, default: sha256)

### Complete Workflow Example

```bash
# Step 1: List devices to identify your USB
flicker list -v

# Step 2: Write ISO with verification
sudo flicker write \
    --iso ~/Downloads/ubuntu-24.04-desktop-amd64.iso \
    --device /dev/sdb \
    --verify

# Step 3: Done! Safely remove your USB
```

## 🔒 Safety Features

Flicker includes multiple safety checks to prevent accidental data loss:

1. **Root Permission Check** - Ensures proper privileges
2. **ISO Validation** - Verifies file exists and is valid
3. **Device Validation** - Confirms device exists and is a block device
4. **USB Detection** - Warns if target is not a removable device
5. **Size Check** - Ensures ISO fits on device
6. **Mount Check** - Auto-unmounts mounted partitions
7. **Confirmation Prompts** - Multiple confirmations before writing
8. **Verification** - Optional byte-by-byte verification

## 🛠️ Development

### Project Structure

```
flicker/
├── src/
│   ├── main.rs          # Entry point
│   ├── cli/             # CLI argument parsing
│   │   └── mod.rs
│   ├── usb/             # USB device detection
│   │   └── mod.rs
│   ├── writer/          # ISO writing logic
│   │   └── mod.rs
│   └── utils.rs         # Helper functions
├── docs/                # Documentation
├── Cargo.toml           # Project configuration
└── README.md
```

### Building from Source

```bash
# Development build (faster compilation)
cargo build

# Release build (optimized)
cargo build --release

# Run tests
cargo test

# Run with logging
RUST_LOG=debug cargo run -- list -v

# Check code without building
cargo check

# Format code
cargo fmt

# Run linter
cargo clippy
```

## 📋 Roadmap

### ✅ Beta 1 (Current)
- [x] USB device detection
- [x] ISO writing with progress
- [x] Data verification
- [x] Auto-unmounting
- [x] Safety validations

### 🚧 Beta 2 (Planned)
- [x] ISO checksum verification (SHA256, MD5)
- [x] Multi-device support (write to multiple USBs)
- [x] Resume interrupted writes
- [x] GUI version (optional)

## 🐛 Troubleshooting

### "Permission denied"
```bash
# Always use sudo for writing
sudo flicker write --iso file.iso --device /dev/sdb
```

### "Device not found"
```bash
# List devices to find correct path
flicker list -v

# Check with system tools
lsblk
```

### "ISO file not found"
```bash
# Use absolute path
sudo flicker write --iso /home/user/Downloads/ubuntu.iso --device /dev/sdb

# Or relative with ~
sudo flicker write --iso ~/Downloads/ubuntu.iso --device /dev/sdb
```

### "Verification failed"
Possible causes:
- Faulty USB device
- USB disconnected during write
- Bad ISO file

Solutions:
- Try another USB device
- Verify ISO checksum
- Re-download ISO file

For more help, see the detailed documentation:
- [CLI Write Command](docs/COMMAND_WRITE.md)
- [Graphical Interface (GUI)](docs/GUI_INTERFACE.md)

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- Inspired by [Rufus](https://rufus.ie/) (Windows)
- Built with [Rust](https://www.rust-lang.org/) 🦀
- Uses [clap](https://github.com/clap-rs/clap) for CLI parsing
- Uses [indicatif](https://github.com/console-rs/indicatif) for progress bars

---

*Flicker - Making bootable USB creation simple, fast, and safe on Linux*