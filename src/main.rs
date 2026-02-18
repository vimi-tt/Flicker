mod cli;
mod usb;
mod utils;
mod writer;

use cli::{Cli, Commands};
use anyhow::Result;

fn main() -> Result<()> {
    // Parse command line arguments
    let cli = Cli::parse_args();

    // Execute the appropriate command
    match cli.command {
        Commands::List { verbose } => {
            println!("🔍 Listing USB devices...");
            if verbose {
                println!("(Verbose mode enabled)");
            }
            list_devices(verbose)?;
        }

        Commands::Write {
            iso,
            device,
            yes,
            verify,
        } => {
            println!("📝 Writing ISO to device...");
            println!("  ISO:    {:?}", iso);
            println!("  Device: {:?}", device);
            println!("  Auto-confirm: {}", yes);
            println!("  Verify: {}", verify);
            
            write_iso(&iso, &device, yes, verify)?;
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
            
            verify_iso(&iso, checksum.as_deref(), &algorithm)?;
        }
    }

    Ok(())
}

// List USB devices
fn list_devices(verbose: bool) -> Result<()> {
    let devices = usb::list_usb_devices()?;
    usb::display_devices(&devices, verbose);
    Ok(())
}

fn write_iso(
    iso: &std::path::Path,
    device: &std::path::Path,
    skip_confirm: bool,
    verify: bool,
) -> Result<()> {
    use std::io::{self, Write};

    // 1. Check root permissions
    println!("\n🔐 Checking permissions...");
    utils::require_root_privileges()?;
    println!("✓ Running with root privileges");

    // 2. Validate ISO file
    println!("\n📋 Validating ISO file...");
    utils::validate_iso_file(iso)?;
    
    let iso_metadata = std::fs::metadata(iso)?;
    let iso_size = iso_metadata.len();
    println!("✓ ISO file valid: {} ({})", iso.display(), utils::format_size(iso_size));

    // 3. Validate device
    println!("\n💾 Validating device...");
    utils::validate_device_path(device)?;
    println!("✓ Device valid: {}", device.display());

    // 4. Check if device is really a USB
    let device_name = device.file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow::anyhow!("Invalid device name"))?;
    
    let usb_devices = usb::list_usb_devices()?;
    let is_usb = usb_devices.iter().any(|d| d.name == device_name);
    
    if !is_usb {
        println!("\n⚠️  WARNING: {} does not appear to be a removable USB device!", device.display());
        println!("   This might be an internal disk!");
        
        if !skip_confirm {
            print!("\n❓ Are you ABSOLUTELY SURE you want to continue? (type 'YES' in capitals): ");
            io::stdout().flush()?;
            
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            
            if input.trim() != "YES" {
                println!("❌ Operation cancelled for safety.");
                return Ok(());
            }
        }
    }

    // 5. Check device size
    if let Some(usb_dev) = usb_devices.iter().find(|d| d.name == device_name) {
        if iso_size > usb_dev.size {
            anyhow::bail!(
                "❌ ISO file ({}) is larger than device ({})!\n   ISO: {}\n   Device: {}",
                utils::format_size(iso_size),
                utils::format_size(usb_dev.size),
                iso_size,
                usb_dev.size
            );
        }
        
        println!("\n📊 Space check:");
        println!("   ISO size:    {}", utils::format_size(iso_size));
        println!("   Device size: {}", utils::format_size(usb_dev.size));
        println!("   ✓ Sufficient space available");
        
        // 6. Unmount if necessary
        if usb_dev.is_mounted {
            println!("\n🔓 Device is mounted, unmounting...");
            utils::unmount_device(device_name)?;
            println!("✓ Device unmounted successfully");
        }
    }

    // 7. Final confirmation
    if !skip_confirm {
        println!("\n{}", "=".repeat(60));
        println!("⚠️  FINAL WARNING ⚠️");
        println!("{}", "=".repeat(60));
        println!();
        println!("This will COMPLETELY ERASE all data on:");
        println!("   Device: {}", device.display());
        println!("   Size:   {}", utils::format_size(usb_devices.iter()
            .find(|d| d.name == device_name)
            .map(|d| d.size)
            .unwrap_or(0)));
        println!();
        println!("And write:");
        println!("   ISO:    {}", iso.display());
        println!("   Size:   {}", utils::format_size(iso_size));
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
    
    writer::write_iso_to_device(iso, device, verify)?;
    
    let elapsed = start_time.elapsed();
    let speed = writer::calculate_speed(iso_size, elapsed.as_secs_f64());
    
    println!("\n{}", "=".repeat(60));
    println!("✅ SUCCESS!");
    println!("{}", "=".repeat(60));
    println!("   Total time: {:.2}s", elapsed.as_secs_f64());
    println!("   Average speed: {}", speed);
    println!("   Device: {}", device.display());
    println!("{}", "=".repeat(60));
    
    Ok(())
}

fn verify_iso(iso: &std::path::Path, expected: Option<&str>, algorithm: &str) -> Result<()> {
    if !iso.exists() {
        anyhow::bail!("❌ ISO file not found: {:?}", iso);
    }

    println!("\n🔐 Calculating {} checksum...", algorithm.to_uppercase());
    println!("   This is a placeholder - actual implementation coming next!");
    
    if let Some(hash) = expected {
        println!("\n   Expected:  {}", hash);
        println!("   Calculated: <will be computed>");
        println!("   Status: <will check match>");
    } else {
        println!("\n   Calculated checksum: <will be computed>");
    }
    
    Ok(())
}