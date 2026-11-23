use anyhow::{Context, Result};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;

use crate::format::{
    format_partition, partition_path, run_checked_cmd, settle_partitions, wipe_device_signatures,
    FileSystem, PartitionScheme,
};
use crate::writer::ProgressMsg;

/// Optional Rufus-style Windows 10/11 unattended customizations (`autounattend.xml`).
#[derive(Debug, Clone, Default)]
pub struct WindowsOptions {
    pub bypass_tpm_ram: bool,
    pub bypass_msa: bool,
    pub disable_telemetry: bool,
}

impl WindowsOptions {
    pub fn any_enabled(&self) -> bool {
        self.bypass_tpm_ram || self.bypass_msa || self.disable_telemetry
    }

    /// Generate a Windows `autounattend.xml` matching the selected bypass options.
    pub fn generate_autounattend_xml(&self) -> Option<String> {
        if !self.any_enabled() {
            return None;
        }

        let mut pe_commands = Vec::new();
        let mut order = 1;

        if self.bypass_tpm_ram {
            for key in [
                "BypassTPMCheck",
                "BypassSecureBootCheck",
                "BypassRAMCheck",
                "BypassCPUCheck",
            ] {
                pe_commands.push(format!(
                    r#"        <RunSynchronousCommand wcm:action="add">
          <Order>{order}</Order>
          <Path>reg add HKLM\SYSTEM\Setup\LabConfig /v {key} /t REG_DWORD /d 1 /f</Path>
        </RunSynchronousCommand>"#
                ));
                order += 1;
            }
        }

        let mut oobe_entries = Vec::new();
        if self.disable_telemetry {
            oobe_entries.push("        <ProtectYourPC>3</ProtectYourPC>");
        }
        if self.bypass_msa {
            oobe_entries.push("        <HideOnlineAccountScreens>true</HideOnlineAccountScreens>");
            oobe_entries.push("        <SkipMachineOOBE>false</SkipMachineOOBE>");
        }

        let mut specialize_block = String::new();
        if self.bypass_msa {
            specialize_block = r#"  <settings pass="specialize">
    <component name="Microsoft-Windows-Deployment" processorArchitecture="amd64" publicKeyToken="31bf3856ad364e35" language="neutral" versionScope="nonSxS" xmlns:wcm="http://schemas.microsoft.com/WMIConfig/2002/State">
      <RunSynchronous>
        <RunSynchronousCommand wcm:action="add">
          <Order>1</Order>
          <Path>reg add HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\OOBE /v BypassNRO /t REG_DWORD /d 1 /f</Path>
        </RunSynchronousCommand>
      </RunSynchronous>
    </component>
  </settings>
"#
            .to_string();
        }

        let pe_block = if pe_commands.is_empty() {
            String::new()
        } else {
            format!(
                r#"  <settings pass="windowsPE">
    <component name="Microsoft-Windows-Setup" processorArchitecture="amd64" publicKeyToken="31bf3856ad364e35" language="neutral" versionScope="nonSxS" xmlns:wcm="http://schemas.microsoft.com/WMIConfig/2002/State">
      <RunSynchronous>
{}
      </RunSynchronous>
    </component>
  </settings>
"#,
                pe_commands.join("\n")
            )
        };

        let oobe_block = if oobe_entries.is_empty() {
            String::new()
        } else {
            format!(
                r#"  <settings pass="oobeSystem">
    <component name="Microsoft-Windows-Shell-Setup" processorArchitecture="amd64" publicKeyToken="31bf3856ad364e35" language="neutral" versionScope="nonSxS">
      <OOBE>
{}
      </OOBE>
    </component>
  </settings>
"#,
                oobe_entries.join("\n")
            )
        };

        Some(format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
<unattend xmlns="urn:schemas-microsoft-com:unattend">
{pe_block}{specialize_block}{oobe_block}</unattend>
"#
        ))
    }
}

/// RAII guard that guarantees temporary mount points are unmounted and removed on drop.
struct MountGuard {
    mount_point: PathBuf,
    mounted: bool,
}

