<div align="center">

<img src="Flicker.png" alt="Flicker icon" width="112" height="112" />

# Flicker

**A fast, safe, Rufus-style bootable USB creator for Linux, written in Rust.**

[![Release](https://img.shields.io/github/v/release/vimi-tt/Flicker?style=flat-square&color=A83814)](https://github.com/vimi-tt/Flicker/releases)
[![CI](https://img.shields.io/github/actions/workflow/status/vimi-tt/Flicker/ci.yml?branch=main&style=flat-square&label=CI)](https://github.com/vimi-tt/Flicker/actions/workflows/ci.yml)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-211A18?style=flat-square)](https://www.rust-lang.org/)
[![Platform](https://img.shields.io/badge/platform-Linux-211A18?style=flat-square)](https://www.linux.org/)
[![License: MIT](https://img.shields.io/badge/license-MIT-211A18?style=flat-square)](LICENSE)

[Installation](#installation) · [Quick Start](#quick-start) · [CLI Reference](#cli-reference) · [Security Model](#security-model) · [Development](#development)

</div>

---

Flicker creates bootable USB drives from **Linux hybrid ISOs**, **Windows 10/11 installer ISOs**, and **raw or compressed disk images** (`.iso`, `.img`, `.gz`, `.xz`, `.zst`), and cleanly partitions or formats drives (`GPT`/`MBR` + `FAT32`/`NTFS`/`exFAT`/`ext4`). It ships with a **Material Design 3 Expressive** desktop interface built in Slint and a full-featured CLI.

## Capabilities

| Area | What Flicker does |
| :--- | :--- |
| **Image introspection** | Automatically inspects ISO 9660, UDF, MBR/`isohybrid`, and compression magic bytes to select the right write mode and volume label. |
| **Windows 10/11 ISOs** | Creates `GPT`/`MBR` + `NTFS`/`FAT32` layouts (with optional `UEFI:NTFS` boot partition), extracts files via `7z` or loop-mount, and optionally injects `autounattend.xml` to bypass TPM 2.0, Secure Boot, 4 GB RAM, and Microsoft Account checks. |
| **Linux & raw images** | Parallel multi-device DD writes with 4 MiB chunks, periodic `sync_data()`, on-the-fly decompression (`.gz`, `.xz`, `.zst`), resume mode, and optional `ext4` Live persistence (`casper-rw` + `persistence.conf`). |
| **Drive formatting** | Rufus-style non-bootable format mode: wipes stale signatures, creates clean `GPT` or `MBR` tables, and formats to `FAT32`, `NTFS`, `exFAT`, or `ext4` with filesystem-safe labels. |
| **Real verification** | Post-write verification invalidates the Linux kernel buffer/page cache (`BLKFLSBUF` + `POSIX_FADV_DONTNEED`) so every byte is read back from physical USB flash rather than RAM. |
| **Least-privilege GUI** | Runs the desktop interface as your regular user (Wayland-friendly, native file picker in `$HOME`) and elevates only the write/format helper via `pkexec`. |
| **USB hotplug & cancel** | Watches `udevadm` block events to refresh drives automatically on plug/unplug, and supports clean cancellation via the GUI or `Ctrl+C`. |

---

## Installation

### AppImage (recommended)

Download `Flicker-1.0-x86_64.AppImage` from [GitHub Releases](https://github.com/vimi-tt/Flicker/releases):

```bash
chmod +x Flicker-1.0-x86_64.AppImage
./Flicker-1.0-x86_64.AppImage
```

### Lightweight CLI-only binary

For headless systems or terminal-only workflows (no GUI dependencies), download `flicker-1.0-x86_64-linux-cli.tar.gz` from [GitHub Releases](https://github.com/vimi-tt/Flicker/releases):

```bash
tar -xzf flicker-1.0-x86_64-linux-cli.tar.gz
sudo install -m 755 flicker /usr/local/bin/flicker
```

### Build from source

Requires Rust `1.85+`. For GUI builds on Debian/Ubuntu, install `pkg-config libfontconfig1-dev libxkbcommon-dev libgtk-3-dev`.

```bash
git clone https://github.com/vimi-tt/Flicker.git
cd Flicker

# Build and install GUI + CLI + desktop entry
./install.sh

# Or install CLI-only without GUI dependencies
./install.sh --cli-only

# Uninstall later
./install.sh --uninstall
```

---

## Quick Start

### Desktop interface

Launch **Flicker** from your application menu, run the AppImage, or start it from a terminal:

```bash
flicker
```

Select a target drive (updated automatically on USB plug/unplug), choose **ISO / Disk Image** or **Format Only**, pick your image, review the auto-detected settings, and click **Start Flash**. Administrator privileges are requested via `pkexec` only after you confirm erasure.

### Command-line examples

```bash
# List connected USB storage devices with full details
flicker list -v

# Inspect an image before writing (type, volume label, hybrid/UDF/compression)
flicker inspect --iso ~/Downloads/Win11_24H2.iso

# Write a Linux hybrid ISO with physical read-back verification
sudo flicker write --iso ubuntu-24.04.iso --device /dev/sdb --verify

# Write a Linux ISO and create a 4 GB ext4 persistence partition
sudo flicker write --iso ubuntu-24.04.iso --device /dev/sdb --persistence 4096

# Write a Windows 11 ISO (GPT + NTFS) with TPM/RAM & Microsoft Account bypass
sudo flicker write --iso Win11.iso --device /dev/sdb --mode iso --scheme gpt --fs ntfs \
  --win-bypass-tpm --win-bypass-msa

# Write a compressed raw image (.gz, .xz, or .zst) to multiple drives in parallel
sudo flicker write --iso raspberry-pi-os.img.xz --device /dev/sdb /dev/sdc --verify

# Format a USB drive cleanly (non-bootable)
sudo flicker format --device /dev/sdb --scheme gpt --fs exfat --label MY_USB

# Verify an ISO checksum (SHA256 or MD5)
flicker verify --iso ubuntu-24.04.iso --checksum <expected_hash> --algorithm sha256
```

---

## CLI Reference

### `flicker list`

Lists detected USB and removable storage devices (including external USB SSDs/HDDs, excluding empty card readers and optical drives).

| Flag | Description |
| :--- | :--- |
| `-v, --verbose` | Show vendor, model, exact byte size, serial number, and mount points. |

### `flicker inspect`

Inspects image headers without mounting.

| Flag | Description |
| :--- | :--- |
| `-i, --iso <FILE>` | Path to `.iso`, `.img`, `.gz`, `.xz`, or `.zst` image. |

### `flicker write`

Writes an image to one or more target devices. Requires `root` (`sudo`).

| Flag | Default | Description |
| :--- | :--- | :--- |
| `-i, --iso <FILE>` | *required* | Source image path. |
| `-d, --device <DEV>...` | *required* | One or more target block devices (also accepts `--devices`). |
| `-m, --mode <MODE>` | `auto` | `auto` (detect from image), `dd` (raw write), or `iso` (partition + extract). |
| `-s, --scheme <SCHEME>` | `gpt` | Partition scheme for ISO mode (`gpt` or `mbr`). |
| `-f, --fs <FS>` | `ntfs` | Target filesystem for ISO mode (`ntfs`, `fat32`, `exfat`, `ext4`). |
| `-l, --label <LABEL>` | *detected* | Custom volume label for ISO mode. |
| `-v, --verify` | `false` | Verify written bytes directly from physical media after syncing. |
| `-r, --resume` | `false` | Skip 4 MiB chunks that already match on the target device (DD mode). |
| `--win-bypass-tpm` | `false` | Inject `autounattend.xml` bypassing TPM 2.0, Secure Boot, CPU, and 4 GB RAM checks. |
| `--win-bypass-msa` | `false` | Inject `autounattend.xml` bypassing Microsoft Account (`BypassNRO`). |
| `--persistence <MB>` | `0` | Create an `ext4` persistence partition (`casper-rw`) after a DD write. |
| `-y, --yes` | `false` | Skip interactive confirmation prompts. |

### `flicker format`

Wipes, partitions, and formats a USB device without writing an image. Requires `root` (`sudo`).

| Flag | Default | Description |
| :--- | :--- | :--- |
| `-d, --device <DEV>` | *required* | Target block device (e.g., `/dev/sdb`). |
| `-s, --scheme <SCHEME>` | `gpt` | Partition scheme (`gpt` or `mbr`). |
| `-f, --fs <FS>` | `fat32` | Filesystem (`fat32`, `ntfs`, `exfat`, `ext4`). |
| `-l, --label <LABEL>` | `FLICKER_USB` | Volume label (automatically sanitized for the chosen filesystem). |
| `-y, --yes` | `false` | Skip interactive confirmation prompt. |

### `flicker verify`

Calculates or compares a file checksum.

| Flag | Default | Description |
| :--- | :--- | :--- |
| `-i, --iso <FILE>` | *required* | File to hash. |
| `-c, --checksum <HASH>` | *none* | Expected hash to compare against (case-insensitive). |
| `-a, --algorithm <ALGO>` | `sha256` | Hash algorithm (`sha256` or `md5`). |

---

## Security Model

Writing to raw block devices is destructive by nature. Flicker enforces the following safeguards across both GUI and CLI:

1. **Unprivileged GUI** — The Slint window never runs as `root`. Only the minimal write/format helper is elevated via `pkexec` (using `$APPIMAGE` when running from an AppImage so FUSE permissions are respected).
2. **Exact mount resolution** — `/proc/mounts` is matched strictly by device/partition boundary (`/dev/sdb`, `/dev/sdb1`, `/dev/nvme0n1p1`), eliminating substring collisions (such as `sda` vs `sdaa`). Unmount failures abort the operation rather than writing over mounted filesystems.
3. **Pre-flight validation** — Verifies block device type, image integrity, and available capacity (for both detected USBs and explicitly confirmed non-USB targets) before prompting for confirmation.
4. **Physical read-back verification** — Calls `ioctl(BLKFLSBUF)` and `posix_fadvise(POSIX_FADV_DONTNEED)` before verification so faulty or counterfeit drives cannot pass via the OS page cache.
5. **RAII cleanup** — Temporary mount points created during Windows ISO extraction or persistence setup are unmounted and removed automatically even on error or cancellation.

> **Note on UEFI:NTFS:** Modern UEFI firmware can boot directly from GPT/NTFS partitions. For older UEFI systems that lack a native NTFS driver, place Rufus's `uefi-ntfs.img` at `/usr/share/flicker/uefi-ntfs.img` (or `assets/uefi-ntfs.img`) and Flicker will automatically create the companion `UEFI:NTFS` boot partition.

---

## Development

### Repository layout

```text
Flicker/
├── src/
│   ├── main.rs            # Entry point, CLI handlers, and pkexec helper commands
│   ├── cli/mod.rs         # Clap argument definitions
│   ├── gui/mod.rs         # Unprivileged Slint GUI controller & helper bridge
│   ├── iso/mod.rs         # Pure-Rust ISO9660/UDF/compression introspection
│   ├── usb/mod.rs         # Sysfs USB bus detection & udevadm hotplug watcher
│   ├── writer/mod.rs      # Parallel DD writer, cache invalidation, streaming decompression
│   ├── format/mod.rs      # Partitioning & filesystem formatting (GPT/MBR)
│   ├── windows/mod.rs     # Windows ISO extraction, autounattend.xml, Linux persistence
│   ├── checksum/mod.rs    # Generic SHA256 / MD5 file hashing
│   └── utils.rs           # Validations, exact /proc/mounts parser, unmounting
├── ui/
│   ├── theme.slint        # Material Design 3 Expressive semantic tokens (Light/Dark)
│   ├── components.slint   # Segmented buttons, expressive CTA, device rows, progress card
│   └── appwindow.slint    # Two-pane desktop layout & confirmation dialog
├── packaging/
│   ├── flicker.desktop
│   ├── io.github.vimi_tt.Flicker.metainfo.xml
│   └── appimage/build-appimage.sh
└── .github/workflows/
    ├── ci.yml             # Clippy (-D warnings) & tests (default + --no-default-features)
    └── release.yml        # Automated AppImage & CLI tarball GitHub Releases
```

### Common commands

```bash
# Run linter (GUI + CLI and CLI-only)
cargo clippy --all-targets -- -D warnings
cargo clippy --no-default-features --all-targets -- -D warnings

# Run test suite
cargo test
cargo test --no-default-features

# Build AppImage locally (requires appimagetool in PATH or target/)
./packaging/appimage/build-appimage.sh
```

---

## License

Released under the [MIT License](LICENSE).
