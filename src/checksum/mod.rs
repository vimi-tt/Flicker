use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use md5::Md5;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::Path;

const HASH_BUFFER_SIZE: usize = 4 * 1024 * 1024;

/// Supported checksum algorithms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChecksumAlgorithm {
    Sha256,
    Md5,
}

impl ChecksumAlgorithm {
    pub fn parse(name: &str) -> Result<Self> {
        match name.trim().to_lowercase().as_str() {
            "sha256" | "sha-256" => Ok(Self::Sha256),
            "md5" => Ok(Self::Md5),
            other => anyhow::bail!(
                "❌ Unsupported algorithm: {}. Please use 'sha256' or 'md5'.",
                other
            ),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Sha256 => "SHA256",
            Self::Md5 => "MD5",
        }
    }
}

fn hash_file<D: Digest>(
    file: &mut File,
    mut on_progress: impl FnMut(u64),
) -> Result<String> {
    let mut hasher = D::new();
    let mut buffer = vec![0u8; HASH_BUFFER_SIZE];
    let mut total_read = 0u64;

    loop {
        let bytes_read = file.read(&mut buffer).context("Failed to read ISO file")?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
        total_read += bytes_read as u64;
        on_progress(total_read);
    }

    Ok(hex::encode(hasher.finalize()))
}

/// Calculate the checksum of a file, invoking `on_progress(bytes_read, total_bytes)`.
pub fn calculate_checksum_with_progress(
    iso: &Path,
    algorithm: ChecksumAlgorithm,
    mut on_progress: impl FnMut(u64, u64),
) -> Result<String> {
    if !iso.exists() {
        anyhow::bail!("❌ ISO file not found: {:?}", iso);
    }

    let mut file = File::open(iso).with_context(|| format!("Failed to open {:?}", iso))?;
    let total_size = file.metadata()?.len();

    match algorithm {
        ChecksumAlgorithm::Sha256 => hash_file::<Sha256>(&mut file, |n| on_progress(n, total_size)),
        ChecksumAlgorithm::Md5 => hash_file::<Md5>(&mut file, |n| on_progress(n, total_size)),
    }
}

/// Verify or print an ISO checksum from the CLI with a progress bar.
pub fn verify_iso_cli(iso: &Path, expected: Option<&str>, algorithm_str: &str) -> Result<()> {
    let algorithm = ChecksumAlgorithm::parse(algorithm_str)?;

    if !iso.exists() {
        anyhow::bail!("❌ ISO file not found: {:?}", iso);
    }

    let iso_size = std::fs::metadata(iso)?.len();
    println!("\n🔐 Calculating {} checksum...", algorithm.label());

    let progress = ProgressBar::new(iso_size);
    progress.set_style(
        ProgressStyle::default_bar()
            .template(
                "{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})",
            )
            .expect("Failed to create progress bar template")
            .progress_chars("#>-"),
    );
    progress.set_message(format!("Calculating {}...", algorithm.label()));

    let calculated = calculate_checksum_with_progress(iso, algorithm, |read, _| {
        progress.set_position(read);
    })?;

    progress.finish_with_message("✓ Checksum calculated");

    if let Some(hash) = expected {
        println!("\n   Expected:   {}", hash);
        println!("   Calculated: {}", calculated);
        if hash.trim().eq_ignore_ascii_case(&calculated) {
            println!("   Status:     ✅ MATCH");
        } else {
            println!("   Status:     ❌ MISMATCH");
            anyhow::bail!("Checksum verification failed!");
        }
    } else {
        println!("\n   Calculated checksum: {}", calculated);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checksum_sha256_and_md5() {
        let tmp = std::env::temp_dir().join(format!("flicker_hash_{}.bin", std::process::id()));
        std::fs::write(&tmp, b"hello world").unwrap();

        let sha = calculate_checksum_with_progress(&tmp, ChecksumAlgorithm::Sha256, |_, _| {}).unwrap();
        let md5 = calculate_checksum_with_progress(&tmp, ChecksumAlgorithm::Md5, |_, _| {}).unwrap();

        let _ = std::fs::remove_file(&tmp);

        assert_eq!(
            sha,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
        assert_eq!(md5, "5eb63bbbe01eeed093cb22bb8f5acdc3");
    }

    #[test]
    fn test_checksum_algorithm_parsing() {
        assert_eq!(ChecksumAlgorithm::parse("SHA256").unwrap(), ChecksumAlgorithm::Sha256);
        assert_eq!(ChecksumAlgorithm::parse("sha-256").unwrap(), ChecksumAlgorithm::Sha256);
        assert_eq!(ChecksumAlgorithm::parse("md5").unwrap(), ChecksumAlgorithm::Md5);
        assert!(ChecksumAlgorithm::parse("sha1").is_err());
    }
}
