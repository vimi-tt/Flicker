# ✍️ Usage Guide - `write` Command

## Syntax

```bash
sudo flicker write --iso <ISO_FILE> --device <DEVICE> [OPTIONS]
```

## ⚠️ IMPORTANT

- **Always use `sudo`** - writing to devices requires administrator privileges
- **All data will be erased** - make sure to choose the correct device
- **Do not remove the USB** during writing

## Options

- `--iso <FILE>` or `-i <FILE>` : Path to ISO file (required)
- `--device <DEVICE>` or `-d <DEVICE>` : Target device (required)
- `--yes` or `-y` : Skip confirmation (use with caution!)
- `--verify` or `-v` : Verify data after writing (recommended)

## Examples

### 1. Basic Write

```bash
sudo flicker write --iso ~/Downloads/ubuntu-24.04.iso --device /dev/sdb
```

**What happens:**
1. Checks root permissions
2. Validates ISO and device
3. Checks if it fits on USB
4. Unmounts if necessary
5. Asks for confirmation
6. Writes with progress bar
7. Syncs data
8. Shows time and speed

### 2. With Verification

```bash
sudo flicker write \
    --iso ~/Downloads/ubuntu-24.04.iso \
    --device /dev/sdb \
    --verify
```

**Time:** ~2x slower (verifies all bytes)  
**Advantage:** Guarantees 100% integrity

### 3. Without Confirmation (Automation)

```bash
sudo flicker write \
    --iso ~/Downloads/ubuntu-24.04.iso \
    --device /dev/sdb \
    --yes
```

**⚠️ WARNING:** Use only in scripts where you're sure about the device!

### 4. Short Flags

```bash
sudo flicker write -i ubuntu.iso -d /dev/sdb -v
```

## Recommended Workflow

### Safe Step-by-Step

```bash
# 1. List devices BEFORE connecting the USB
lsblk

# 2. Connect the USB

# 3. List again to identify the new device
flicker list -v

# 4. Confirm which one is your USB (by size, brand, etc)
# Example: /dev/sdb - SanDisk 16GB

# 5. Write with verification
sudo flicker write \
    --iso ~/Downloads/ubuntu-24.04.iso \
    --device /dev/sdb \
    --verify

# 6. Wait for completion and success message

# 7. Safely remove USB
```

## Command Output

### Complete Example

```
🔐 Checking permissions...
✓ Running with root privileges

📋 Validating ISO file...
✓ ISO file valid: /home/user/Downloads/ubuntu-24.04.iso (3.8 GB)

💾 Validating device...
✓ Device valid: /dev/sdb

📊 Space check:
   ISO size:    3.78 GB
   Device size: 14.92 GB
   ✓ Sufficient space available

🔓 Device is mounted, unmounting...
✓ Device unmounted successfully

============================================================
⚠️  FINAL WARNING ⚠️
============================================================

This will COMPLETELY ERASE all data on:
   Device: /dev/sdb
   Size:   14.92 GB

And write:
   ISO:    /home/user/Downloads/ubuntu-24.04.iso
   Size:   3.78 GB

============================================================

❓ Type 'yes' to confirm and start writing: yes

============================================================
🔥 STARTING WRITE PROCESS
============================================================

🚀 Starting ISO write process...
   ISO size: 3.78 GB
   Opening device: /dev/sdb

📝 Writing ISO to device...
⠁ [00:02:15] [################>-----] 2.5GB/3.8GB (00:01:20)

✓ Write completed

🔄 Syncing data to device (this may take a moment)...
✓ Sync completed

🔍 Verifying written data...
⠁ [00:02:10] [####################] 3.8GB/3.8GB (done)
✓ Verification completed

✓ Verification completed successfully

🎉 ISO successfully written to /dev/sdb
   You can now safely remove the USB device

============================================================
✅ SUCCESS!
============================================================
   Total time: 285.34s
   Average speed: 13.24 MB/s
   Device: /dev/sdb
============================================================
```

## Estimated Times

### USB 2.0 (~30 MB/s)

| ISO Size | Write | + Verification |
|----------|-------|----------------|
| 1 GB     | ~35s  | ~70s           |
| 2 GB     | ~70s  | ~140s          |
| 4 GB     | ~140s | ~280s          |
| 8 GB     | ~280s | ~560s          |

### USB 3.0 (~100 MB/s)

| ISO Size | Write | + Verification |
|----------|-------|----------------|
| 1 GB     | ~10s  | ~20s           |
| 2 GB     | ~20s  | ~40s           |
| 4 GB     | ~40s  | ~80s           |
| 8 GB     | ~80s  | ~160s          |

## Troubleshooting

### Error: "Permission denied"

```
❌ Root privileges required!

💡 Please run with sudo:
   sudo flicker write --iso <file> --device <device>
```

**Solution:** Add `sudo` before the command

---

### Error: "ISO file not found"

```
❌ ISO file does not exist: "/path/to/file.iso"
```

**Solutions:**
1. Check the path: `ls -lh /path/to/file.iso`
2. Use absolute path: `~/Downloads/file.iso` or `/home/user/Downloads/file.iso`
3. Use tab completion to avoid typos

