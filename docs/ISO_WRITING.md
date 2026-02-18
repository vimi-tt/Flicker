# 🔥 ISO Writing - Technical Documentation

## Overview

The `writer` module implements safe ISO file writing to USB devices, with verification and visual progress.

## Execution Flow

```
┌─────────────────────────────────────────────────┐
│ 1. Security Checks                              │
│    ├─ Root permissions                         │
│    ├─ ISO file validation                      │
│    ├─ Device validation                        │
│    ├─ Check if really USB                      │
│    └─ Check sufficient space                   │
└────────────────┬────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────┐
│ 2. Preparation                                  │
│    ├─ Unmount device (if mounted)             │
│    └─ User confirmation                        │
└────────────────┬────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────┐
│ 3. Writing                                      │
│    ├─ Open ISO for reading                     │
│    ├─ Open device for writing                  │
│    ├─ Read/write 4MB chunks                    │
│    ├─ Update progress bar                      │
│    └─ Sync to ensure physical write            │
└────────────────┬────────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────────┐
│ 4. Verification (optional)                      │
│    ├─ Read ISO again                           │
│    ├─ Read data written to device              │
│    ├─ Compare byte by byte                     │
│    └─ Report success or error                  │
└─────────────────────────────────────────────────┘
```

## Security Validations

### 1. Root Permissions

```rust
utils::require_root_privileges()?;
```

**Why?** Writing to `/dev/sdX` requires superuser privileges.

**Behavior:**
- If not root: error with message instructing to use `sudo`
- If root: continues

### 2. ISO File Validation

```rust
utils::validate_iso_file(iso)?;
```

**Checks:**
- ✅ File exists
- ✅ Is a regular file (not directory)
- ✅ Minimum size (> 1MB)
- ⚠️ Extension .iso or .img (warning if different)

### 3. Device Validation

```rust
utils::validate_device_path(device)?;
```

**Checks:**
- ✅ Device exists
- ✅ Is a block device (not regular file)

### 4. USB Verification

```rust
let is_usb = usb_devices.iter().any(|d| d.name == device_name);
```

**Protects against:**
- ❌ Accidentally writing to internal disk
- ❌ Writing to system partitions

**Behavior:**
- If not USB: requires extra confirmation (type "YES" in uppercase)
- If USB: continues normally

### 5. Space Verification

```rust
if iso_size > usb_dev.size {
    anyhow::bail!("ISO file is larger than device!");
}
```

**Prevents:**
- ❌ Incomplete write due to lack of space

### 6. Automatic Unmounting

```rust
if usb_dev.is_mounted {
    utils::unmount_device(&device_name)?;
}
```

**Why?**
- Mounted devices cannot be written to directly
- Prevents data corruption

## Writing Process

### Buffer Size

```rust
const BUFFER_SIZE: usize = 4 * 1024 * 1024; // 4MB
```

**Why 4MB?**
- ✅ Balance between memory and performance
- ✅ Reduces system call overhead
- ✅ Works well with USB 2.0 and 3.0

### Reading and Writing

```rust
loop {
    let bytes_read = iso_file.read(&mut buffer)?;
    if bytes_read == 0 { break; }
    
    device_file.write_all(&buffer[..bytes_read])?;
    total_written += bytes_read as u64;
    progress.set_position(total_written);
}
```

**Flow:**
1. Read 4MB chunk from ISO
2. Write chunk to device
3. Update progress
4. Repeat until end of file

### Synchronization (Sync)

```rust
device_file.sync_all()?;
```

**What it does:**
- Forces the kernel to write all cached data to the physical device
- Ensures data is not lost if USB is removed

**Why it's important:**
- Linux uses aggressive write caching
- Data can stay in RAM for seconds/minutes
- `sync_all()` guarantees complete physical write

## Progress Bar

### Bar Style

```rust
{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] 
{bytes}/{total_bytes} ({eta})
```

