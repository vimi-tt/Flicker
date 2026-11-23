# Changelog

All notable changes to Flicker will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Planned
- Windows & macOS host support
- Multiboot ISO support
- Bad Blocks verification pass

## [1.0.0] - 2026-11-23

### Added
- Material Design 3 Expressive UI (`ui/theme.slint`, `ui/components.slint`, `ui/appwindow.slint`) with Flame Ember tokens, light/dark mode, and new 512×512 app icon
- Rufus-style workflow: Boot Selection (`ISO / Disk Image` vs `Format Only`), Partition Scheme (`GPT`/`MBR`), Filesystem (`NTFS`/`FAT32`/`exFAT`/`ext4`), and Volume Label
- Windows 10/11 ISO Mode with GPT/NTFS layout, optional `UEFI:NTFS` boot partition, `7z`/loop extraction, and optional `autounattend.xml` (bypass TPM 2.0/Secure Boot/RAM & Microsoft Account)
- Pure-Rust ISO9660/UDF/compression introspection (`flicker inspect`)
- Streaming decompression for `.gz`, `.xz`, and `.zst` images
- Linux Live `ext4` persistence partition (`casper-rw` + `persistence.conf`)
- Automatic USB hotplug watcher (`udevadm` with `/sys/block` fallback)
- Clean operation cancellation (`Ctrl+C` in CLI, Cancel button in GUI)
- AppImage packaging (`packaging/appimage/build-appimage.sh`), FreeDesktop metadata, and GitHub Actions CI/Release workflows
- Optional `gui` Cargo feature (`--no-default-features` for lightweight CLI-only builds)

### Security & Fixes
- Decoupled GUI from `root`: GUI runs unprivileged (Wayland-friendly) and elevates only `internal-write`/`internal-format` via `pkexec` (with `$APPIMAGE` support)
- Real post-write verification with Linux kernel buffer/page cache invalidation (`BLKFLSBUF` + `POSIX_FADV_DONTNEED`)
- Exact `/proc/mounts` device/partition matching (fixes substring collision bugs)
- True parallel multi-device writing with periodic `sync_data()`
- Fixed CLI `--device` flag, GUI confirmation dialog, state reset after completion, and removed unused dependencies (`thiserror`, `sysinfo`)

## [0.1.0-beta.2] - 2026-09-02

### Added
- Graphical User Interface (GUI) powered by Slint
- Material Design 3 Container Transform animations
- ISO checksum verification (SHA256, MD5)
- Multi-device support (write to multiple USBs simultaneously)
- Resume interrupted writes (skip identical blocks)
- Global desktop entry and icon installation

## [0.1.0-beta.1] - 2026-02-18

### Added
- USB device detection and listing
- ISO file writing to USB devices
- Real-time progress bar with ETA and speed
- Data verification after writing
- Automatic device unmounting
- Multiple safety validations
- CLI interface with clap
- Comprehensive documentation

### Security
- Root permission verification
- Device type validation (USB vs internal disk)
- Size verification before writing
- Multiple user confirmations
- Protection against accidental data loss

### Performance
- 4MB chunk optimization
- USB 2.0: 30-40 MB/s average
- USB 3.0: 100-200 MB/s average
- Efficient sync operations

### Documentation
- Complete README with usage examples
- Technical documentation for USB detection
- Command-specific guides
- Troubleshooting section
- FAQ

## [0.0.1] - 2026-01-23

### Added
- Initial project structure
- Basic CLI framework
- Project planning and design

---

## Release Notes

### Beta 2 (0.1.0-beta.2)

The UI/UX update and Advanced Features! 🎉

**Highlights:**
- Stunning new GUI with Material Design 3 animations
- Wrote to multiple devices at once
- Resume interrupted writes securely
- Integrated ISO Checksum verifier

**Known Issues:**
- Windows/MacOS support pending
- Persistence partitions not yet implemented

### Beta 1 (0.1.0-beta.1)

First public beta release! 🎉

**Highlights:**
- Fully functional ISO writing to USB
- Beautiful CLI with progress bars
- Multiple safety features
- Comprehensive verification system

**Known Issues:**
- Checksum verification not yet implemented
- Single device write only (no parallel writes)
- Linux only (no Windows/MacOS support)

**Testing:**
- Tested with Ubuntu, Debian, Arch Linux ISOs
- Tested on USB 2.0 and USB 3.0 devices
- Verified on multiple Linux distributions

**Feedback Welcome:**
Please report any issues or suggestions on our GitHub Issues page!

---

[Unreleased]: https://github.com/vimi-tt/Flicker/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/vimi-tt/Flicker/compare/v0.1.0-beta.2...v1.0.0
[0.1.0-beta.2]: https://github.com/vimi-tt/Flicker/compare/v0.1.0-beta.1...v0.1.0-beta.2
[0.1.0-beta.1]: https://github.com/vimi-tt/Flicker/releases/tag/v0.1.0-beta.1
[0.0.1]: https://github.com/vimi-tt/Flicker/releases/tag/v0.0.1