---

### Error: "Device does not exist"

```
❌ Device does not exist: /dev/sdb
```

**Solutions:**
1. List devices: `lsblk` or `flicker list`
2. Check if USB is connected
3. It could be `/dev/sdc`, `/dev/sdd`, etc

---

### Error: "ISO file is larger than device"

```
❌ ISO file (8.5 GB) is larger than device (7.45 GB)!
```

**Solution:** Use a larger USB

---

### Error: "Device or resource busy"

```
❌ Failed to open device: Device or resource busy
```

**Cause:** Device mounted but failed to unmount

**Solutions:**
```bash
# Unmount manually
sudo umount /dev/sdb1
sudo umount /dev/sdb2

# Or use force
sudo umount -f /dev/sdb*

# Then try again
sudo flicker write ...
```

---

### Error: "I/O error"

```
❌ Failed to write to device: I/O error
```

**Possible causes:**
1. Faulty USB
2. USB disconnected during writing
3. USB port problem

**Solutions:**
1. Try a different USB
2. Try a different USB port
3. Check dmesg: `dmesg | tail -20`

---

### Error: "Verification failed"

```
❌ Verification failed: data mismatch at offset 1048576
```

**Causes:**
1. Faulty USB
2. Write error
3. Corrupted ISO

**Solutions:**
1. Check ISO checksum: `sha256sum file.iso`
2. Re-download ISO
3. Try another USB
4. Write again without verification (not recommended)

## Safety Validations

### 1. USB Verification

If device is not detected as USB:

```
⚠️  WARNING: /dev/sda does not appear to be a removable USB device!
   This might be an internal disk!

❓ Are you ABSOLUTELY SURE you want to continue? (type 'YES' in capitals):
```

**Type 'YES' only if:**
- You are absolutely sure it's the correct device
- You are using it in a VM or test environment
- You really want to write to an internal disk (rare cases)

### 2. Final Confirmation

```
============================================================
⚠️  FINAL WARNING ⚠️
============================================================

This will COMPLETELY ERASE all data on:
   Device: /dev/sdb
   Size:   14.92 GB

❓ Type 'yes' to confirm and start writing:
```

**Before typing 'yes':**
- ✅ Check the device (is `/dev/sdb` correct?)
- ✅ Confirm the size (matches your USB?)
- ✅ Make sure there's no important data
- ✅ Double-checking is never too much!

## Tips

### 1. Always Check First

```bash
# Before writing
flicker list -v

# Look for your USB in the list
# Note the correct path (/dev/sdX)
```

### 2. Use Tab Completion

```bash
# Instead of typing everything:
sudo flicker write --iso ~/Down<TAB>

# Bash completes automatically
sudo flicker write --iso ~/Downloads/
```

### 3. Verification For Important ISOs

```bash
# For operating systems, always use --verify
sudo flicker write \
    --iso ubuntu-server.iso \
    --device /dev/sdb \
    --verify
```

### 4. Scripts/Automation

```bash
#!/bin/bash
# Script to write to multiple USBs

ISO="ubuntu-24.04.iso"

for device in /dev/sdb /dev/sdc /dev/sdd; do
    echo "Writing to $device..."
    sudo flicker write \
        --iso "$ISO" \
        --device "$device" \
        --yes \
        --verify
done
```

### 5. Check Progress in Another Window

```bash
# Terminal 1: Write
sudo flicker write --iso ubuntu.iso --device /dev/sdb

# Terminal 2: Monitor I/O
watch -n 1 'iostat -x 1 1 | grep sdb'
```

## Comparison with Other Tools

### vs dd

```bash
# dd - traditional command
sudo dd if=ubuntu.iso of=/dev/sdb bs=4M status=progress && sync

# Flicker - safer and more user-friendly
sudo flicker write --iso ubuntu.iso --device /dev/sdb --verify
```

**Flicker advantages:**
- ✅ Automatic validations
- ✅ Automatic unmounting
- ✅ Integrated verification
- ✅ Better progress bar
- ✅ Clear messages
- ✅ Error protection

## FAQ

**Q: Do I need to use sudo?**  
A: Yes, always. Writing to devices requires root privileges.

**Q: Can I write to /dev/sdb1?**  
A: No! Use the whole device (`/dev/sdb`), not partitions (`/dev/sdb1`).

**Q: Will the USB be bootable?**  
A: Yes, if the ISO is bootable (e.g. Linux, Windows installers).

**Q: How long does it take?**  
A: It depends on ISO size and USB speed. See times table above.

**Q: Can I use the computer during writing?**  
A: Yes, but avoid heavy I/O operations. Don't shutdown or suspend the PC.

**Q: What if the process hangs?**  
A: Wait a few minutes. If truly stuck, Ctrl+C and try again.

**Q: How to check if the USB is bootable?**  
A: Test in a VM or restart the PC and boot from the USB.

## References

- [Technical Documentation](ISO_WRITING.md)
- [Device Detection](USB_DETECTION.md)
- [list Command](COMMAND_LIST.md)
- [Graphical Interface (GUI)](GUI_INTERFACE.md)
