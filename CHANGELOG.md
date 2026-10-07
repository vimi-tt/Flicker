# Changelog

All notable changes to Flicker will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Planned
- Windows & macOS support
- Custom persistence partition creation
- Multiboot ISO support

## [1.0.0] - 2026-10-07

### Added
- **Official AppImage Distribution**: Standalone, portable `Flicker-1.0.0-x86_64.AppImage` executable with zero system runtime dependencies.
- **Double-Click Desktop Launch**: Launch the application directly from file managers without needing a terminal.
- **AppStream & Desktop Integration**: Full XDG desktop entry (`flicker.desktop`), multi-resolution icons (16px to 512px), and AppStream metadata (`io.github.vimi_tt.flicker.metainfo.xml`).
- **Complete Visual Redesign (GNOME Adwaita HIG)**:
  - HeaderBar with compact window controls, title, USB rescan action, and light/dark theme toggle.
  - Grouped boxed lists (`PreferencesGroup`) replacing heavy elevation cards.
  - Slim progress indicator with real-time percentage and transfer rate.
  - Collapsible terminal logs view.
  - Action button styling with Adwaita suggested and destructive styles.
- **Material You Dynamic Palette & System Theming**:
  - Centralized design tokens in `ui/theme.slint`.
  - Automatic system Dark/Light mode detection via `org.freedesktop.appearance` portal and `gsettings`.
  - Automatic system accent-color detection as tonal palette seed.
  - Guaranteed WCAG AA contrast compliance for all textual elements.
- **Enhanced Safety**: Inline modal confirmation dialog before any destructive flashing operation.
- **AppImage Privilege Escalation**: Resolved root elevation inside AppImage mounts using `$APPIMAGE` and `--appimage-extract-and-run`.
- **Automated CI/CD**: GitHub Actions workflow targeting Ubuntu 22.04 LTS for broad GLIBC compatibility and automatic release tagging.

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

### Official 1.0.0 Release (1.0.0)

The official 1.0 milestone release of Flicker! 🎉

**Highlights:**
- Official portable AppImage release: run everywhere with a single double-click.
- Stunning GNOME Adwaita redesign with Material You tonal color palettes.
- Seamless Dark/Light system synchronization and system accent color support.
- Fully compatible CLI and GUI modes with safe privilege escalation.

---

[Unreleased]: https://github.com/vimi-tt/Flicker/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/vimi-tt/Flicker/compare/v0.1.0-beta.2...v1.0.0
[0.1.0-beta.2]: https://github.com/vimi-tt/Flicker/compare/v0.1.0-beta.1...v0.1.0-beta.2
[0.1.0-beta.1]: https://github.com/vimi-tt/Flicker/releases/tag/v0.1.0-beta.1
[0.0.1]: https://github.com/vimi-tt/Flicker/releases/tag/v0.0.1
