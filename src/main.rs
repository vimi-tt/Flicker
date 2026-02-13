mod cli;
mod usb;
mod utils;

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

    // Check if ISO file exists
    if !iso.exists() {
        anyhow::bail!("❌ ISO file not found: {:?}", iso);
    }

    // Warning message
    println!("\n⚠️  WARNING ⚠️");
    println!("This will completely erase all data on {:?}", device);
    
    if !skip_confirm {
        print!("\n❓ Are you sure you want to continue? (type 'yes' to confirm): ");
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        if input.trim().to_lowercase() != "yes" {
            println!("❌ Operation cancelled.");
            return Ok(());
        }
    }

    println!("\n🚀 Starting write process...");
    println!("   This is a placeholder - actual implementation coming next!");
    
    if verify {
        println!("\n✓ Verification will be performed after writing");
    }
    
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