use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "flicker")]
#[command(author = "Your Name")]
#[command(version = "0.1.0")]
#[command(about = "A Rufus alternative for Linux written in Rust", long_about = None)]
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

        /// Target devices (e.g., /dev/sdb /dev/sdc)
        #[arg(short, long, value_name = "DEVICE", num_args = 1..)]
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
}

impl Cli {
    /// Parse command line arguments
    pub fn parse_args() -> Self {
        Cli::parse()
    }
}