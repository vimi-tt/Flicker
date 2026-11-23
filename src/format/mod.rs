use anyhow::{Context, Result};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::Sender;
use std::time::Duration;

use crate::writer::ProgressMsg;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartitionScheme {
    Gpt,
    Mbr,
}

impl PartitionScheme {
    pub fn parse(s: &str) -> Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "gpt" => Ok(Self::Gpt),
            "mbr" | "msdos" | "dos" => Ok(Self::Mbr),
            other => anyhow::bail!("Unsupported partition scheme '{}'. Use 'gpt' or 'mbr'.", other),
        }
    }

    pub fn parted_label(self) -> &'static str {
        match self {
            Self::Gpt => "gpt",
            Self::Mbr => "msdos",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Gpt => "GPT",
            Self::Mbr => "MBR",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileSystem {
    Fat32,
    Ntfs,
    Exfat,
    Ext4,
}

impl FileSystem {
    pub fn parse(s: &str) -> Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "fat32" | "vfat" | "fat" => Ok(Self::Fat32),
            "ntfs" => Ok(Self::Ntfs),
            "exfat" => Ok(Self::Exfat),
            "ext4" => Ok(Self::Ext4),
            other => anyhow::bail!(
                "Unsupported filesystem '{}'. Use 'fat32', 'ntfs', 'exfat', or 'ext4'.",
                other
            ),
        }
    }

    pub fn parted_fs_type(self) -> &'static str {
        match self {
            Self::Fat32 => "fat32",
            Self::Ntfs => "ntfs",
            Self::Exfat => "fat32", // parted uses fat32/ntfs GUID for Microsoft Basic Data
            Self::Ext4 => "ext4",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Fat32 => "FAT32",
            Self::Ntfs => "NTFS",
            Self::Exfat => "exFAT",
            Self::Ext4 => "ext4",
        }
    }

    /// Sanitize volume label to conform to the target filesystem's rules.
    pub fn sanitize_label(self, raw: &str) -> String {
        let max_len = match self {
            Self::Fat32 | Self::Exfat => 11,
            Self::Ext4 => 16,
            Self::Ntfs => 32,
        };

        let cleaned: String = raw
            .trim()
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                    if self == Self::Fat32 {
                        c.to_ascii_uppercase()
                    } else {
                        c
                    }
                } else {
                    '_'
                }
            })
            .take(max_len)
            .collect();

        if cleaned.is_empty() {
            "FLICKER_USB".to_string()
        } else {
            cleaned
        }
    }
}

/// Compute the partition device path for a given disk and 1-based partition number.
///
/// Examples:
/// - `/dev/sdb`, 1 -> `/dev/sdb1`
/// - `/dev/nvme0n1`, 1 -> `/dev/nvme0n1p1`
/// - `/dev/mmcblk0`, 2 -> `/dev/mmcblk0p2`
pub fn partition_path(device: &Path, part_num: u32) -> PathBuf {
    let dev_str = device.to_string_lossy();
    let needs_p = dev_str
        .chars()
        .last()
        .is_some_and(|c| c.is_ascii_digit());
    if needs_p {
        PathBuf::from(format!("{}p{}", dev_str, part_num))
    } else {
        PathBuf::from(format!("{}{}", dev_str, part_num))
    }
}

fn emit(tx: &Option<Sender<ProgressMsg>>, msg: String) {
    println!("{}", msg);
    if let Some(t) = tx {
        let _ = t.send(ProgressMsg::Log(msg));
    }
}

fn emit_status(tx: &Option<Sender<ProgressMsg>>, status: &str, progress: f32) {
    if let Some(t) = tx {
        let _ = t.send(ProgressMsg::Status(status.to_string()));
        let _ = t.send(ProgressMsg::Progress(progress));
    }
}

pub fn run_checked_cmd(program: &str, args: &[&str]) -> Result<()> {
    let output = Command::new(program)
        .args(args)
        .output()
        .with_context(|| format!("Failed to execute '{}'. Is it installed?", program))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        anyhow::bail!(
            "'{} {}' failed: {}{}",
            program,
            args.join(" "),
            stderr.trim(),
            if stderr.trim().is_empty() {
                stdout.trim()
            } else {
                ""
            }
        );
    }
    Ok(())
}

/// Wipe existing partition table and filesystem signatures from a block device.
pub fn wipe_device_signatures(device: &Path) -> Result<()> {
    let dev_str = device.to_string_lossy();
    if Command::new("wipefs").args(["-a", &dev_str]).status().is_ok_and(|s| s.success()) {
        return Ok(());
    }

    // Fallback: zero out the first 1 MiB in pure Rust
    let mut file = OpenOptions::new()
        .write(true)
        .open(device)
        .with_context(|| format!("Failed to open {:?} for wiping", device))?;
    let zeros = vec![0u8; 1024 * 1024];
    file.write_all(&zeros)?;
    file.sync_all()?;
    Ok(())
}

