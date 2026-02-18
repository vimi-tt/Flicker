# 📋 Usage Guide - `list` Command

## Basic Syntax

```bash
flicker list [OPTIONS]
```

## Options

- `--verbose`, `-v` : Display detailed information about each device

## Usage Examples

### 1. Simple List

```bash
flicker list
```

**Expected output:**
```
📀 Available USB devices:

  /dev/sdb - SanDisk Ultra (14.92 GB)
  /dev/sdc - Kingston DataTraveler (29.84 GB) ⚠️  [MOUNTED]

💡 Use 'flicker write --iso <file> --device <device>' to write an ISO

⚠️  1 device(s) are currently mounted
   Flicker will automatically unmount them before writing
```

### 2. Detailed List (Verbose)

```bash
flicker list --verbose
```

or

```bash
flicker list -v
```

**Expected output:**
```
📀 Available USB devices:

  /dev/sdb - SanDisk Ultra (14.92 GB)
    ├─ Name:     sdb
    ├─ Vendor:   SanDisk
    ├─ Model:    Ultra
    ├─ Size:     14.92 GB (16013942784 bytes)
    ├─ Serial:   4C530001234567890123
    └─ Mounted:  No

  /dev/sdc - Kingston DataTraveler (29.84 GB) ⚠️  [MOUNTED]
    ├─ Name:     sdc
    ├─ Vendor:   Kingston
    ├─ Model:    DataTraveler 3.0
    ├─ Size:     29.84 GB (32026656768 bytes)
    ├─ Mounted:  Yes
    ├─ Mount[0]: /media/user/KINGSTON
    └─ Mount[1]: /media/user/DATA

💡 Use 'flicker write --iso <file> --device <device>' to write an ISO

⚠️  1 device(s) are currently mounted
   Flicker will automatically unmount them before writing
```

## Understanding the Output

### Symbols and Indicators

- 📀 - Indicates devices section
- ⚠️ [MOUNTED] - Device is mounted (needs to be unmounted before writing)
- ├─ - Additional information (verbose)
- └─ - Last additional information (verbose)

### Displayed Information

#### Normal Mode
- Device path (`/dev/sdb`)
- Description (Vendor + Model + Size)
- Mount status

#### Verbose Mode (additional)
- Device internal name
- Manufacturer (Vendor)
- Model
- Size in bytes and human-readable format
- Serial number (if available)
- Mount points (if mounted)

## Use Cases

### Before Writing an ISO

Always list devices first to:
1. Identify the correct device
2. Check the size (ensure the ISO fits)
3. See if it's mounted
4. Confirm the exact path (`/dev/sdX`)

```bash
# Step 1: List devices
flicker list -v

# Step 2: Identify the correct device
# Example: /dev/sdb is my 16GB USB

# Step 3: Write
sudo flicker write --iso ubuntu.iso --device /dev/sdb
```

### Troubleshooting

#### "I don't see my USB device"

1. Check if the USB is physically connected
2. Run `lsblk` to see all devices
3. Try with sudo: `sudo flicker list -v`
4. Check `dmesg | tail` for kernel errors

```bash
# View kernel logs about USB
dmesg | grep -i usb | tail -20
```

#### "Device appears but without information"

Some devices may have empty fields in `/sys`. This is normal for:
- Very old devices
- Generic devices without proper firmware
- USB-SATA adapters

Use `lsblk` and `sudo fdisk -l` for additional information:

```bash
lsblk -o NAME,SIZE,TYPE,MOUNTPOINT,VENDOR,MODEL
sudo fdisk -l /dev/sdb
```

#### "Permission denied"

Normally sudo is not needed to list, but if you encounter issues:

```bash
sudo flicker list -v
```

## Tips

### Identify the Correct Device

1. **By size**: USBs have known sizes (8GB, 16GB, 32GB, etc)
2. **By brand**: Check the physical brand on the USB
3. **By mount**: If mounted, see files in `/media` or `/mnt`

### Recommended Workflow

```bash
# 1. BEFORE connecting the USB
lsblk

# 2. Connect the USB

# 3. List again
flicker list

# 4. The new device is your USB!
```

### Useful Aliases

Add to your `~/.bashrc`:

```bash
alias flist='flicker list -v'
alias fusb='flicker list'
```

Then you can use:
```bash
flist    # detailed listing
fusb     # quick listing
```

## FAQ

**Q: Do I need sudo?**  
A: No for listing. Sudo is only needed for writing.

**Q: Why doesn't my internal disk appear?**  
A: Flicker filters only removable devices (USB). Internal disks are ignored for safety.

**Q: What about SD cards?**  
A: Depending on the reader, they may appear as `/dev/mmcblk*`. Support in development.

**Q: Can I use it with external NVMe?**  
A: Yes, if it appears as removable in `/sys/block/*/removable`.

## References

- [Technical Documentation](USB_DETECTION.md)
- [write Command](COMMAND_WRITE.md)
- [verify Command](COMMAND_VERIFY.md)
