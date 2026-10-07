# 🖥️ Usage Guide - Graphical Interface (GUI)

## Overview

Flicker 1.0 features a completely redesigned declarative Graphical User Interface built with **Slint**. The interface adheres to the modern **GNOME Human Interface Guidelines (HIG)** and **libadwaita** conventions, combined with the core dynamic tonal palette principles of **Material You**.

## Launching

### Direct AppImage Launch (Double-Click)
The primary way to use Flicker GUI is double-clicking the official `Flicker-1.0.0-x86_64.AppImage` directly from your file manager (Files / Nautilus, Dolphin, Nemo, Thunar). No terminal window is required!

### Command Line
You can also launch the GUI from the terminal:
```bash
# If installed globally:
flicker

# Or via AppImage:
./Flicker-1.0.0-x86_64.AppImage
```

### Privilege Escalation (`pkexec`)

Flicker requires root privileges to write raw disk data directly to removable block devices.
When launched as a standard user:
1. Flicker automatically requests privilege elevation using `pkexec`, bringing up your desktop environment's native Polkit authentication prompt.
2. In AppImage environments, Flicker detects the `$APPIMAGE` variable and safely invokes `--appimage-extract-and-run`. This completely avoids the Linux limitation where root cannot access user-mounted FUSE filesystems (`/tmp/.mount_XXXX`).
3. Essential display and session environment variables (`DISPLAY`, `XAUTHORITY`, `WAYLAND_DISPLAY`, `XDG_RUNTIME_DIR`, `DBUS_SESSION_BUS_ADDRESS`) are propagated seamlessly.

## Interface Structure

The redesign features a clean, responsive single-column layout centered with standard Adwaita spacing:

### 1. Compact HeaderBar
- **Application Title & Subtitle**: Centered title and utility description.
- **🔄 Rescan Action**: Instantly rescans the host system for newly inserted or removed USB devices.
- **🌓 Theme Switcher**: Allows manual toggle between Light and Dark mode, although Flicker automatically detects your desktop preference.

### 2. Grouped Boxed Lists (`PreferencesGroup`)
- **Source Image**: Displays the currently selected image file. Click `Select Image…` to open the native system file dialog.
- **Target Drive**: A curated dropdown menu displaying detected USB removable devices with their paths, vendor/model names, and human-readable capacity.
- **Write Options**:
  - `Verify after writing`: Byte-by-byte checksum verification against the ISO file once flashing concludes.
  - `Resume interrupted write`: Compares blocks and skips identical chunks to accelerate recovery from aborted writes.

### 3. Activity & Progress View
- **Status Banner**: Displays clear visual feedback (`info`, `warning`, `success`, `error`) indicating current state.
- **Slim Progress Bar**: High-precision thin Adwaita-style progress indicator with percentage and active status.
- **Collapsible Terminal Logs**: A toggleable detailed log console for inspecting real-time write rates, unmounting notifications, and sync operations.

### 4. Safety Confirmation Dialog
Before any destructive operation begins, Flicker displays a modal confirmation card (`AdwMessageDialog` style) explicitly naming the device to be wiped and asking for final confirmation.

## Dynamic Theming & System Integration

- **Automatic Color-Scheme Detection**: Connects to `org.freedesktop.portal.Settings` (and fallback `gsettings`) to match your system's Dark or Light appearance.
- **System Accent Color**: Detects your desktop's configured accent color and uses it as the seed for dynamic action highlights and focus rings.
- **Accessibility & Contrast**: Built to exceed WCAG AA contrast standards across both Light and Dark themes.

## References

- [CLI Write Command](COMMAND_WRITE.md)
- [Technical Documentation](ISO_WRITING.md)
- [Device Detection](USB_DETECTION.md)
