use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "flicker", author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// List all available USB devices
    List {
        /// Show detailed information about devices
        #[arg(short, long)]
        verbose: bool,
    },

    /// Write an ISO image to USB devices
    Write {
        /// Path to the ISO file
        #[arg(short, long, value_name = "FILE")]
        iso: PathBuf,

        /// Target device(s) (e.g., /dev/sdb /dev/sdc)
        #[arg(short = 'd', long = "device", alias = "devices", value_name = "DEVICE", num_args = 1..)]
        devices: Vec<PathBuf>,

        /// Skip confirmation prompt
        #[arg(short = 'y', long)]
        yes: bool,

        /// Verify after writing
        #[arg(short, long)]
        verify: bool,

        /// Resume interrupted write
        #[arg(short = 'r', long)]
        resume: bool,

        /// Write mode: auto (detect ISO type), dd (raw copy), or iso (extract/Windows mode)
        #[arg(short = 'm', long, default_value = "auto")]
        mode: String,

        /// Partition scheme for ISO mode (gpt or mbr)
        #[arg(short = 's', long, default_value = "gpt")]
        scheme: String,

        /// Filesystem for ISO mode (ntfs, fat32, exfat, ext4)
        #[arg(short = 'f', long, default_value = "ntfs")]
        fs: String,

        /// Custom volume label for ISO mode (defaults to detected ISO label)
        #[arg(short = 'l', long)]
        label: Option<String>,

        /// Windows 11: inject autounattend.xml to bypass TPM 2.0, Secure Boot, and 4GB RAM checks
        #[arg(long)]
        win_bypass_tpm: bool,

        /// Windows 11: inject autounattend.xml to bypass Microsoft Account requirement (local account)
        #[arg(long)]
        win_bypass_msa: bool,

        /// Linux Live: create an ext4 persistence partition of the given size in MiB after DD write
        #[arg(long, value_name = "MB", default_value_t = 0)]
        persistence: u64,
    },

    /// Verify ISO file checksum
    Verify {
        /// Path to the ISO file
        #[arg(short, long, value_name = "FILE")]
        iso: PathBuf,

        /// Expected checksum (SHA256)
        #[arg(short, long, value_name = "HASH")]
        checksum: Option<String>,

        /// Checksum algorithm to use
        #[arg(short = 'a', long, default_value = "sha256")]
        algorithm: String,
    },

    /// Inspect an ISO/IMG file and display detected image type, volume label, and boot capabilities
    Inspect {
        /// Path to the image file (.iso, .img, .gz, .xz, .zst)
        #[arg(short, long, value_name = "FILE")]
        iso: PathBuf,
    },

    /// Partition and format a USB device (Rufus "Non-bootable" mode)
    Format {
        /// Target device (e.g., /dev/sdb)
        #[arg(short = 'd', long = "device", value_name = "DEVICE")]
        device: PathBuf,

        /// Partition scheme (gpt or mbr)
        #[arg(short = 's', long, default_value = "gpt")]
        scheme: String,

        /// Filesystem (fat32, ntfs, exfat, ext4)
        #[arg(short = 'f', long, default_value = "fat32")]
        fs: String,

        /// Volume label
        #[arg(short = 'l', long, default_value = "FLICKER_USB")]
        label: String,

        /// Skip confirmation prompt
        #[arg(short = 'y', long)]
        yes: bool,
    },

    /// Internal privileged helper invoked by the GUI via pkexec
    #[command(hide = true)]
    InternalWrite {
        #[arg(long, value_name = "FILE")]
        iso: PathBuf,

        #[arg(short = 'd', long = "device", alias = "devices", value_name = "DEVICE", num_args = 1..)]
        devices: Vec<PathBuf>,

        #[arg(long)]
        verify: bool,

        #[arg(long)]
        resume: bool,

        #[arg(long, default_value = "auto")]
        mode: String,

        #[arg(long, default_value = "gpt")]
        scheme: String,

        #[arg(long, default_value = "ntfs")]
        fs: String,

        #[arg(long)]
        label: Option<String>,

        #[arg(long)]
        win_bypass_tpm: bool,

        #[arg(long)]
        win_bypass_msa: bool,

        #[arg(long, default_value_t = 0)]
        persistence: u64,
    },

    /// Internal privileged format helper invoked by the GUI via pkexec
    #[command(hide = true)]
    InternalFormat {
        #[arg(short = 'd', long = "device", value_name = "DEVICE")]
        device: PathBuf,

        #[arg(long, default_value = "gpt")]
        scheme: String,

        #[arg(long, default_value = "fat32")]
        fs: String,

        #[arg(long, default_value = "FLICKER_USB")]
        label: String,
    },
}

impl Cli {
    /// Parse command line arguments
    pub fn parse_args() -> Self {
        Cli::parse()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_accepts_singular_and_plural_device_flags() {
        let cli1 = Cli::try_parse_from([
            "flicker",
            "write",
            "--iso",
            "ubuntu.iso",
            "--device",
            "/dev/sdb",
            "/dev/sdc",
            "--verify",
        ])
        .unwrap();

        match cli1.command.unwrap() {
            Commands::Write {
                iso,
                devices,
                verify,
                ..
            } => {
                assert_eq!(iso, PathBuf::from("ubuntu.iso"));
                assert_eq!(
                    devices,
                    vec![PathBuf::from("/dev/sdb"), PathBuf::from("/dev/sdc")]
                );
                assert!(verify);
            }
            _ => panic!("expected Write command"),
        }

        let cli2 = Cli::try_parse_from([
            "flicker",
            "write",
            "-i",
            "arch.iso",
            "--devices",
            "/dev/sdd",
        ])
        .unwrap();

        match cli2.command.unwrap() {
            Commands::Write { devices, .. } => {
                assert_eq!(devices, vec![PathBuf::from("/dev/sdd")]);
            }
            _ => panic!("expected Write command"),
        }
    }
}