impl MountGuard {
    fn mount(device: &Path, label: &str, extra_args: &[&str]) -> Result<Self> {
        let mount_point = std::env::temp_dir().join(format!(
            "flicker-{}-{}",
            label,
            std::process::id()
        ));
        std::fs::create_dir_all(&mount_point)
            .with_context(|| format!("Failed to create mount point {:?}", mount_point))?;

        let mut cmd = Command::new("mount");
        for arg in extra_args {
            cmd.arg(arg);
        }
        cmd.arg(device).arg(&mount_point);

        let output = cmd.output().context("Failed to execute mount")?;
        if !output.status.success() {
            let _ = std::fs::remove_dir(&mount_point);
            let err = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to mount {:?}: {}", device, err.trim());
        }

        Ok(Self {
            mount_point,
            mounted: true,
        })
    }

    fn path(&self) -> &Path {
        &self.mount_point
    }
}

impl Drop for MountGuard {
    fn drop(&mut self) {
        if self.mounted {
            let _ = Command::new("sync").status();
            let _ = Command::new("umount").arg(&self.mount_point).status();
            self.mounted = false;
        }
        let _ = std::fs::remove_dir(&self.mount_point);
    }
}

fn emit_log(tx: &Option<Sender<ProgressMsg>>, msg: String) {
    println!("{}", msg);
    if let Some(t) = tx {
        let _ = t.send(ProgressMsg::Log(msg));
    }
}

fn emit_progress(tx: &Option<Sender<ProgressMsg>>, status: &str, progress: f32) {
    if let Some(t) = tx {
        let _ = t.send(ProgressMsg::Status(status.to_string()));
        let _ = t.send(ProgressMsg::Progress(progress.clamp(0.0, 1.0)));
    }
}

fn is_cancelled(cancel: &Option<Arc<AtomicBool>>) -> bool {
    cancel
        .as_ref()
        .is_some_and(|flag| flag.load(Ordering::SeqCst))
}

fn find_uefi_ntfs_image() -> Option<PathBuf> {
    let candidates = [
        PathBuf::from("/usr/share/flicker/uefi-ntfs.img"),
        PathBuf::from("assets/uefi-ntfs.img"),
    ];
    candidates.into_iter().find(|p| p.is_file())
}

