use anyhow::{Context, Result};
use std::path::PathBuf;

/// Represents a USB storage device
#[derive(Debug, Clone)]
pub struct UsbDevice {
    /// Device path (e.g., /dev/sdb)
    pub path: PathBuf,
    
    /// Device name (e.g., sdb)
    pub name: String,
    
    /// Total size in bytes
    pub size: u64,
    
    /// Vendor name
    pub vendor: String,
    
    /// Model name
    pub model: String,
    
    /// Whether the device is currently mounted
    pub is_mounted: bool,
    
    /// Mount points if mounted
    pub mount_points: Vec<String>,
    
    /// Device serial number (if available)
    pub serial: Option<String>,
}

impl UsbDevice {
    /// Format size in human-readable format
    pub fn size_human(&self) -> String {
        crate::utils::format_size(self.size)
    }
    
    /// Get a short description of the device
    pub fn description(&self) -> String {
        format!("{} {} ({})", self.vendor.trim(), self.model.trim(), self.size_human())
    }
}

/// List all USB storage devices
pub fn list_usb_devices() -> Result<Vec<UsbDevice>> {
    let mut devices = Vec::new();
    
    // Read block devices from /sys/block
    let sys_block = std::path::Path::new("/sys/block");
    
    if !sys_block.exists() {
        anyhow::bail!("Cannot access /sys/block - are you running on Linux?");
    }
    
    for entry in std::fs::read_dir(sys_block).context("Failed to read /sys/block")? {
        let entry = entry?;
        let device_name = entry.file_name();
        let device_name_str = device_name.to_string_lossy();
        
        // Skip loop, ram, device-mapper, optical (sr), zram, floppy, and md RAID devices
        if device_name_str.starts_with("loop")
            || device_name_str.starts_with("ram")
            || device_name_str.starts_with("dm-")
            || device_name_str.starts_with("sr")
            || device_name_str.starts_with("zram")
            || device_name_str.starts_with("fd")
            || device_name_str.starts_with("md")
        {
            continue;
        }
        
        let device_path = entry.path();
        
        // Check if it's a USB or removable storage device
        if let Ok(device) = check_if_usb_device(&device_path, &device_name_str) {
            devices.push(device);
        }
    }
    
    devices.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(devices)
}

/// Check if a block device is connected over a USB bus by inspecting its sysfs canonical path
fn is_usb_bus(device_path: &std::path::Path) -> bool {
    std::fs::canonicalize(device_path)
        .ok()
        .map(|resolved| resolved.to_string_lossy().contains("/usb"))
        .unwrap_or(false)
}

/// Check if a block device is a USB/removable storage device and return its information
fn check_if_usb_device(device_path: &std::path::Path, device_name: &str) -> Result<UsbDevice> {
    // Check if connected over USB bus or flagged as removable
    let removable_path = device_path.join("removable");
    let is_removable = std::fs::read_to_string(&removable_path)
        .unwrap_or_default()
        .trim()
        == "1";

    if !is_usb_bus(device_path) && !is_removable {
        anyhow::bail!("Not a USB or removable device");
    }

    // Get device size and ignore empty media (e.g., empty SD card readers)
    let size = read_device_size(device_path)?;
    if size == 0 {
        anyhow::bail!("Device has zero size (no media present)");
    }
    
    // Get vendor and model
    let vendor = read_sysfs_value(device_path, "device/vendor").unwrap_or_else(|_| "Unknown".to_string());
    let model = read_sysfs_value(device_path, "device/model").unwrap_or_else(|_| "Device".to_string());
    
    // Get serial if available
    let serial = read_sysfs_value(device_path, "device/serial").ok();
    
    // Check if mounted
    let (is_mounted, mount_points) = check_mount_status(device_name)?;
    
    Ok(UsbDevice {
        path: PathBuf::from(format!("/dev/{}", device_name)),
        name: device_name.to_string(),
        size,
        vendor,
        model,
        is_mounted,
        mount_points,
        serial,
    })
}

/// Read device size from sysfs
fn read_device_size(device_path: &std::path::Path) -> Result<u64> {
    let size_path = device_path.join("size");
    let size_str = std::fs::read_to_string(&size_path)
        .context("Failed to read device size")?;
    
    let sectors: u64 = size_str.trim().parse()
        .context("Failed to parse device size")?;
    
    // Size is in 512-byte sectors
    Ok(sectors * 512)
}

