mod checksum;
mod cli;
mod format;
#[cfg(feature = "gui")]
mod gui;
mod iso;
mod usb;
mod utils;
mod writer;

use anyhow::Result;
use cli::{Cli, Commands};
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

fn main() -> Result<()> {
    let cli = Cli::parse_args();

    if let Some(cmd) = cli.command {
        match cmd {
            Commands::List { verbose } => {
                println!("🔍 Listing USB devices...");
                if verbose {
                    println!("(Verbose mode enabled)");
                }
                list_devices(verbose)?;
            }

            Commands::Write {
                iso,
                devices,
                yes,
                verify,
                resume,
            } => {
                println!("📝 Writing ISO to device(s)...");
                println!("  ISO:     {:?}", iso);
                println!("  Devices: {:?}", devices);
                println!("  Auto-confirm: {}", yes);
                println!("  Verify:  {}", verify);
                println!("  Resume:  {}", resume);

                write_iso(&iso, &devices, yes, verify, resume)?;
            }

            Commands::Verify {
                iso,
                checksum,
                algorithm,
            } => {
                println!("✓ Verifying ISO checksum...");
                println!("  ISO:       {:?}", iso);
                println!("  Algorithm: {}", algorithm);
                if let Some(ref hash) = checksum {
                    println!("  Expected:  {}", hash);
                }

                checksum::verify_iso_cli(&iso, checksum.as_deref(), &algorithm)?;
            }

            Commands::Inspect { iso } => {
                let info = iso::inspect_image(&iso)?;
                println!("🔎 Image Inspection: {}", iso.display());
                println!("   Type:             {}", info.kind.label());
                println!("   Volume Label:     {}", info.volume_label);
                println!("   Compression:      {}", info.compression.label());
                println!("   Hybrid (MBR+ISO): {}", if info.is_hybrid { "Yes" } else { "No" });
                println!("   El Torito Boot:   {}", if info.is_el_torito { "Yes" } else { "No" });
                println!("   UDF Filesystem:   {}", if info.is_udf { "Yes" } else { "No" });
                println!("   Summary:          {}", info.summary);
                println!("   Recommended Mode: {:?}", info.recommended_mode);
            }

            Commands::Format {
                device,
                scheme,
                fs,
                label,
                yes,
            } => {
                format_usb_cli(&device, &scheme, &fs, &label, yes)?;
            }

            Commands::InternalWrite {
                iso,
                devices,
                verify,
                resume,
            } => {
                run_internal_write(&iso, &devices, verify, resume)?;
            }

            Commands::InternalFormat {
                device,
                scheme,
                fs,
                label,
            } => {
                run_internal_format(&device, &scheme, &fs, &label)?;
            }
        }
    } else {
        #[cfg(feature = "gui")]
        {
            gui::run_gui()?;
        }
        #[cfg(not(feature = "gui"))]
        {
            anyhow::bail!(
                "GUI support is disabled in this build. Run 'flicker --help' for CLI usage or rebuild with '--features gui'."
            );
        }
    }

    Ok(())
}

