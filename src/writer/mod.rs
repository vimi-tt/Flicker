use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

/// Buffer size for reading/writing (4MB chunks for optimal performance)
const BUFFER_SIZE: usize = 4 * 1024 * 1024;

/// Flush kernel writeback cache every 16MB so progress reflects real physical writes
const SYNC_INTERVAL: u64 = 16 * 1024 * 1024;

/// Global flag set by SIGINT handler on CLI runs
static SIGINT_CANCELLED: AtomicBool = AtomicBool::new(false);

extern "C" fn handle_sigint(_sig: libc::c_int) {
    SIGINT_CANCELLED.store(true, Ordering::SeqCst);
}

/// Install a POSIX SIGINT handler that marks operations for clean cancellation.
pub fn install_ctrlc_handler() {
    SIGINT_CANCELLED.store(false, Ordering::SeqCst);
    unsafe {
        libc::signal(libc::SIGINT, handle_sigint as *const () as libc::sighandler_t);
    }
}

fn is_cancelled(cancel: Option<&Arc<AtomicBool>>) -> bool {
    if SIGINT_CANCELLED.load(Ordering::SeqCst) {
        return true;
    }
    cancel.is_some_and(|flag| flag.load(Ordering::SeqCst))
}

/// Invalidate the Linux kernel buffer/page cache for a device so verification reads
/// actual bytes from the physical USB medium instead of RAM.
fn invalidate_device_cache(file: &File) {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::io::AsRawFd;
        const BLKFLSBUF: libc::Ioctl = 0x1261;
        let fd = file.as_raw_fd();
        unsafe {
            // Best-effort block device buffer flush (returns ENOTTY on regular files in tests)
            let _ = libc::ioctl(fd, BLKFLSBUF);
            // Also advise kernel to drop cached pages
            let _ = libc::posix_fadvise(fd, 0, 0, libc::POSIX_FADV_DONTNEED);
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = file;
    }
}

pub enum ProgressMsg {
    Log(String),
    Progress(f32),
    Status(String),
}

fn emit_log(tx: &Option<std::sync::mpsc::Sender<ProgressMsg>>, msg: String) {
    println!("{}", msg);
    if let Some(t) = tx {
        let _ = t.send(ProgressMsg::Log(msg));
    }
}