/// Read a value from sysfs
fn read_sysfs_value(device_path: &std::path::Path, subpath: &str) -> Result<String> {
    let full_path = device_path.join(subpath);
    let value = std::fs::read_to_string(&full_path)
        .context(format!("Failed to read {}", subpath))?;
    
    Ok(value.trim().to_string())
}

/// Check if device is mounted and return mount points
fn check_mount_status(device_name: &str) -> Result<(bool, Vec<String>)> {
    let mounts = std::fs::read_to_string("/proc/mounts")
        .context("Failed to read /proc/mounts")?;

    let mount_points: Vec<String> = crate::utils::parse_mounts_for_device(&mounts, device_name)
        .into_iter()
        .map(|(_, mp)| mp)
        .collect();

    Ok((!mount_points.is_empty(), mount_points))
}

/// Display devices in a formatted list
pub fn display_devices(devices: &[UsbDevice], verbose: bool) {
    if devices.is_empty() {
        println!("❌ No USB devices found");
        println!("\n💡 Tips:");
        println!("   • Make sure your USB device is connected");
        println!("   • Try running with sudo if you don't see expected devices");
        return;
    }
    
    println!("📀 Available USB devices:\n");
    
    for device in devices {
        print!("  {} - {}", device.path.display(), device.description());
        
        if device.is_mounted {
            print!(" ⚠️  [MOUNTED]");
        }
        println!();
        
        if verbose {
            println!("    ├─ Name:     {}", device.name);
            println!("    ├─ Vendor:   {}", device.vendor.trim());
            println!("    ├─ Model:    {}", device.model.trim());
            println!("    ├─ Size:     {} ({} bytes)", device.size_human(), device.size);
            
            if let Some(ref serial) = device.serial {
                println!("    ├─ Serial:   {}", serial);
            }
            
            if device.is_mounted {
                println!("    ├─ Mounted:  Yes");
                for (i, mount_point) in device.mount_points.iter().enumerate() {
                    if i == device.mount_points.len() - 1 {
                        println!("    └─ Mount[{}]: {}", i, mount_point);
                    } else {
                        println!("    ├─ Mount[{}]: {}", i, mount_point);
                    }
                }
            } else {
                println!("    └─ Mounted:  No");
            }
            
            println!();
        }
    }
    
    println!("\n💡 Use 'flicker write --iso <file> --device <device>' to write an ISO");
    
    // Warning about mounted devices
    let mounted_count = devices.iter().filter(|d| d.is_mounted).count();
    if mounted_count > 0 {
        println!("\n⚠️  {} device(s) are currently mounted", mounted_count);
        println!("   Flicker will automatically unmount them before writing");
    }
}

/// Spawn a background watcher that invokes `on_change` whenever a block device is added or removed.
///
/// Uses `udevadm monitor --udev --subsystem-match=block` when available, falling back to
/// lightweight `/sys/block` polling.
#[cfg(feature = "gui")]
pub fn spawn_usb_hotplug_watcher(on_change: impl Fn() + Send + 'static) {
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    std::thread::spawn(move || {
        if let Ok(mut child) = Command::new("udevadm")
            .args(["monitor", "--udev", "--subsystem-match=block"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        {
            if let Some(stdout) = child.stdout.take() {
                let reader = BufReader::new(stdout);
                let mut last_notify = Instant::now() - Duration::from_secs(10);
                for line in reader.lines().map_while(Result::ok) {
                    if (line.contains(" add ") || line.contains(" remove "))
                        && last_notify.elapsed() >= Duration::from_millis(500)
                    {
                        last_notify = Instant::now();
                        // Give kernel/sysfs a moment to settle device nodes and mount points
                        std::thread::sleep(Duration::from_millis(400));
                        on_change();
                    }
                }
            }
            let _ = child.kill();
            let _ = child.wait();
            return;
        }

        // Fallback: poll `/sys/block` every 2 seconds
        let mut prev_snapshot: Vec<String> = Vec::new();
        loop {
            std::thread::sleep(Duration::from_secs(2));
            let mut current: Vec<String> = std::fs::read_dir("/sys/block")
                .ok()
                .map(|entries| {
                    entries
                        .flatten()
                        .map(|e| e.file_name().to_string_lossy().into_owned())
                        .collect()
                })
                .unwrap_or_default();
            current.sort();
            if current != prev_snapshot {
                prev_snapshot = current;
                on_change();
            }
        }
    });
}