/// Privileged helper entry point invoked by the unprivileged GUI via `pkexec`.
/// Streams structured progress lines to stdout and monitors stdin for cancellation.
fn run_internal_write(
    iso: &std::path::Path,
    devices: &[std::path::PathBuf],
    verify: bool,
    resume: bool,
) -> Result<()> {
    utils::require_root_privileges()?;
    utils::validate_iso_file(iso)?;

    let iso_size = std::fs::metadata(iso)?.len();

    for device in devices {
        utils::validate_device_path(device)?;
        let device_name = device
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid device name for {}", device.display()))?;

        if let Ok(dev_size) = utils::get_block_device_size(device_name) {
            if iso_size > dev_size {
                anyhow::bail!(
                    "ISO ({}) is larger than {} ({})",
                    utils::format_size(iso_size),
                    device.display(),
                    utils::format_size(dev_size)
                );
            }
        }

        utils::unmount_device(device_name)?;
    }

    let cancel_flag = Arc::new(AtomicBool::new(false));
    let cancel_watch = Arc::clone(&cancel_flag);
    std::thread::spawn(move || {
        use std::io::BufRead;
        let stdin = std::io::stdin();
        for line in stdin.lock().lines().map_while(Result::ok) {
            if line.trim().eq_ignore_ascii_case("CANCEL") {
                cancel_watch.store(true, Ordering::SeqCst);
                break;
            }
        }
    });

    let (tx, rx) = std::sync::mpsc::channel();
    let emitter = std::thread::spawn(move || {
        let mut stdout = std::io::stdout().lock();
        while let Ok(msg) = rx.recv() {
            match msg {
                writer::ProgressMsg::Progress(p) => {
                    let _ = writeln!(stdout, "FLICKER_PROGRESS:{:.4}", p);
                }
                writer::ProgressMsg::Status(s) => {
                    let _ = writeln!(stdout, "FLICKER_STATUS:{}", s.replace('\n', " "));
                }
                writer::ProgressMsg::Log(l) => {
                    for part in l.lines() {
                        let _ = writeln!(stdout, "FLICKER_LOG:{}", part);
                    }
                }
            }
            let _ = stdout.flush();
        }
    });

    let res = writer::write_iso_to_devices(
        iso,
        devices,
        verify,
        resume,
        Some(tx),
        Some(cancel_flag),
    );
    let _ = emitter.join();
    res
}

fn run_internal_format(
    device: &std::path::Path,
    scheme_str: &str,
    fs_str: &str,
    label: &str,
) -> Result<()> {
    let scheme = format::PartitionScheme::parse(scheme_str)?;
    let fs = format::FileSystem::parse(fs_str)?;

    let (tx, rx) = std::sync::mpsc::channel();
    let emitter = std::thread::spawn(move || {
        let mut stdout = std::io::stdout().lock();
        while let Ok(msg) = rx.recv() {
            match msg {
                writer::ProgressMsg::Progress(p) => {
                    let _ = writeln!(stdout, "FLICKER_PROGRESS:{:.4}", p);
                }
                writer::ProgressMsg::Status(s) => {
                    let _ = writeln!(stdout, "FLICKER_STATUS:{}", s.replace('\n', " "));
                }
                writer::ProgressMsg::Log(l) => {
                    for part in l.lines() {
                        let _ = writeln!(stdout, "FLICKER_LOG:{}", part);
                    }
                }
            }
            let _ = stdout.flush();
        }
    });

    let res = format::format_device(device, scheme, fs, label, Some(tx));
    let _ = emitter.join();
    res
}

fn format_usb_cli(
    device: &std::path::Path,
    scheme_str: &str,
    fs_str: &str,
    label: &str,
    skip_confirm: bool,
) -> Result<()> {
    use std::io;

    utils::require_root_privileges()?;
    utils::validate_device_path(device)?;
    let scheme = format::PartitionScheme::parse(scheme_str)?;
    let fs = format::FileSystem::parse(fs_str)?;

    if !skip_confirm {
        println!("\n⚠️  FINAL WARNING: This will COMPLETELY ERASE {}", device.display());
        println!("   Scheme:     {}", scheme.label());
        println!("   Filesystem: {}", fs.label());
        println!("   Label:      {}", fs.sanitize_label(label));
        print!("\n❓ Type 'yes' to confirm formatting: ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if input.trim().to_lowercase() != "yes" {
            println!("❌ Operation cancelled.");
            return Ok(());
        }
    }

    format::format_device(device, scheme, fs, label, None)
}

// List USB devices
fn list_devices(verbose: bool) -> Result<()> {
    let devices = usb::list_usb_devices()?;
    usb::display_devices(&devices, verbose);
    Ok(())
}