/// Write ISO file to one or more devices in parallel, with optional resume, verification, and cancellation.
pub fn write_iso_to_devices(
    iso_path: &Path,
    device_paths: &[PathBuf],
    verify: bool,
    resume: bool,
    tx: Option<std::sync::mpsc::Sender<ProgressMsg>>,
    cancel: Option<Arc<AtomicBool>>,
) -> Result<()> {
    if device_paths.is_empty() {
        anyhow::bail!("No target devices specified");
    }

    emit_log(&tx, "\n🚀 Starting ISO write process...".to_string());

    let iso_size = std::fs::metadata(iso_path)
        .with_context(|| format!("Failed to open ISO file: {:?}", iso_path))?
        .len();

    if iso_size == 0 {
        anyhow::bail!("❌ ISO file is empty (0 bytes): {:?}", iso_path);
    }

    emit_log(
        &tx,
        format!("   ISO size: {}", crate::utils::format_size(iso_size)),
    );

    let total_target_bytes = iso_size.saturating_mul(device_paths.len() as u64);
    let progress = if tx.is_none() {
        let pb = create_progress_bar(total_target_bytes);
        if device_paths.len() > 1 {
            pb.set_message(format!(
                "📝 Writing ISO to {} devices in parallel...",
                device_paths.len()
            ));
        }
        Some(pb)
    } else {
        None
    };

    let written_counters: Vec<Arc<AtomicU64>> = (0..device_paths.len())
        .map(|_| Arc::new(AtomicU64::new(0)))
        .collect();

    let start_time = std::time::Instant::now();
    let last_reported_percent = Arc::new(AtomicU64::new(0));

    let results: Vec<Result<()>> = std::thread::scope(|s| {
        let mut handles = Vec::with_capacity(device_paths.len());

        for (idx, dev_path) in device_paths.iter().enumerate() {
            let counter = Arc::clone(&written_counters[idx]);
            let all_counters = written_counters.clone();
            let tx_clone = tx.clone();
            let cancel_clone = cancel.clone();
            let progress_ref = &progress;
            let last_pct = Arc::clone(&last_reported_percent);

            handles.push(s.spawn(move || {
                write_single_device(
                    iso_path,
                    iso_size,
                    dev_path,
                    resume,
                    &counter,
                    &all_counters,
                    total_target_bytes,
                    start_time,
                    &last_pct,
                    progress_ref.as_ref(),
                    &tx_clone,
                    cancel_clone.as_ref(),
                )
            }));
        }

        handles
            .into_iter()
            .map(|h| {
                h.join()
                    .unwrap_or_else(|_| Err(anyhow::anyhow!("Writer thread panicked")))
            })
            .collect()
    });

    if let Some(pb) = progress {
        if is_cancelled(cancel.as_ref()) {
            pb.finish_with_message("⚠ Write cancelled");
        } else {
            pb.finish_with_message("✓ Write completed");
        }
    }

    // Collect per-device errors without aborting sibling devices prematurely
    let mut errors = Vec::new();
    for (dev_path, res) in device_paths.iter().zip(results) {
        if let Err(e) = res {
            errors.push(format!("{}: {}", dev_path.display(), e));
        }
    }

    if !errors.is_empty() {
        anyhow::bail!("Write failed:\n  - {}", errors.join("\n  - "));
    }

    emit_log(&tx, "✓ Write and sync completed".to_string());

    if verify {
        emit_log(&tx, "\n🔍 Verifying written data (reading back from device)...".to_string());
        for dev_path in device_paths {
            if is_cancelled(cancel.as_ref()) {
                anyhow::bail!("Operation cancelled by user");
            }
            emit_log(&tx, format!("   Verifying {:?}...", dev_path));
            verify_write(iso_path, dev_path, iso_size, tx.clone(), cancel.clone())?;
        }
        emit_log(&tx, "✓ Verification completed successfully".to_string());
    }

    emit_log(
        &tx,
        format!(
            "\n🎉 ISO successfully written to {} device(s)",
            device_paths.len()
        ),
    );
    emit_log(&tx, "   You can now safely remove the USB device(s)".to_string());

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn write_single_device(
    iso_path: &Path,
    iso_size: u64,
    dev_path: &Path,
    resume: bool,
    my_counter: &AtomicU64,
    all_counters: &[Arc<AtomicU64>],
    total_target_bytes: u64,
    start_time: std::time::Instant,
    last_reported_percent: &AtomicU64,
    progress: Option<&ProgressBar>,
    tx: &Option<std::sync::mpsc::Sender<ProgressMsg>>,
    cancel: Option<&Arc<AtomicBool>>,
) -> Result<()> {
    emit_log(tx, format!("   Opening device: {:?}", dev_path));

    let mut iso_file = File::open(iso_path)
        .with_context(|| format!("Failed to open ISO file: {:?}", iso_path))?;

    let mut dev_file = OpenOptions::new()
        .read(resume)
        .write(true)
        .open(dev_path)
        .with_context(|| format!("Failed to open device: {:?}", dev_path))?;

    let mut iso_buffer = vec![0u8; BUFFER_SIZE];
    let mut dev_buffer = if resume {
        vec![0u8; BUFFER_SIZE]
    } else {
        Vec::new()
    };

    let mut resuming = resume;
    let mut total_processed = 0u64;
    let mut bytes_since_sync = 0u64;

    loop {
        if is_cancelled(cancel) {
            let _ = dev_file.sync_data();
            anyhow::bail!("Operation cancelled by user");
        }

        let bytes_read = iso_file
            .read(&mut iso_buffer)
            .context("Failed to read from ISO file")?;

        if bytes_read == 0 {
            break;
        }

        let chunk = &iso_buffer[..bytes_read];
        let mut wrote_chunk = false;

        if resuming {
            let dev_chunk = &mut dev_buffer[..bytes_read];
            if dev_file.read_exact(dev_chunk).is_ok() && dev_chunk == chunk {
                // Chunk already matches on device; skip writing
            } else {
                resuming = false;
                dev_file
                    .seek(SeekFrom::Start(total_processed))
                    .with_context(|| format!("Failed to seek device {:?}", dev_path))?;
                dev_file
                    .write_all(chunk)
                    .with_context(|| format!("Failed to write to device {:?}", dev_path))?;
                wrote_chunk = true;
            }
        } else {
            dev_file
                .write_all(chunk)
                .with_context(|| format!("Failed to write to device {:?}", dev_path))?;
            wrote_chunk = true;
        }

        total_processed += bytes_read as u64;
        if wrote_chunk {
            bytes_since_sync += bytes_read as u64;
            if bytes_since_sync >= SYNC_INTERVAL {
                dev_file
                    .sync_data()
                    .with_context(|| format!("Failed to sync device {:?}", dev_path))?;
                bytes_since_sync = 0;
            }
        }

        my_counter.store(total_processed, Ordering::Relaxed);
        let aggregate: u64 = all_counters
            .iter()
            .map(|c| c.load(Ordering::Relaxed))
            .sum();

        if let Some(pb) = progress {
            pb.set_position(aggregate);
        }

        let percent = aggregate
            .saturating_mul(100)
            .checked_div(total_target_bytes)
            .unwrap_or(100);

        let prev = last_reported_percent.load(Ordering::Relaxed);
        if (percent >= prev + 2 || aggregate >= total_target_bytes)
            && last_reported_percent
                .compare_exchange(prev, percent, Ordering::SeqCst, Ordering::Relaxed)
                .is_ok()
        {
            if let Some(t) = tx {
                let speed_str = calculate_speed(aggregate, start_time.elapsed().as_secs_f64());
                let _ = t.send(ProgressMsg::Log(format!(
                    "   ⏳ Writing... {}% ({})",
                    percent, speed_str
                )));
                let _ = t.send(ProgressMsg::Status(format!("Writing data... {}%", percent)));
                let ratio = if total_target_bytes > 0 {
                    (aggregate as f32 / total_target_bytes as f32).clamp(0.0, 1.0)
                } else {
                    1.0
                };
                let _ = t.send(ProgressMsg::Progress(ratio));
            }
        }
    }

    emit_log(
        tx,
        format!("🔄 Syncing data to {:?} (DO NOT REMOVE)...", dev_path),
    );
    if let Some(t) = tx {
        let _ = t.send(ProgressMsg::Status(
            "Syncing device (DO NOT REMOVE)...".into(),
        ));
    }
    dev_file
        .sync_all()
        .with_context(|| format!("Failed to sync device {:?}", dev_path))?;

    // Drop kernel page cache immediately after syncing so any subsequent verification reads physical media
    invalidate_device_cache(&dev_file);

    let _ = iso_size;
    Ok(())
}

/// Create a styled progress bar
fn create_progress_bar(total_size: u64) -> ProgressBar {
    let progress = ProgressBar::new(total_size);

    progress.set_style(
        ProgressStyle::default_bar()
            .template(
                "{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})",
            )
            .expect("Failed to create progress bar template")
            .progress_chars("#>-"),
    );

    progress.set_message("📝 Writing ISO to device...");

    progress
}

/// Verify that written data matches the ISO file by reading directly from the device
fn verify_write(
    iso_path: &Path,
    device_path: &Path,
    iso_size: u64,
    tx: Option<std::sync::mpsc::Sender<ProgressMsg>>,
    cancel: Option<Arc<AtomicBool>>,
) -> Result<()> {
    if iso_size == 0 {
        anyhow::bail!("Cannot verify empty ISO (0 bytes)");
    }

    let mut iso_file =
        File::open(iso_path).context("Failed to open ISO file for verification")?;

    let mut device_file =
        File::open(device_path).context("Failed to open device for verification")?;

    // Invalidate OS page/buffer cache so verification reads physical media, not RAM
    invalidate_device_cache(&device_file);

    let dev_name = device_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();

    let progress = if tx.is_none() {
        let pb = ProgressBar::new(iso_size);
        pb.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.yellow/blue}] {bytes}/{total_bytes} ({eta})",
                )
                .expect("Failed to create progress bar template")
                .progress_chars("#>-"),
        );
        pb.set_message(format!("🔍 Verifying {}", dev_name));
        Some(pb)
    } else {
        None
    };

    if let Some(t) = &tx {
        let _ = t.send(ProgressMsg::Status(format!("Verifying {}...", dev_name)));
    }

    let mut iso_buffer = vec![0u8; BUFFER_SIZE];
    let mut device_buffer = vec![0u8; BUFFER_SIZE];
    let mut total_verified = 0u64;

    let start_time = std::time::Instant::now();
    let mut last_percent = 0u64;

    loop {
        if is_cancelled(cancel.as_ref()) {
            if let Some(pb) = progress {
                pb.finish_with_message("⚠ Verification cancelled");
            }
            anyhow::bail!("Operation cancelled by user");
        }

        let iso_read = iso_file
            .read(&mut iso_buffer)
            .context("Failed to read from ISO during verification")?;

        if iso_read == 0 {
            break;
        }

        device_file
            .read_exact(&mut device_buffer[..iso_read])
            .context("Failed to read from device during verification")?;

        if iso_buffer[..iso_read] != device_buffer[..iso_read] {
            if let Some(pb) = progress {
                pb.finish_with_message("✗ Verification failed");
            }
            if let Some(t) = &tx {
                let _ = t.send(ProgressMsg::Log(format!(
                    "✗ Verification failed for {:?}",
                    device_path
                )));
            }
            anyhow::bail!(
                "Verification failed for {:?}: data mismatch at offset {}\n\
                 This could indicate:\n\
                 • Faulty USB device\n\
                 • Write error during transfer\n\
                 • Device was disconnected during write\n\n\
                 Recommendation: Try writing again or use a different USB device",
                device_path,
                total_verified
            );
        }

        total_verified += iso_read as u64;
        if let Some(pb) = &progress {
            pb.set_position(total_verified);
        }

        let percent = total_verified.saturating_mul(100) / iso_size;
        if percent >= last_percent + 5 || total_verified >= iso_size {
            if let Some(t) = &tx {
                let speed_str = calculate_speed(total_verified, start_time.elapsed().as_secs_f64());
                let _ = t.send(ProgressMsg::Log(format!(
                    "   🔍 Verifying... {}% ({})",
                    percent, speed_str
                )));
                let _ = t.send(ProgressMsg::Status(format!(
                    "Verifying {}... {}%",
                    dev_name, percent
                )));
                let _ = t.send(ProgressMsg::Progress(
                    (total_verified as f32 / iso_size as f32).clamp(0.0, 1.0),
                ));
            }
            last_percent = percent;
        }
    }

    if let Some(pb) = progress {
        pb.finish_with_message("✓ Verification completed");
    }

    Ok(())
}

