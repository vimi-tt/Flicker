# Flicker

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org/)
[![Platform](https://img.shields.io/badge/platform-Linux-blue.svg)](https://www.linux.org/)

> A modern, fast, and safe raw ISO/IMG USB bootable drive creator for Linux, written in Rust 🦀

Flicker allows you to create bootable USB drives from Linux hybrid ISO and raw `.img` disk images (similar to BalenaEtcher / Fedora Media Writer), with both a Slint desktop GUI and a full-featured CLI.

> **Scope note:** Flicker writes raw/hybrid images directly to block devices. Standard Windows installer ISOs (which are not hybrid images and require GPT/MBR partitioning + NTFS/FAT32 extraction) are not yet supported.

## ✨ Features

- 🔍 **Smart USB Detection** - Detects USB drives (including external USB SSDs/HDDs) via sysfs bus inspection while filtering empty readers and optical drives
- 🚀 **Parallel Multi-Device Writing** - Optimized 4MB chunk writes with periodic sync and true per-device parallel threads
- 🛡️ **Safety First** - Exact `/proc/mounts` matching, mandatory unmounting, size checks, and explicit confirmations in both GUI and CLI
- 🔐 **Least-Privilege GUI** - The graphical interface runs unprivileged (Wayland-friendly) and elevates only the write helper via `pkexec`
- ✅ **Real Data Verification** - Post-write verification invalidates the Linux kernel buffer/page cache (`BLKFLSBUF` / `POSIX_FADV_DONTNEED`) so data is read back from physical USB flash
- ⏹️ **Clean Cancellation** - Cancel writes safely from the GUI or with `Ctrl+C` in the CLI
- 🔓 **Auto Unmount** - Automatically unmounts target partitions before writing
- 💻 **CLI & Optional GUI** - Build with GUI + CLI by default, or `--no-default-features` for a lightweight CLI-only binary

## 📦 Installation

### Official Installer

The recommended way to install Flicker is using the provided installation script. It will automatically build the project, install the binary globally, and configure the desktop entry so you can launch Flicker directly from your application menu with its official icon.

```bash
# Clone the repository
git clone https://github.com/vimi-tt/Flicker.git
cd Flicker

# Run the installer
./install.sh
```

## 🚀 Quick Start

### 1. Launch the Graphical Interface (GUI)

Flicker Beta 2 features a beautiful Material Design 3 interface. You can launch it from your desktop environment's application menu (after running `./install.sh`), or directly from the terminal:

```bash
flicker
```

*(Note: The GUI will automatically prompt for your password using `pkexec` to elevate privileges securely. No need to run as sudo!)*

### 2. Write ISO to USB (CLI Mode)

If you prefer the command line, Flicker still supports a fully-featured CLI:

```bash
sudo flicker write --iso ubuntu-24.04.iso --device /dev/sdb
```

### 3. Write with verification (CLI)

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
│   ├── main.rs          # Entry point & CLI orchestration
│   ├── cli/             # CLI argument parsing
│   │   └── mod.rs
│   ├── gui/             # Slint GUI & pkexec helper bridge
│   │   └── mod.rs
│   ├── checksum/        # SHA256 / MD5 verification
│   │   └── mod.rs
│   ├── usb/             # USB bus & block device detection
│   │   └── mod.rs
│   ├── writer/          # Parallel ISO writer, cache invalidation & verification
│   │   └── mod.rs
│   └── utils.rs         # Validations, exact mount parsing & unmounting
├── ui/                  # Slint GUI declarations
│   └── appwindow.slint
├── .github/workflows/   # CI pipeline
├── docs/                # Documentation
├── Cargo.toml           # Project configuration
├── build.rs             # Slint build script
├── install.sh           # Installer (--cli-only, --uninstall)
├── CHANGELOG.md         # Release history
├── Flicker.png          # App icon
└── README.md
```

### Building from Source

```bash
# Development build (GUI + CLI)
cargo build

# Release build (GUI + CLI)
cargo build --release

# Lightweight CLI-only build (no Slint/GUI dependencies)
cargo build --release --no-default-features

# Run tests
cargo test

# Run linter
cargo clippy --all-targets -- -D warnings
```

## 📋 Roadmap

### 🚧 Beta 3 (Planned)
- [ ] Direct App Launch (Start Flicker by double-clicking without ever needing a terminal window)
- [ ] Custom persistence partition creation
- [ ] Multiboot ISO support

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