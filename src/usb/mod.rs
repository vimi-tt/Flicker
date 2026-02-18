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
        let size = self.size as f64;
        
        if size < 1024.0 {
            format!("{} B", size)
        } else if size < 1024.0 * 1024.0 {
            format!("{:.2} KB", size / 1024.0)
        } else if size < 1024.0 * 1024.0 * 1024.0 {
            format!("{:.2} MB", size / (1024.0 * 1024.0))
        } else if size < 1024.0 * 1024.0 * 1024.0 * 1024.0 {
            format!("{:.2} GB", size / (1024.0 * 1024.0 * 1024.0))
        } else {
            format!("{:.2} TB", size / (1024.0 * 1024.0 * 1024.0 * 1024.0))
        }
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
        
        // Skip loop devices, ram devices, etc.
        if device_name_str.starts_with("loop")
            || device_name_str.starts_with("ram")
            || device_name_str.starts_with("dm-")
        {
            continue;
        }
        
        let device_path = entry.path();
        
        // Check if it's a removable device (USB)
        if let Ok(device) = check_if_usb_device(&device_path, &device_name_str) {
            devices.push(device);
        }
    }
    
    Ok(devices)
}

/// Check if a block device is a USB device and return its information
fn check_if_usb_device(device_path: &std::path::Path, device_name: &str) -> Result<UsbDevice> {
    // Check if removable
    let removable_path = device_path.join("removable");
    let is_removable = std::fs::read_to_string(&removable_path)
        .unwrap_or_default()
        .trim() == "1";
    
    if !is_removable {
        anyhow::bail!("Not a removable device");
    }
    
    // Get device size
    let size = read_device_size(device_path)?;
    
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
    
    let mut mount_points = Vec::new();
    
    for line in mounts.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }
        
        // Check if this line is for our device or its partitions
        let mount_device = parts[0];
        if mount_device.contains(device_name) {
            mount_points.push(parts[1].to_string());
        }
    }
    
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