/// Ask the kernel and udev to refresh partition nodes and wait until `expected_part` exists.
pub fn settle_partitions(device: &Path, expected_part: &Path) -> Result<()> {
    let dev_str = device.to_string_lossy();
    let _ = Command::new("partprobe").arg(dev_str.as_ref()).status();
    let _ = Command::new("udevadm")
        .args(["settle", "--timeout=10"])
        .status();

    for _ in 0..20 {
        if expected_part.exists() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(250));
    }

    anyhow::bail!(
        "Timed out waiting for partition device {:?} to appear",
        expected_part
    )
}

/// Format an already-created partition with the requested filesystem and sanitized label.
pub fn format_partition(partition: &Path, fs: FileSystem, label: &str) -> Result<()> {
    let part_str = partition.to_string_lossy();
    let clean_label = fs.sanitize_label(label);

    match fs {
        FileSystem::Fat32 => {
            run_checked_cmd("mkfs.vfat", &["-F", "32", "-n", &clean_label, &part_str])
        }
        FileSystem::Ntfs => {
            run_checked_cmd("mkfs.ntfs", &["-f", "-L", &clean_label, &part_str])
        }
        FileSystem::Exfat => {
            run_checked_cmd("mkfs.exfat", &["-L", &clean_label, &part_str])
        }
        FileSystem::Ext4 => {
            run_checked_cmd("mkfs.ext4", &["-F", "-L", &clean_label, &part_str])
        }
    }
}

/// Partition and format a USB device (Rufus "Non-bootable" / Format mode).
pub fn format_device(
    device: &Path,
    scheme: PartitionScheme,
    fs: FileSystem,
    label: &str,
    tx: Option<Sender<ProgressMsg>>,
) -> Result<()> {
    crate::utils::require_root_privileges()?;
    crate::utils::validate_device_path(device)?;

    let device_name = device
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow::anyhow!("Invalid device path: {:?}", device))?;

    let clean_label = fs.sanitize_label(label);
    emit(
        &tx,
        format!(
            "🧹 Formatting {} ({} / {}, label: '{}')...",
            device.display(),
            scheme.label(),
            fs.label(),
            clean_label
        ),
    );

    emit_status(&tx, "Unmounting existing partitions...", 0.1);
    crate::utils::unmount_device(device_name)?;

    emit_status(&tx, "Wiping old partition signatures...", 0.25);
    wipe_device_signatures(device)?;

    emit_status(&tx, "Creating new partition table...", 0.45);
    let dev_str = device.to_string_lossy();
    run_checked_cmd("parted", &["-s", &dev_str, "mklabel", scheme.parted_label()])?;
    run_checked_cmd(
        "parted",
        &[
            "-s",
            "-a",
            "optimal",
            &dev_str,
            "mkpart",
            "primary",
            fs.parted_fs_type(),
            "1MiB",
            "100%",
        ],
    )?;

    let part1 = partition_path(device, 1);
    emit_status(&tx, "Waiting for kernel partition table update...", 0.6);
    settle_partitions(device, &part1)?;

    emit_status(
        &tx,
        &format!("Formatting {} as {}...", part1.display(), fs.label()),
        0.75,
    );
    format_partition(&part1, fs, &clean_label)?;

    emit_status(&tx, "Syncing device...", 0.95);
    let _ = Command::new("sync").status();

    emit(
        &tx,
        format!(
            "✅ Successfully formatted {} ({} / {}, label: '{}')",
            device.display(),
            scheme.label(),
            fs.label(),
            clean_label
        ),
    );
    emit_status(&tx, "✅ Format completed successfully!", 1.0);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_partition_path_naming() {
        assert_eq!(partition_path(Path::new("/dev/sdb"), 1), PathBuf::from("/dev/sdb1"));
        assert_eq!(
            partition_path(Path::new("/dev/nvme0n1"), 1),
            PathBuf::from("/dev/nvme0n1p1")
        );
        assert_eq!(
            partition_path(Path::new("/dev/mmcblk0"), 2),
            PathBuf::from("/dev/mmcblk0p2")
        );
    }

    #[test]
    fn test_sanitize_label() {
        assert_eq!(FileSystem::Fat32.sanitize_label("ubuntu 24.04 lts"), "UBUNTU_24_0");
        assert_eq!(FileSystem::Ntfs.sanitize_label("Windows 11 Pro"), "Windows_11_Pro");
        assert_eq!(FileSystem::Ext4.sanitize_label(""), "FLICKER_USB");
        assert_eq!(FileSystem::Exfat.sanitize_label("a".repeat(20).as_str()).len(), 11);
    }

    #[test]
    fn test_scheme_and_fs_parsing() {
        assert_eq!(PartitionScheme::parse("GPT").unwrap(), PartitionScheme::Gpt);
        assert_eq!(PartitionScheme::parse("mbr").unwrap(), PartitionScheme::Mbr);
        assert!(PartitionScheme::parse("apm").is_err());

        assert_eq!(FileSystem::parse("fat32").unwrap(), FileSystem::Fat32);
        assert_eq!(FileSystem::parse("NTFS").unwrap(), FileSystem::Ntfs);
        assert_eq!(FileSystem::parse("exfat").unwrap(), FileSystem::Exfat);
        assert_eq!(FileSystem::parse("ext4").unwrap(), FileSystem::Ext4);
        assert!(FileSystem::parse("btrfs").is_err());
    }
}
