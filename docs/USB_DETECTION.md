# 🔍 USB Device Detection - Technical Documentation

## How It Works

Flicker detects USB devices on Linux by directly reading from the `/sys` filesystem, which is a Linux kernel interface to expose hardware information.

## Detection Architecture

### 1. Device Location (`/sys/block`)

The Linux kernel exposes all block devices in `/sys/block/`. Each device has its own directory:

```
/sys/block/
├── sda/          # Internal disk (SATA/NVMe)
├── sdb/          # Possible USB
├── sdc/          # Possible USB
├── loop0/        # Loop device (ignored)
└── ram0/         # RAM disk (ignored)
```

### 2. USB Device Identification

To identify if a device is USB (removable), we check:

```rust
/sys/block/sdb/removable
```

If the content is `"1"`, it's a removable device (USB).

### 3. Information Collection

For each USB device, we collect:

#### a) Size
```rust
/sys/block/sdb/size  // In 512-byte sectors
```

We convert sectors → bytes → human-readable format (GB, TB, etc.)

#### b) Manufacturer (Vendor)
```rust
/sys/block/sdb/device/vendor
```
Example: `"SanDisk"`, `"Kingston"`, `"Samsung"`

#### c) Model
```rust
/sys/block/sdb/device/model
```
Example: `"Ultra"`, `"DataTraveler 3.0"`

#### d) Serial Number (optional)
```rust
/sys/block/sdb/device/serial
```

### 4. Mount Verification

We read `/proc/mounts` to check if the device or its partitions are mounted:

```rust
/proc/mounts
```

Example content:
```
/dev/sdc1 /media/user/USB ext4 rw,relatime 0 0
/dev/sdc2 /media/user/DATA vfat rw,relatime 0 0
```

We look for lines containing the device name (e.g., `sdc`).

## Data Structure

```rust
pub struct UsbDevice {
    pub path: PathBuf,           // /dev/sdb
    pub name: String,            // sdb
    pub size: u64,               // bytes
    pub vendor: String,          // SanDisk
    pub model: String,           // Ultra
    pub is_mounted: bool,        // true/false
    pub mount_points: Vec<String>, // ["/media/user/USB"]
    pub serial: Option<String>,  // Optional
}
```

## Execution Flow

```
┌─────────────────────────────────────┐
│ 1. Read /sys/block                  │
│    List all devices                 │
└──────────────┬──────────────────────┘
               │
               ▼
┌─────────────────────────────────────┐
│ 2. Filter devices                   │
│    • Ignore loop*, ram*, dm-*       │
│    • Check removable == 1           │
└──────────────┬──────────────────────┘
               │
               ▼
┌─────────────────────────────────────┐
│ 3. Collect information              │
│    • Size (size)                    │
│    • Vendor                         │
│    • Model                          │
│    • Serial (if available)          │
└──────────────┬──────────────────────┘
               │
               ▼
┌─────────────────────────────────────┐
│ 4. Check mount status               │
│    • Read /proc/mounts              │
│    • Look for device_name           │
│    • Extract mount points           │
└──────────────┬──────────────────────┘
               │
               ▼
┌─────────────────────────────────────┐
│ 5. Create UsbDevice struct          │
│    Return Vec<UsbDevice>            │
└─────────────────────────────────────┘
```

## Output Example

### Normal Mode
```
📀 Available USB devices:

  /dev/sdb - SanDisk Ultra (14.92 GB)
  /dev/sdc - Kingston DataTraveler (29.84 GB) ⚠️  [MOUNTED]
```

### Verbose Mode (`--verbose`)
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
```

## Error Handling

The code uses `anyhow::Result` for error handling:

- **No permission**: Some files in `/sys` may require sudo
- **Device not found**: May have been removed during reading
- **Parse failed**: Malformed data in sysfs

## Known Limitations

1. **Linux only**: Uses Linux-specific APIs (`/sys`, `/proc`)
2. **Requires /sys access**: Usually available without root
3. **Virtual devices**: We filter loop/ram/dm but others may exist
4. **Hot-plug**: Doesn't detect changes in real-time (needs to run again)

## Future Improvements

- [ ] Use `udev` for real-time detection
- [ ] Support for removable NVMe
- [ ] SD card detection
- [ ] Device caching
- [ ] Hot-plug notifications

## Useful Linux Commands

For debugging and comparison:

```bash
# List block devices
lsblk

# View detailed information
sudo fdisk -l

# View USB devices only
lsusb

# Sysfs information
cat /sys/block/sdb/removable
cat /sys/block/sdb/size
cat /sys/block/sdb/device/vendor
cat /sys/block/sdb/device/model

# View mounts
cat /proc/mounts | grep sdb
mount | grep sdb
```

## References

- [Linux Kernel Documentation - sysfs](https://www.kernel.org/doc/Documentation/filesystems/sysfs.txt)
- [proc(5) man page](https://man7.org/linux/man-pages/man5/proc.5.html)
- [Device File System (devfs)](https://en.wikipedia.org/wiki/Device_file)