/// Write a Windows (or general extractable) ISO in Rufus "ISO Mode":
/// partitions the USB drive, formats it (NTFS/FAT32), extracts ISO contents,
/// optionally flashes `UEFI:NTFS` boot partition, and writes `autounattend.xml`.
#[allow(clippy::too_many_arguments)]
pub fn write_iso_mode(
    iso_path: &Path,
    device: &Path,
    scheme: PartitionScheme,
    fs: FileSystem,
    label: &str,
    win_opts: &WindowsOptions,
    tx: Option<Sender<ProgressMsg>>,
    cancel: Option<Arc<AtomicBool>>,
) -> Result<()> {
    crate::utils::require_root_privileges()?;
    crate::utils::validate_iso_file(iso_path)?;
    crate::utils::validate_device_path(device)?;

    let device_name = device
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow::anyhow!("Invalid device path: {:?}", device))?;

    emit_log(
        &tx,
        format!(
            "\n🪟 Starting ISO Mode write to {} ({} / {}, label: '{}')...",
            device.display(),
            scheme.label(),
            fs.label(),
            fs.sanitize_label(label)
        ),
    );

    emit_progress(&tx, "Unmounting existing partitions...", 0.05);
    crate::utils::unmount_device(device_name)?;

    if is_cancelled(&cancel) {
        anyhow::bail!("Operation cancelled by user");
    }

    emit_progress(&tx, "Wiping old partition table...", 0.10);
    wipe_device_signatures(device)?;

    emit_progress(&tx, "Creating partition table...", 0.15);
    let dev_str = device.to_string_lossy();
    let uefi_ntfs_img = if fs == FileSystem::Ntfs && scheme == PartitionScheme::Gpt {
        find_uefi_ntfs_image()
    } else {
        None
    };

    run_checked_cmd("parted", &["-s", &dev_str, "mklabel", scheme.parted_label()])?;

    if let Some(ref uefi_img) = uefi_ntfs_img {
        emit_log(
            &tx,
            format!(
                "   Found UEFI:NTFS bootloader ({}) — creating dual-partition GPT layout",
                uefi_img.display()
            ),
        );
        run_checked_cmd(
            "parted",
            &[
                "-s",
                "-a",
                "optimal",
                &dev_str,
                "mkpart",
                "MainData",
                "ntfs",
                "1MiB",
                "-2MiB",
            ],
        )?;
        run_checked_cmd(
            "parted",
            &[
                "-s",
                &dev_str,
                "mkpart",
                "UEFI_NTFS",
                "fat16",
                "-2MiB",
                "100%",
            ],
        )?;
        let _ = run_checked_cmd("parted", &["-s", &dev_str, "set", "2", "msftdata", "on"]);
    } else {
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
        if scheme == PartitionScheme::Mbr {
            let _ = run_checked_cmd("parted", &["-s", &dev_str, "set", "1", "boot", "on"]);
        } else if fs == FileSystem::Fat32 {
            let _ = run_checked_cmd("parted", &["-s", &dev_str, "set", "1", "esp", "on"]);
        }
        if fs == FileSystem::Ntfs && scheme == PartitionScheme::Gpt {
            emit_log(
                &tx,
                "   ℹ️  Note: Optional 'uefi-ntfs.img' not found in /usr/share/flicker/uefi-ntfs.img; modern UEFIs boot NTFS directly.".to_string(),
            );
        }
    }

    let part1 = partition_path(device, 1);
    settle_partitions(device, &part1)?;

    if let Some(ref uefi_img) = uefi_ntfs_img {
        let part2 = partition_path(device, 2);
        settle_partitions(device, &part2)?;
        emit_progress(&tx, "Flashing UEFI:NTFS boot partition...", 0.22);
        let data = std::fs::read(uefi_img)?;
        std::fs::write(&part2, &data)
            .with_context(|| format!("Failed to write UEFI:NTFS image to {:?}", part2))?;
    }

    emit_progress(
        &tx,
        &format!("Formatting {} as {}...", part1.display(), fs.label()),
        0.28,
    );
    format_partition(&part1, fs, label)?;

    if is_cancelled(&cancel) {
        anyhow::bail!("Operation cancelled by user");
    }

    emit_progress(&tx, "Mounting target partition...", 0.35);
    let target_mount = MountGuard::mount(&part1, "win-target", &[])?;

    emit_progress(&tx, "Extracting ISO files to USB (this may take several minutes)...", 0.40);
    extract_iso_to_dir(iso_path, target_mount.path(), &tx, &cancel)?;

    if let Some(xml) = win_opts.generate_autounattend_xml() {
        let xml_path = target_mount.path().join("autounattend.xml");
        std::fs::write(&xml_path, xml)
            .with_context(|| format!("Failed to write {:?}", xml_path))?;
        emit_log(
            &tx,
            "   ✓ Wrote Windows customization file (autounattend.xml)".to_string(),
        );
    }

    emit_progress(&tx, "Syncing and unmounting USB (DO NOT REMOVE)...", 0.95);
    drop(target_mount);

    emit_log(
        &tx,
        format!("🎉 ISO Mode write completed successfully on {}", device.display()),
    );
    emit_progress(&tx, "✅ Write completed successfully!", 1.0);
    Ok(())
}

fn extract_iso_to_dir(
    iso_path: &Path,
    dest_dir: &Path,
    tx: &Option<Sender<ProgressMsg>>,
    cancel: &Option<Arc<AtomicBool>>,
) -> Result<()> {
    // Prefer `7z` because it handles UDF (Windows 10/11 ISOs) reliably without requiring loop mount
    if Command::new("7z").arg("--help").output().is_ok() {
        emit_log(tx, "   Extracting ISO archive via 7z...".to_string());
        let mut child = Command::new("7z")
            .args(["x", "-y", "-bsp1", &format!("-o{}", dest_dir.display())])
            .arg(iso_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Failed to start 7z")?;

        if let Some(stdout) = child.stdout.take() {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                if is_cancelled(cancel) {
                    let _ = child.kill();
                    let _ = child.wait();
                    anyhow::bail!("Operation cancelled by user");
                }
                if let Some(pct_idx) = line.find('%') {
                    let prefix = line[..pct_idx].trim();
                    if let Ok(pct) = prefix.parse::<u32>() {
                        let mapped = 0.40 + (pct.min(100) as f32 / 100.0) * 0.52;
                        emit_progress(tx, &format!("Extracting ISO... {}%", pct.min(100)), mapped);
                    }
                }
            }
        }

        let status = child.wait()?;
        if !status.success() {
            anyhow::bail!("7z extraction failed with exit status {}", status);
        }
        return Ok(());
    }

    // Fallback: loop-mount ISO and copy recursively
    emit_log(
        tx,
        "   7z not found — falling back to loop-mount recursive copy...".to_string(),
    );
    let iso_mount = MountGuard::mount(iso_path, "iso-src", &["-o", "loop,ro"])?;
    copy_dir_recursive(iso_mount.path(), dest_dir, cancel)?;
    Ok(())
}

