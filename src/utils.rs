use anyhow::{Context, Result};
use std::path::Path;

/// Check if running as root/sudo
pub fn check_root_privileges() -> bool {
    // Check if effective user ID is 0 (root)
    unsafe { libc::geteuid() == 0 }
}

/// Ensure the user has root privileges, or return an error
pub fn require_root_privileges() -> Result<()> {
    if !check_root_privileges() {
        anyhow::bail!(
            "❌ Root privileges required!\n\n\
             💡 Please run with sudo:\n\
                sudo flicker write --iso <file> --device <device>"
        );
    }
    Ok(())
}

/// Check if a device path is valid and exists
pub fn validate_device_path(device: &Path) -> Result<()> {
    if !device.exists() {
        anyhow::bail!("❌ Device does not exist: {:?}", device);
    }
    
    // Check if it's a block device
    let metadata = std::fs::metadata(device)
        .context("Failed to get device metadata")?;
    
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::FileTypeExt;
        if !metadata.file_type().is_block_device() {
            anyhow::bail!("❌ {:?} is not a block device", device);
        }
    }
    
    Ok(())
}

/// Check if a file is a valid ISO
pub fn validate_iso_file(iso_path: &Path) -> Result<()> {
    if !iso_path.exists() {
        anyhow::bail!("❌ ISO file does not exist: {:?}", iso_path);
    }
    
    if !iso_path.is_file() {
        anyhow::bail!("❌ {:?} is not a file", iso_path);
    }
    
    // Check file size (should be at least a few MB)
    let metadata = std::fs::metadata(iso_path)
        .context("Failed to get ISO file metadata")?;
    
    let size = metadata.len();
    if size < 1024 * 1024 {
        anyhow::bail!("❌ File seems too small to be a valid ISO ({})", format_size(size));
    }
    
    // Optional: Check file extension
    if let Some(ext) = iso_path.extension() {
        let ext_lower = ext.to_string_lossy().to_lowercase();
        if ext_lower != "iso" && ext_lower != "img" {
            println!("⚠️  Warning: File extension is not .iso or .img");
        }
    }
    
    Ok(())
}

/// Format size in human-readable format
pub fn format_size(size: u64) -> String {
    let size_f = size as f64;
    
    if size_f < 1024.0 {
        format!("{} B", size)
    } else if size_f < 1024.0 * 1024.0 {
        format!("{:.2} KB", size_f / 1024.0)
    } else if size_f < 1024.0 * 1024.0 * 1024.0 {
        format!("{:.2} MB", size_f / (1024.0 * 1024.0))
    } else if size_f < 1024.0 * 1024.0 * 1024.0 * 1024.0 {
        format!("{:.2} GB", size_f / (1024.0 * 1024.0 * 1024.0))
    } else {
        format!("{:.2} TB", size_f / (1024.0 * 1024.0 * 1024.0 * 1024.0))
    }
}

/// Read block device size in bytes from sysfs (`/sys/block/<name>/size`)
pub fn get_block_device_size(device_name: &str) -> Result<u64> {
    let size_path = Path::new("/sys/block").join(device_name).join("size");
    let size_str = std::fs::read_to_string(&size_path)
        .with_context(|| format!("Failed to read device size from {}", size_path.display()))?;

    let sectors: u64 = size_str
        .trim()
        .parse()
        .context("Failed to parse device sector count")?;

    Ok(sectors.saturating_mul(512))
}

/// Check if a mount source from `/proc/mounts` belongs to `device_name` or one of its partitions.
///
/// Prevents substring false positives such as matching `/dev/sdaa` when targeting `sda`.
pub fn is_mount_for_device(mount_device: &str, device_name: &str) -> bool {
    let Some(dev) = mount_device.strip_prefix("/dev/") else {
        return false;
    };

    if dev == device_name {
        return true;
    }

    let Some(suffix) = dev.strip_prefix(device_name) else {
        return false;
    };

    if suffix.is_empty() {
        return true;
    }

    // Devices ending with a digit (e.g., nvme0n1, mmcblk0) use 'p<digits>' for partitions.
    if device_name.chars().last().is_some_and(|c| c.is_ascii_digit()) {
        suffix
            .strip_prefix('p')
            .is_some_and(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
    } else {
        // Traditional SCSI/SATA/USB devices (e.g., sda, sdb) use '<digits>' for partitions.
        suffix.chars().all(|c| c.is_ascii_digit())
    }
}

/// Parse `/proc/mounts` content and return `(device_path, mount_point)` entries for `device_name`.
pub fn parse_mounts_for_device(mounts_content: &str, device_name: &str) -> Vec<(String, String)> {
    let mut mounted = Vec::new();

    for line in mounts_content.lines() {
        let mut parts = line.split_whitespace();
        let (Some(mount_device), Some(mount_point)) = (parts.next(), parts.next()) else {
            continue;
        };

        if is_mount_for_device(mount_device, device_name) {
            let decoded_mount = mount_point.replace("\\040", " ");
            mounted.push((mount_device.to_string(), decoded_mount));
        }
    }

    mounted
}

/// Unmount all partitions of a device
pub fn unmount_device(device_name: &str) -> Result<()> {
    let mounts = std::fs::read_to_string("/proc/mounts")
        .context("Failed to read /proc/mounts")?;

    let mounted_partitions = parse_mounts_for_device(&mounts, device_name);

    if mounted_partitions.is_empty() {
        return Ok(());
    }

    println!("\n🔓 Unmounting {} partition(s)...", mounted_partitions.len());

    for (device, mount_point) in mounted_partitions {
        print!("   Unmounting {} from {}... ", device, mount_point);

        let output = std::process::Command::new("umount")
            .arg(&device)
            .output()
            .context("Failed to execute umount")?;

        if output.status.success() {
            println!("✓");
        } else {
            let error = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to unmount {}: {}", device, error.trim());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_format_size() {
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(1024), "1.00 KB");
        assert_eq!(format_size(1024 * 1024), "1.00 MB");
        assert_eq!(format_size(1024 * 1024 * 1024), "1.00 GB");
    }
}