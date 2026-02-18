use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

/// Buffer size for reading/writing (4MB chunks for optimal performance)
const BUFFER_SIZE: usize = 4 * 1024 * 1024;

/// Write ISO file to a device
pub fn write_iso_to_device(
    iso_path: &Path,
    device_path: &Path,
    verify: bool,
) -> Result<()> {
    println!("\n🚀 Starting ISO write process...");
    
    // Open ISO file for reading
    let mut iso_file = File::open(iso_path)
        .context(format!("Failed to open ISO file: {:?}", iso_path))?;
    
    let iso_size = iso_file.metadata()?.len();
    println!("   ISO size: {}", crate::utils::format_size(iso_size));
    
    // Open device for writing
    println!("   Opening device: {:?}", device_path);
    let mut device_file = File::create(device_path)
        .context(format!("Failed to open device: {:?}", device_path))?;
    
    // Create progress bar
    let progress = create_progress_bar(iso_size);
    
    // Write data in chunks
    let mut buffer = vec![0u8; BUFFER_SIZE];
    let mut total_written = 0u64;
    
    loop {
        // Read chunk from ISO
        let bytes_read = iso_file.read(&mut buffer)
            .context("Failed to read from ISO file")?;
        
        if bytes_read == 0 {
            break; // End of file
        }
        
        // Write chunk to device
        device_file.write_all(&buffer[..bytes_read])
            .context("Failed to write to device")?;
        
        total_written += bytes_read as u64;
        progress.set_position(total_written);
    }
    
    progress.finish_with_message("✓ Write completed");
    
    // Sync to ensure all data is written
    println!("\n🔄 Syncing data to device (this may take a moment)...");
    device_file.sync_all()
        .context("Failed to sync device")?;
    
    println!("✓ Sync completed");
    
    // Verify if requested
    if verify {
        println!("\n🔍 Verifying written data...");
        verify_write(iso_path, device_path, iso_size)?;
        println!("✓ Verification completed successfully");
    }
    
    println!("\n🎉 ISO successfully written to {:?}", device_path);
    println!("   You can now safely remove the USB device");
    
    Ok(())
}

/// Create a styled progress bar
fn create_progress_bar(total_size: u64) -> ProgressBar {
    let progress = ProgressBar::new(total_size);
    
    progress.set_style(
        ProgressStyle::default_bar()
            .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .expect("Failed to create progress bar template")
            .progress_chars("#>-")
    );
    
    progress.set_message("📝 Writing ISO to device...");
    
    progress
}

/// Verify that written data matches the ISO file
fn verify_write(iso_path: &Path, device_path: &Path, iso_size: u64) -> Result<()> {
    let mut iso_file = File::open(iso_path)
        .context("Failed to open ISO file for verification")?;
    
    let mut device_file = File::open(device_path)
        .context("Failed to open device for verification")?;
    
    let progress = ProgressBar::new(iso_size);
    progress.set_style(
        ProgressStyle::default_bar()
            .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.yellow/blue}] {bytes}/{total_bytes} ({eta})")
            .expect("Failed to create progress bar template")
            .progress_chars("#>-")
    );
    progress.set_message("🔍 Verifying data...");
    
    let mut iso_buffer = vec![0u8; BUFFER_SIZE];
    let mut device_buffer = vec![0u8; BUFFER_SIZE];
    let mut total_verified = 0u64;
    
    loop {
        // Read from ISO
        let iso_read = iso_file.read(&mut iso_buffer)
            .context("Failed to read from ISO during verification")?;
        
        if iso_read == 0 {
            break; // Reached end of ISO file - verification complete
        }
        
        // Read same amount from device
        let device_read = device_file.read_exact(&mut device_buffer[..iso_read])
            .context("Failed to read from device during verification")
            .map(|_| iso_read)?;
        
        // Compare the data
        if iso_buffer[..iso_read] != device_buffer[..device_read] {
            progress.finish_with_message("✗ Verification failed");
            anyhow::bail!(
                "Verification failed: data mismatch at offset {}\n\
                 This could indicate:\n\
                 • Faulty USB device\n\
                 • Write error during transfer\n\
                 • Device was disconnected during write\n\n\
                 Recommendation: Try writing again or use a different USB device",
                total_verified
            );
        }
        
        total_verified += iso_read as u64;
        progress.set_position(total_verified);
    }
    
    progress.finish_with_message("✓ Verification completed");
    
    Ok(())
}

/// Calculate write speed
pub fn calculate_speed(bytes: u64, elapsed_secs: f64) -> String {
    if elapsed_secs == 0.0 {
        return "N/A".to_string();
    }
    
    let bytes_per_sec = bytes as f64 / elapsed_secs;
    
    if bytes_per_sec < 1024.0 * 1024.0 {
        format!("{:.2} KB/s", bytes_per_sec / 1024.0)
    } else if bytes_per_sec < 1024.0 * 1024.0 * 1024.0 {
        format!("{:.2} MB/s", bytes_per_sec / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB/s", bytes_per_sec / (1024.0 * 1024.0 * 1024.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_calculate_speed() {
        // 1 MB in 1 second = 1 MB/s
        assert_eq!(calculate_speed(1024 * 1024, 1.0), "1.00 MB/s");
        
        // 100 MB in 10 seconds = 10 MB/s
        assert_eq!(calculate_speed(100 * 1024 * 1024, 10.0), "10.00 MB/s");
    }
}