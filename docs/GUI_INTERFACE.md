# 🖥️ Usage Guide - Graphical Interface (GUI)

## Overview

Flicker Beta 2 introduces a modern, declarative Graphical User Interface built with **Slint**. The interface is designed following the **Material Design 3** guidelines, providing a premium, fluid, and robust user experience.

## Launching

To launch the GUI, simply run `flicker` without any command-line arguments:

```bash
flicker
```

### Privilege Escalation (`pkexec`)

Flicker requires root privileges to write data directly to USB block devices. If you launch the GUI as a standard user, Flicker will automatically elevate its privileges using `pkexec`. 

It seamlessly propagates essential environment variables (such as `DISPLAY`, `XAUTHORITY`, `XDG_RUNTIME_DIR`, and `WAYLAND_DISPLAY`) so the interface renders perfectly within your user's desktop environment while retaining `root` permissions under the hood.

## Interface Layout

The interface is structured in a two-column layout using absolute positioning to ensure stability and pixel-perfect rendering across any OS:

### Left Column (Configuration)
- **Source Image**: Displays the currently selected ISO. Click `Browse...` to open a native file dialog.
- **Target Device**: A dropdown menu displaying all connected removable USB devices. It displays the path, model, and total size.
- **Refresh**: Rescans the system for newly inserted or removed USB devices instantly.

### Right Column (Feedback & Options)
- **Options**:
  - `Verify data after writing`: Enables byte-by-byte checksum validation after the flash completes.
  - `Resume interrupted write`: Skips blocks that are already identical, saving time on retries.
- **Terminal Log**: A large, read-only text area displaying real-time verbose output of the flashing process, identical to the CLI output. This eliminates the need for hidden terminal windows.

## The "Morphing Container" Animation

Flicker implements a highly customized Material Design 3 **Container Transform** animation for its primary action.

1. **Idle State**: The action begins as a standard "FLASH!" button located at the bottom of the Left Column.
2. **Active State (Flashing)**: Upon clicking, the button's text evaporates, and the component fluidly morphs—sliding across the screen to the Right Column. It expands its dimensions, morphs its border-radius from a pill shape to a standard container, and shifts its color to a Material 3 Secondary Container palette (`#E8DEF8`).
3. **Progress Tracking**: Once the transformation completes, a dynamic `ProgressIndicator` and a status text element fade in to provide real-time updates.

This complex physical animation is driven by a `cubic-bezier(0.2, 0, 0, 1)` curve (the official *Emphasized Decelerate* standard) to ensure a buttery smooth, professional interaction.

## Architecture & Synchronization

The GUI is fully decoupled from the core synchronous flashing logic, guaranteeing that the UI thread never hangs or freezes during heavy I/O operations.

- **Asynchronous Channels**: Flicker utilizes Rust's standard `std::sync::mpsc::channel` to bridge the backend operations and the Slint event loop.
- **Message Protocol**: The background writing thread emits `ProgressMsg` variants (`Log`, `Progress`, `Status`).
- **Event Loop Injection**: The receiver thread consumes these messages and injects them into the UI thread using `slint::invoke_from_event_loop`, ensuring thread-safe updates to the declarative properties.

## Troubleshooting

### "GUI fails to open / Cannot connect to Display"
```
qt.qpa.xcb: could not connect to display
```
**Solution**: This happens if your X11 or Wayland environment variables aren't propagated to the root user. Flicker handles this automatically via `pkexec`, but if your system configuration prevents it, you can run the GUI directly from a root terminal preserving the environment:
```bash
sudo -E flicker
```

## References

- [Write Command (CLI)](COMMAND_WRITE.md)
- [Technical Documentation](ISO_WRITING.md)
- [Device Detection](USB_DETECTION.md)