/// Calculate write speed
pub fn calculate_speed(bytes: u64, elapsed_secs: f64) -> String {
    if elapsed_secs <= 0.0 {
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
    use std::sync::atomic::AtomicUsize;

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn temp_test_path(label: &str) -> PathBuf {
        let n = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir().join(format!("flicker_test_{}_{}_{}.bin", std::process::id(), label, n))
    }

    #[test]
    fn test_calculate_speed() {
        // 1 MB in 1 second = 1 MB/s
        assert_eq!(calculate_speed(1024 * 1024, 1.0), "1.00 MB/s");

        // 100 MB in 10 seconds = 10 MB/s
        assert_eq!(calculate_speed(100 * 1024 * 1024, 10.0), "10.00 MB/s");

        // Zero or negative elapsed time
        assert_eq!(calculate_speed(1024, 0.0), "N/A");
    }

    #[test]
    fn test_write_and_verify_parallel_devices() {
        let iso = temp_test_path("iso_parallel");
        let dev1 = temp_test_path("dev1");
        let dev2 = temp_test_path("dev2");

        // 5 MB payload (crosses the 4 MB BUFFER_SIZE boundary)
        let payload: Vec<u8> = (0..(5 * 1024 * 1024)).map(|i| (i % 251) as u8).collect();
        std::fs::write(&iso, &payload).unwrap();
        std::fs::write(&dev1, vec![0u8; payload.len()]).unwrap();
        std::fs::write(&dev2, vec![0u8; payload.len()]).unwrap();

        let res = write_iso_to_devices(
            &iso,
            &[dev1.clone(), dev2.clone()],
            true,
            false,
            None,
            None,
        );

        let data1 = std::fs::read(&dev1).unwrap();
        let data2 = std::fs::read(&dev2).unwrap();

        let _ = std::fs::remove_file(&iso);
        let _ = std::fs::remove_file(&dev1);
        let _ = std::fs::remove_file(&dev2);

        res.unwrap();
        assert_eq!(data1, payload);
        assert_eq!(data2, payload);
    }

    #[test]
    fn test_resume_skips_matching_and_rewrites_divergent() {
        let iso = temp_test_path("iso_resume");
        let dev = temp_test_path("dev_resume");

        let payload: Vec<u8> = (0..(5 * 1024 * 1024)).map(|i| ((i * 7) % 253) as u8).collect();
        let mut partial = payload.clone();
        // Corrupt the second chunk while keeping the first 4MB chunk identical
        for byte in partial.iter_mut().skip(4 * 1024 * 1024) {
            *byte ^= 0xFF;
        }

        std::fs::write(&iso, &payload).unwrap();
        std::fs::write(&dev, &partial).unwrap();

        let res = write_iso_to_devices(&iso, std::slice::from_ref(&dev), true, true, None, None);
        let final_data = std::fs::read(&dev).unwrap();

        let _ = std::fs::remove_file(&iso);
        let _ = std::fs::remove_file(&dev);

        res.unwrap();
        assert_eq!(final_data, payload);
    }

    #[test]
    fn test_write_cancellation() {
        let iso = temp_test_path("iso_cancel");
        let dev = temp_test_path("dev_cancel");

        std::fs::write(&iso, vec![42u8; 1024 * 1024]).unwrap();
        std::fs::write(&dev, vec![0u8; 1024 * 1024]).unwrap();

        let cancel = Arc::new(AtomicBool::new(true));
        let res = write_iso_to_devices(
            &iso,
            std::slice::from_ref(&dev),
            false,
            false,
            None,
            Some(cancel),
        );

        let _ = std::fs::remove_file(&iso);
        let _ = std::fs::remove_file(&dev);

        let err = res.unwrap_err().to_string();
        assert!(err.contains("cancelled"), "unexpected error: {}", err);
    }

    #[test]
    fn test_write_rejects_empty_iso() {
        let iso = temp_test_path("iso_empty");
        let dev = temp_test_path("dev_empty");

        std::fs::write(&iso, b"").unwrap();
        std::fs::write(&dev, b"").unwrap();

        let res = write_iso_to_devices(&iso, std::slice::from_ref(&dev), false, false, None, None);

        let _ = std::fs::remove_file(&iso);
        let _ = std::fs::remove_file(&dev);

        assert!(res.is_err());
    }
}