fn copy_dir_recursive(
    src: &Path,
    dst: &Path,
    cancel: &Option<Arc<AtomicBool>>,
) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        if is_cancelled(cancel) {
            anyhow::bail!("Operation cancelled by user");
        }
        let entry = entry?;
        let ty = entry.file_type()?;
        let dst_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &dst_path, cancel)?;
        } else if ty.is_file() {
            std::fs::copy(entry.path(), &dst_path)?;
        }
    }
    Ok(())
}

/// Create an ext4 persistence partition in the free space remaining after a Linux hybrid ISO write.
///
/// Uses `casper-rw` label (Ubuntu/Mint compatible) and also writes `/ union` to `persistence.conf`
/// (Debian/Kali compatible).
pub fn create_persistence_partition(
    device: &Path,
    iso_size_bytes: u64,
    persistence_mb: u64,
    tx: Option<Sender<ProgressMsg>>,
) -> Result<()> {
    if persistence_mb == 0 {
        return Ok(());
    }

    let start_mb = (iso_size_bytes / (1024 * 1024)) + 4;
    let end_mb = start_mb + persistence_mb;
    let dev_str = device.to_string_lossy();

    emit_log(
        &tx,
        format!(
            "\n💾 Creating {} MB Linux persistence partition on {}...",
            persistence_mb,
            device.display()
        ),
    );
    emit_progress(&tx, "Creating persistence partition...", 0.92);

    run_checked_cmd(
        "parted",
        &[
            "-s",
            "-a",
            "optimal",
            &dev_str,
            "mkpart",
            "primary",
            "ext4",
            &format!("{}MiB", start_mb),
            &format!("{}MiB", end_mb),
        ],
    )?;

    // Determine newly created partition number (typically partition 3 on Ubuntu/Debian hybrid ISOs,
    // or fallback to scanning `/sys/block/<dev>/` for the highest partition number)
    let dev_name = device
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow::anyhow!("Invalid device path"))?;

    let _ = Command::new("partprobe").arg(dev_str.as_ref()).status();
    let _ = Command::new("udevadm")
        .args(["settle", "--timeout=10"])
        .status();

    let part_num = find_highest_partition_number(dev_name).unwrap_or(3);
    let persist_part = partition_path(device, part_num);
    settle_partitions(device, &persist_part)?;

    format_partition(&persist_part, FileSystem::Ext4, "casper-rw")?;

    // Write Debian/Kali `persistence.conf` as well
    if let Ok(guard) = MountGuard::mount(&persist_part, "persist", &[]) {
        let _ = std::fs::write(guard.path().join("persistence.conf"), "/ union\n");
    }

    emit_log(
        &tx,
        format!(
            "✓ Persistence partition created at {} ({} MB, label: casper-rw)",
            persist_part.display(),
            persistence_mb
        ),
    );
    Ok(())
}

fn find_highest_partition_number(device_name: &str) -> Option<u32> {
    let sys_dir = Path::new("/sys/block").join(device_name);
    let mut max_part = None;
    for entry in std::fs::read_dir(sys_dir).ok()?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if let Some(suffix) = name.strip_prefix(device_name) {
            let digits = suffix.strip_prefix('p').unwrap_or(suffix);
            if let Ok(n) = digits.parse::<u32>() {
                max_part = Some(max_part.map_or(n, |cur: u32| cur.max(n)));
            }
        }
    }
    max_part
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_autounattend_none_when_disabled() {
        let opts = WindowsOptions::default();
        assert!(!opts.any_enabled());
        assert!(opts.generate_autounattend_xml().is_none());
    }

    #[test]
    fn test_autounattend_generates_expected_bypass_keys() {
        let opts = WindowsOptions {
            bypass_tpm_ram: true,
            bypass_msa: true,
            disable_telemetry: true,
        };
        let xml = opts.generate_autounattend_xml().unwrap();
        assert!(xml.contains("BypassTPMCheck"));
        assert!(xml.contains("BypassSecureBootCheck"));
        assert!(xml.contains("BypassRAMCheck"));
        assert!(xml.contains("BypassNRO"));
        assert!(xml.contains("<ProtectYourPC>3</ProtectYourPC>"));
    }
}