fn write_iso(
    iso: &std::path::Path,
    devices: &[std::path::PathBuf],
    skip_confirm: bool,
    verify: bool,
    resume: bool,
) -> Result<()> {
    use std::io;

    // 1. Check root permissions
    println!("\n🔐 Checking permissions...");
    utils::require_root_privileges()?;
    println!("✓ Running with root privileges");

    // 2. Validate ISO file
    println!("\n📋 Validating ISO file...");
    utils::validate_iso_file(iso)?;

    let iso_metadata = std::fs::metadata(iso)?;
    let iso_size = iso_metadata.len();
    println!(
        "✓ ISO file valid: {} ({})",
        iso.display(),
        utils::format_size(iso_size)
    );

    let usb_devices = usb::list_usb_devices()?;

    for device in devices {
        // 3. Validate device
        println!("\n💾 Validating device: {}...", device.display());
        utils::validate_device_path(device)?;
        println!("✓ Device valid: {}", device.display());

        // 4. Check if device is really a USB
        let device_name = device
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid device name for {}", device.display()))?;

        let is_usb = usb_devices.iter().any(|d| d.name == device_name);

        if !is_usb {
            println!(
                "\n⚠️  WARNING: {} does not appear to be a removable USB device!",
                device.display()
            );
            println!("   This might be an internal disk!");

            if !skip_confirm {
                print!(
                    "\n❓ Are you ABSOLUTELY SURE you want to continue with {}? (type 'YES' in capitals): ",
                    device.display()
                );
                io::stdout().flush()?;

                let mut input = String::new();
                io::stdin().read_line(&mut input)?;

                if input.trim() != "YES" {
                    println!("❌ Operation cancelled for safety.");
                    return Ok(());
                }
            }
        }

        // 5. Check device size (works for both detected USBs and confirmed non-USB block devices)
        let dev_size = usb_devices
            .iter()
            .find(|d| d.name == device_name)
            .map(|d| d.size)
            .or_else(|| utils::get_block_device_size(device_name).ok());

        if let Some(size) = dev_size {
            if iso_size > size {
                anyhow::bail!(
                    "❌ ISO file ({}) is larger than device {} ({})!\n   ISO: {}\n   Device: {}",
                    utils::format_size(iso_size),
                    device.display(),
                    utils::format_size(size),
                    iso_size,
                    size
                );
            }

            println!("📊 Space check for {}:", device.display());
            println!("   ISO size:    {}", utils::format_size(iso_size));
            println!("   Device size: {}", utils::format_size(size));
            println!("   ✓ Sufficient space available");
        }

        // 6. Unmount any mounted partitions on the target device
        utils::unmount_device(device_name)?;
    }

    // 7. Final confirmation
    if !skip_confirm {
        println!("\n{}", "=".repeat(60));
        println!("⚠️  FINAL WARNING ⚠️");
        println!("{}", "=".repeat(60));
        println!();
        println!("This will COMPLETELY ERASE all data on the following devices:");
        for device in devices {
            let device_name = device
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");
            let size = usb_devices
                .iter()
                .find(|d| d.name == device_name)
                .map(|d| d.size)
                .or_else(|| utils::get_block_device_size(device_name).ok())
                .unwrap_or(0);
            println!("   - {} ({})", device.display(), utils::format_size(size));
        }
        println!();
        println!("And write:");
        println!("   ISO:    {}", iso.display());
        println!("   Size:   {}", utils::format_size(iso_size));
        if resume {
            println!("\n   [Resume Mode] Will skip writing data that already matches.");
        }
        println!();
        println!("{}", "=".repeat(60));

        print!("\n❓ Type 'yes' to confirm and start writing: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if input.trim().to_lowercase() != "yes" {
            println!("\n❌ Operation cancelled.");
            return Ok(());
        }
    }

    // 8. Start write process
    println!("\n{}", "=".repeat(60));
    println!("🔥 STARTING WRITE PROCESS");
    println!("{}", "=".repeat(60));

    let start_time = std::time::Instant::now();
    writer::install_ctrlc_handler();

    writer::write_iso_to_devices(iso, devices, verify, resume, None, None)?;

    let elapsed = start_time.elapsed();
    let speed = writer::calculate_speed(iso_size * devices.len() as u64, elapsed.as_secs_f64());

    println!("\n{}", "=".repeat(60));
    println!("✅ SUCCESS!");
    println!("{}", "=".repeat(60));
    println!("   Total time: {:.2}s", elapsed.as_secs_f64());
    println!("   Overall speed: {}", speed);
    println!("   Devices written: {}", devices.len());
    println!("{}", "=".repeat(60));

    Ok(())
}
