# Flicker

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![Platform](https://img.shields.io/badge/platform-Linux-blue.svg)](https://www.linux.org/)
[![Release](https://img.shields.io/badge/release-v1.0.0-blue.svg)](https://github.com/vimi-tt/Flicker/releases)

> A modern, fast, and safe USB bootable drive creator for Linux, written in Rust 🦀

Flicker is a Rufus alternative for Linux that allows you to easily create bootable USB drives from ISO images. Built with Rust for maximum safety, performance, and reliability.

---

## ✨ Features

- 🔍 **Smart USB Detection** - Automatically detects and lists removable USB drives safely
- 🚀 **High-Performance Writing** - Optimized 4MB chunk buffered writes with minimal CPU usage
- 🛡️ **Safety First** - Device validation, size checks, auto-unmount, and modal confirmation dialogs
- 🎨 **GNOME Adwaita Design** - Clean HIG interface with boxed lists, compact header bar, and Material You dynamic color palette
- 🌓 **Adaptive System Theming** - Automatic Light/Dark mode and accent color detection via desktop portal
- 📦 **Official AppImage** - Standalone, single-file executable that opens directly with a double-click
- 📊 **Real-time Metrics** - Slim progress bar with percentage, transfer speed, and ETA
- ✅ **Data Verification** - Optional byte-by-byte verification after writing
- 💻 **Complete CLI Mode** - Full command-line interface (`list`, `write`, `verify`) for headless systems and scripts
- ⚡ **Zero External Dependencies** - Self-contained binary without heavy GTK or Electron runtimes

---

## 📦 Installation & Usage

### Option 1: Official AppImage (Recommended)

Download the pre-built, portable AppImage directly from GitHub Releases. No installation or compiler required:

1. Download `Flicker-1.0.0-x86_64.AppImage` from the [Releases page](https://github.com/vimi-tt/Flicker/releases).
2. Make it executable:
   ```bash
   chmod +x Flicker-1.0.0-x86_64.AppImage
   ```
3. **Double-click** the file in your desktop file manager to open it immediately, or launch it from the terminal:
   ```bash
   ./Flicker-1.0.0-x86_64.AppImage
   ```

You can also use all CLI commands directly through the AppImage:
```bash
./Flicker-1.0.0-x86_64.AppImage list -v
sudo ./Flicker-1.0.0-x86_64.AppImage write --iso ubuntu.iso --device /dev/sdb --verify
```

### Option 2: System Installation via `install.sh`

If you prefer building from source and installing globally to `/usr/local/bin` with desktop integration:

```bash
git clone https://github.com/vimi-tt/Flicker.git
cd Flicker
./install.sh
```

---

## 🔐 Privilege Elevation (`pkexec` and `sudo`)

Writing raw disk images to block devices (`/dev/sdX`) requires root privileges in Linux.

- **Graphical Interface (GUI):**
  When launched without root, Flicker automatically elevates privileges via `pkexec`, prompting you with a native Polkit authentication dialog.
  Inside AppImage environments, Flicker detects the `$APPIMAGE` runtime variable and re-executes using `--appimage-extract-and-run`, completely bypassing user-space FUSE mount restrictions.
- **Command-Line Interface (CLI):**
  In CLI mode, execute destructive operations with `sudo`:
  ```bash
  sudo flicker write --iso image.iso --device /dev/sdb
  ```

---

## 🚀 Quick Start

### 1. Launch the Graphical Interface (GUI)

Simply launch Flicker with double-click or run:
```bash
flicker
```

### 2. Write ISO to USB (CLI Mode)

```bash
sudo flicker write --iso ubuntu-24.04.iso --device /dev/sdb
```

### 3. Write with verification (CLI)

```bash
sudo flicker write --iso ubuntu-24.04.iso --device /dev/sdb --verify
```

---

## 📖 CLI Commands

### `list` - List Removable USB Devices

```bash
# Simple list
flicker list

# Detailed information (model, size, serial, mount points)
flicker list -v
```

### `write` - Write ISO to USB

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
sudo flicker write --iso ubuntu.iso --device /dev/sdb /dev/sdc

# Resume interrupted write
sudo flicker write --iso ubuntu.iso --device /dev/sdb --resume
```

### `verify` - Verify ISO Checksum

```bash
flicker verify --iso <FILE> [--checksum <HASH>] [--algorithm <ALGO>]
```

**Options:**
- `--iso, -i <FILE>` - Path to ISO file (required)
- `--checksum, -c <HASH>` - Expected checksum to verify against
- `--algorithm, -a <ALGO>` - Algorithm (`sha256` or `md5`, default: `sha256`)

---

## 🛠️ Building & Packaging

### Building Locally

```bash
# Install dependencies (Ubuntu/Debian)
sudo apt install build-essential pkg-config libudev-dev

# Compile in release mode
cargo build --release

# Run tests
cargo test

# Run linter
cargo clippy -- -D warnings
```

### Building the AppImage Locally

Run the packaging script to generate a standalone AppImage:

```bash
# Build on host
./packaging/build-appimage.sh

# Or build inside an Ubuntu 22.04 container for maximum GLIBC backward compatibility:
./packaging/build-appimage.sh --container
```

The resulting AppImage will be available at `target/Flicker-1.0.0-x86_64.AppImage`.

---

## 📋 Roadmap

- [x] **Direct App Launch** - Start Flicker with a double-click without needing a terminal (AppImage)
- [x] **GNOME Adwaita & Material You UI** - Modern native Linux HIG aesthetics
- [x] **Safe Root Escalation** - Seamless `pkexec` within AppImage
- [ ] Custom persistence partition creation
- [ ] Multiboot ISO support
- [ ] Windows & macOS experimental support

---

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

*Flicker - Making bootable USB creation simple, fast, and safe on Linux*
