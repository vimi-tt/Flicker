use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// Buffer size for reading/writing (4MB chunks for optimal performance)
const BUFFER_SIZE: usize = 4 * 1024 * 1024;

pub enum ProgressMsg {
    Log(String),
    Progress(f32),
    Status(String),
}

/// Write ISO file to multiple devices simultaneously with optional resume
pub fn write_iso_to_devices(
    iso_path: &Path,
    device_paths: &[PathBuf],
    verify: bool,
    resume: bool,
    tx: Option<std::sync::mpsc::Sender<ProgressMsg>>,
) -> Result<()> {
    macro_rules! log_msg {
        ($($arg:tt)*) => {
            let msg = format!($($arg)*);
            println!("{}", msg);
            if let Some(t) = &tx {
                let _ = t.send(ProgressMsg::Log(msg));
            }
        }
    }
    
    log_msg!("\n🚀 Starting ISO write process...");
    
    // Open ISO file for reading
    let mut iso_file = File::open(iso_path)
        .context(format!("Failed to open ISO file: {:?}", iso_path))?;
    
    let iso_size = iso_file.metadata()?.len();
    log_msg!("   ISO size: {}", crate::utils::format_size(iso_size));
    
    // Open devices
    let mut device_files = Vec::new();
    for dev_path in device_paths {
        log_msg!("   Opening device: {:?}", dev_path);
        let file = OpenOptions::new()
            .read(resume)
            .write(true)
            .open(dev_path)
            .context(format!("Failed to open device: {:?}", dev_path))?;
        device_files.push(file);
    }
    
    let progress = create_progress_bar(iso_size);
    if device_paths.len() > 1 {
        progress.set_message(format!("📝 Writing ISO to {} devices...", device_paths.len()));
    }
    
    let mut iso_buffer = vec![0u8; BUFFER_SIZE];
    
    // Resume tracking
    let mut resuming = vec![resume; device_paths.len()];
    let mut dev_buffer = vec![0u8; BUFFER_SIZE];
    let mut total_processed = 0u64;
    
    let start_time = std::time::Instant::now();
    let mut last_percent = 0;
    
    loop {
        let bytes_read = iso_file.read(&mut iso_buffer)
            .context("Failed to read from ISO file")?;
        
        if bytes_read == 0 {
            break; // End of file
        }
        
        let chunk = &iso_buffer[..bytes_read];
        
        for (i, dev_file) in device_files.iter_mut().enumerate() {
            if resuming[i] {
                let dev_chunk = &mut dev_buffer[..bytes_read];
                if let Ok(()) = dev_file.read_exact(dev_chunk) {
                    if dev_chunk == chunk {
                        // Match, skip writing
                        continue;
                    }
                }
                // Mismatch or read error: stop resuming, seek to correct position
                resuming[i] = false;
                dev_file.seek(SeekFrom::Start(total_processed))
                    .context(format!("Failed to seek device {:?}", device_paths[i]))?;
            }
            
            dev_file.write_all(chunk)
                .context(format!("Failed to write to device {:?}", device_paths[i]))?;
        }
        
        total_processed += bytes_read as u64;
        progress.set_position(total_processed);
        
        let percent = (total_processed as f64 / iso_size as f64 * 100.0) as u32;
        if percent >= last_percent + 2 || total_processed == iso_size {
            if let Some(t) = &tx {
                let speed_str = calculate_speed(total_processed, start_time.elapsed().as_secs_f64());
                let _ = t.send(ProgressMsg::Log(format!("   ⏳ Writing... {}% ({})", percent, speed_str)));
                let _ = t.send(ProgressMsg::Status(format!("Writing data... {}%", percent)));
            }
            last_percent = percent;
        }
        
        if let Some(t) = &tx {
            let _ = t.send(ProgressMsg::Progress(total_processed as f32 / iso_size as f32));
        }
    }
    
    progress.finish_with_message("✓ Write completed");
    log_msg!("✓ Write completed");
    
    // Sync
    log_msg!("\n🔄 Syncing data to devices (this may take a moment)...");
    if let Some(t) = &tx {
        let _ = t.send(ProgressMsg::Status("Syncing device (DO NOT REMOVE)...".into()));
    }
    for (i, dev_file) in device_files.iter_mut().enumerate() {
        dev_file.sync_all()
            .context(format!("Failed to sync device {:?}", device_paths[i]))?;
    }
    log_msg!("✓ Sync completed");
    
    if verify {
        log_msg!("\n🔍 Verifying written data...");
        for dev_path in device_paths {
            log_msg!("   Verifying {:?}...", dev_path);
            verify_write(iso_path, dev_path, iso_size, tx.clone())?;
        }
        log_msg!("✓ Verification completed successfully");
    }
    
    log_msg!("\n🎉 ISO successfully written to {} device(s)", device_paths.len());
    log_msg!("   You can now safely remove the USB device(s)");
    
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
fn verify_write(iso_path: &Path, device_path: &Path, iso_size: u64, tx: Option<std::sync::mpsc::Sender<ProgressMsg>>) -> Result<()> {
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
    let dev_name = device_path.file_name().unwrap_or_default().to_string_lossy();
    progress.set_message(format!("🔍 Verifying {}", dev_name));
    if let Some(t) = &tx {
        let _ = t.send(ProgressMsg::Status(format!("Verifying {}...", dev_name)));
    }
    
    let mut iso_buffer = vec![0u8; BUFFER_SIZE];
    let mut device_buffer = vec![0u8; BUFFER_SIZE];
    let mut total_verified = 0u64;
    
    let start_time = std::time::Instant::now();
    let mut last_percent = 0;
    
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
            if let Some(t) = &tx {
                let _ = t.send(ProgressMsg::Log(format!("✗ Verification failed for {:?}", device_path)));
            }
            anyhow::bail!(
                "Verification failed for {:?}: data mismatch at offset {}\n\
                 This could indicate:\n\
                 • Faulty USB device\n\
                 • Write error during transfer\n\
                 • Device was disconnected during write\n\n\
                 Recommendation: Try writing again or use a different USB device",
                 device_path, total_verified
            );
        }
        
        total_verified += iso_read as u64;
        progress.set_position(total_verified);
        
        let percent = (total_verified as f64 / iso_size as f64 * 100.0) as u32;
        if percent >= last_percent + 5 || total_verified == iso_size {
            if let Some(t) = &tx {
                let speed_str = calculate_speed(total_verified, start_time.elapsed().as_secs_f64());
                let _ = t.send(ProgressMsg::Log(format!("   🔍 Verifying... {}% ({})", percent, speed_str)));
                let _ = t.send(ProgressMsg::Status(format!("Verifying {}... {}%", dev_name, percent)));
            }
            last_percent = percent;
        }
        
        if let Some(t) = &tx {
            let _ = t.send(ProgressMsg::Progress(total_verified as f32 / iso_size as f32));
        }
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