**Shows:**
- 🔄 Animated spinner
- ⏱️ Elapsed time
- 📊 Visual progress bar
- 📈 Bytes written / total
- ⏰ Estimated time remaining (ETA)

**Example:**
```
📝 Writing ISO to device...
⠁ [00:01:23] [########>---------] 512MB/2GB (00:02:15)
```

## Post-Write Verification

### Process

```rust
verify_write(iso_path, device_path, iso_size)?;
```

**How it works:**
1. Reopens ISO file in read mode
2. Opens device in read mode
3. Reads both in 4MB chunks
4. Compares byte by byte
5. Reports any differences

**When to use:**
- ✅ Critical ISOs (operating systems, installers)
- ✅ When you need to guarantee 100% integrity
- ❌ Can double total time (reads everything again)

### Error Detection

```rust
if iso_buffer[..iso_read] != device_buffer[..device_read] {
    anyhow::bail!("Verification failed at offset {}", total_verified);
}
```

**Reports:**
- Exact position where data differs
- Allows debugging of hardware issues

## Performance

### Typical Speeds

| USB Type | Theoretical Speed | Real Speed (Flicker) |
|----------|-------------------|----------------------|
| USB 2.0  | 60 MB/s           | 30-40 MB/s           |
| USB 3.0  | 640 MB/s          | 100-200 MB/s         |
| USB 3.1  | 1250 MB/s         | 200-400 MB/s         |

**Note:** Real speed depends on:
- USB quality
- PC USB controller
- Data type (compressible or not)

### Speed Calculation

```rust
pub fn calculate_speed(bytes: u64, elapsed_secs: f64) -> String {
    let bytes_per_sec = bytes as f64 / elapsed_secs;
    format!("{:.2} MB/s", bytes_per_sec / (1024.0 * 1024.0))
}
```

## Error Handling

### Possible Errors

1. **Permission denied**
   - Cause: Not run with sudo
   - Solution: `sudo flicker write ...`

2. **Device or resource busy**
   - Cause: Device still mounted
   - Solution: Flicker unmounts automatically

3. **No space left on device**
   - Cause: ISO larger than USB
   - Solution: Flicker checks before starting

4. **I/O error**
   - Cause: Faulty USB or disconnected
   - Solution: Try different USB or reconnect

5. **Verification failed**
   - Cause: Write error or faulty USB
   - Solution: Try again or try different USB

## Comparison with `dd`

### Equivalent dd command

```bash
sudo dd if=/path/to/iso of=/dev/sdb bs=4M status=progress && sync
```

## Usage Examples

### Basic

```bash
sudo flicker write --iso ubuntu-24.04.iso --device /dev/sdb
```

### With verification

```bash
sudo flicker write --iso ubuntu-24.04.iso --device /dev/sdb --verify
```

### Without confirmation (scripts)

```bash
sudo flicker write --iso ubuntu-24.04.iso --device /dev/sdb --yes
```

### Complete workflow

```bash
# 1. List devices
flicker list -v

# 2. Identify correct USB (e.g. /dev/sdb)

# 3. Write with verification
sudo flicker write \
    --iso ~/Downloads/ubuntu-24.04.iso \
    --device /dev/sdb \
    --verify
```

### System Calls Used

```rust
// Open file
File::open()  → open() syscall

// Read data
file.read()   → read() syscall

// Write data
file.write()  → write() syscall

// Sync
file.sync_all() → fsync() syscall

// Metadata
file.metadata() → stat() syscall
```

## Future Improvements

- [ ] Direct I/O (O_DIRECT) for cache bypass
- [ ] Parallelization (simultaneous read + write)
- [ ] Support for multiple USBs in parallel
- [ ] On-the-fly compression
- [ ] Automatic retry on error
- [ ] MBR backup before writing
- [ ] ISO checksum verification before writing

## References

- [Linux I/O Documentation](https://www.kernel.org/doc/Documentation/filesystems/)
- [sync(2) man page](https://man7.org/linux/man-pages/man2/sync.2.html)
