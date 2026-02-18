# Changelog

All notable changes to Flicker will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Planned
- ISO checksum verification (SHA256, MD5)
- Multi-device support
- Resume interrupted writes
- Configuration file support

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

[Unreleased]: https://github.com/yourusername/flicker/compare/v0.1.0-beta.1...HEAD
[0.1.0-beta.1]: https://github.com/yourusername/flicker/releases/tag/v0.1.0-beta.1
[0.0.1]: https://github.com/yourusername/flicker/releases/tag/v0.0.1
