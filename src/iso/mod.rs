use anyhow::{Context, Result};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Compression wrapper detected on the source image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    None,
    Gzip,
    Xz,
    Zstd,
}

impl Compression {
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Gzip => "Gzip (.gz)",
            Self::Xz => "XZ (.xz)",
            Self::Zstd => "Zstandard (.zst)",
        }
    }
}

/// High-level classification of the selected disk image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageKind {
    WindowsIso,
    LinuxHybridIso,
    Iso9660,
    RawDiskImage,
}

impl ImageKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::WindowsIso => "Windows Installer ISO",
            Self::LinuxHybridIso => "Linux Hybrid ISO (isohybrid)",
            Self::Iso9660 => "ISO 9660 Image",
            Self::RawDiskImage => "Raw Disk Image",
        }
    }
}

/// Recommended write strategy inferred from the image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteMode {
    Auto,
    Dd,
    Iso,
}

impl WriteMode {
    pub fn parse(s: &str) -> Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "dd" | "raw" => Ok(Self::Dd),
            "iso" | "windows" => Ok(Self::Iso),
            other => anyhow::bail!("Unsupported write mode '{}'. Use 'auto', 'dd', or 'iso'.", other),
        }
    }
}

/// Metadata extracted by inspecting the image headers without mounting it.
#[derive(Debug, Clone)]
pub struct IsoInfo {
    pub kind: ImageKind,
    pub compression: Compression,
    pub volume_label: String,
    pub is_hybrid: bool,
    pub is_el_torito: bool,
    pub is_udf: bool,
    pub recommended_mode: WriteMode,
    pub summary: String,
}

/// Inspect a disk image or ISO file by reading its magic bytes, MBR, ISO9660 PVD, and UDF headers.
pub fn inspect_image(path: &Path) -> Result<IsoInfo> {
    let mut file = File::open(path).with_context(|| format!("Failed to open {:?}", path))?;
    let file_len = file.metadata()?.len();

    let mut header = [0u8; 512];
    let header_read = file.read(&mut header).unwrap_or(0);

    let compression = detect_compression(&header[..header_read], path);
    if compression != Compression::None {
        let label = default_label_from_path(path);
        let summary = format!(
            "Compressed {} image — will stream-decompress in DD mode",
            compression.label()
        );
        return Ok(IsoInfo {
            kind: ImageKind::RawDiskImage,
            compression,
            volume_label: label,
            is_hybrid: false,
            is_el_torito: false,
            is_udf: false,
            recommended_mode: WriteMode::Dd,
            summary,
        });
    }

    // Check MBR signature (0x55 0xAA at bytes 510..512)
    let has_mbr_sig = header_read >= 512 && header[510] == 0x55 && header[511] == 0xAA;

    // Inspect ISO 9660 Primary Volume Descriptor at sector 16 (offset 0x8000 = 32768)
    let mut has_iso9660 = false;
    let mut is_el_torito = false;
    let mut is_udf = false;
    let mut volume_label = String::new();
    let mut system_id = String::new();
    let mut app_id = String::new();
    let mut has_windows_markers = false;

    if file_len >= 0x8800 {
        let mut pvd = [0u8; 2048];
        if file.seek(SeekFrom::Start(0x8000)).is_ok() && file.read_exact(&mut pvd).is_ok() {
            // Standard ISO 9660 PVD: type == 1, magic == "CD001"
            if pvd[0] == 1 && &pvd[1..6] == b"CD001" {
                has_iso9660 = true;
                system_id = parse_iso_ascii(&pvd[8..40]);
                volume_label = parse_iso_ascii(&pvd[40..72]);
                app_id = parse_iso_ascii(&pvd[574..702]);

                // Root directory record is at bytes 156..190 in PVD
                if let Some(root_names) = read_iso9660_root_entries(&mut file, &pvd[156..190]) {
                    let has_sources = root_names.iter().any(|n| n.eq_ignore_ascii_case("SOURCES"));
                    let has_bootmgr = root_names.iter().any(|n| {
                        let up = n.to_ascii_uppercase();
                        up.starts_with("BOOTMGR") || up == "SETUP.EXE"
                    });
                    if has_sources && has_bootmgr {
                        has_windows_markers = true;
                    }
                }
            }

            // Scan subsequent volume descriptors (sectors 17..32) for El Torito and UDF (BEA01/NSR02/NSR03)
            let mut sector = [0u8; 2048];
            for idx in 17u64..32 {
                let offset = idx * 2048;
                if offset + 2048 > file_len {
                    break;
                }
                if file.seek(SeekFrom::Start(offset)).is_err()
                    || file.read_exact(&mut sector).is_err()
                {
                    break;
                }
                if sector[0] == 0
                    && &sector[1..6] == b"CD001"
                    && &sector[7..30] == b"EL TORITO SPECIFICATION"
                {
                    is_el_torito = true;
                }
                if &sector[1..6] == b"BEA01"
                    || &sector[1..6] == b"NSR02"
                    || &sector[1..6] == b"NSR03"
                {
                    is_udf = true;
                }
            }
        }
    }

    // Windows 10/11 ISOs are frequently UDF-only (no ISO9660 PVD) with UDF anchor/PVD containing
    // volume identifiers like "CCCOMA_X64FRE_..." or "ESD-ISO" or "WIN11".
    if is_udf && !has_windows_markers {
        if let Some(udf_label) = probe_udf_volume_label(&mut file, file_len) {
            if volume_label.is_empty() {
                volume_label = udf_label;
            }
        }
    }

    let label_upper = volume_label.to_ascii_uppercase();
    let app_upper = app_id.to_ascii_uppercase();
    let sys_upper = system_id.to_ascii_uppercase();

    if !has_windows_markers
        && (is_udf || has_iso9660)
        && (label_upper.contains("CCCOMA")
            || label_upper.contains("ESD-ISO")
            || label_upper.contains("WIN10")
            || label_upper.contains("WIN11")
            || label_upper.starts_with("WINDOWS")
            || label_upper.contains("IR5_SSS")
            || label_upper.contains("CPBA_")
            || app_upper.contains("CDIMAGE")
            || app_upper.contains("OSCDIMG")
            || sys_upper.contains("WIN32"))
    {
        has_windows_markers = true;
    }

    if volume_label.is_empty() {
        volume_label = default_label_from_path(path);
    }

    let is_hybrid = has_iso9660 && has_mbr_sig;

    let (kind, recommended_mode, summary) = if has_windows_markers {
        (
            ImageKind::WindowsIso,
            WriteMode::Iso,
            format!(
                "Windows ISO detected ({}) — Recommended: ISO Mode (GPT + NTFS)",
                volume_label
            ),
        )
    } else if is_hybrid {
        (
            ImageKind::LinuxHybridIso,
            WriteMode::Dd,
            format!(
                "Linux Hybrid ISO detected ({}) — Recommended: DD Mode",
                volume_label
            ),
        )
    } else if has_iso9660 || is_udf {
        (
            ImageKind::Iso9660,
            WriteMode::Dd,
            format!("ISO image detected ({}) — DD Mode", volume_label),
        )
    } else {
        (
            ImageKind::RawDiskImage,
            WriteMode::Dd,
            format!("Raw disk image ({}) — DD Mode", volume_label),
        )
    };

    Ok(IsoInfo {
        kind,
        compression,
        volume_label,
        is_hybrid,
        is_el_torito,
        is_udf,
        recommended_mode,
        summary,
    })
}

fn detect_compression(header: &[u8], path: &Path) -> Compression {
    if header.len() >= 2 && header[0] == 0x1F && header[1] == 0x8B {
        return Compression::Gzip;
    }
    if header.len() >= 6 && header[..6] == [0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00] {
        return Compression::Xz;
    }
    if header.len() >= 4 && header[..4] == [0x28, 0xB5, 0x2F, 0xFD] {
        return Compression::Zstd;
    }

    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .as_deref()
    {
        Some("gz") => Compression::Gzip,
        Some("xz") => Compression::Xz,
        Some("zst") => Compression::Zstd,
        _ => Compression::None,
    }
}

fn parse_iso_ascii(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .trim_matches(|c: char| c == '\0' || c.is_whitespace())
        .to_string()
}

fn default_label_from_path(path: &Path) -> String {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("FLICKER_USB");
    let cleaned: String = stem
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_uppercase() } else { '_' })
        .take(16)
        .collect();
    if cleaned.is_empty() {
        "FLICKER_USB".to_string()
    } else {
        cleaned
    }
}

/// Read root directory entry names from an ISO 9660 Primary Volume Descriptor.
fn read_iso9660_root_entries(file: &mut File, root_record: &[u8]) -> Option<Vec<String>> {
    if root_record.len() < 34 {
        return None;
    }
    // Little-endian LBA (bytes 2..6) and data length (bytes 10..14)
    let lba = u32::from_le_bytes(root_record[2..6].try_into().ok()?) as u64;
    let data_len = u32::from_le_bytes(root_record[10..14].try_into().ok()?) as usize;
    if lba == 0 || data_len == 0 || data_len > 65536 {
        return None;
    }

    file.seek(SeekFrom::Start(lba * 2048)).ok()?;
    let mut dir_buf = vec![0u8; data_len];
    file.read_exact(&mut dir_buf).ok()?;

    let mut names = Vec::new();
    let mut pos = 0usize;
    while pos < dir_buf.len() {
        let rec_len = dir_buf[pos] as usize;
        if rec_len == 0 {
            // Advance to next 2048-byte sector boundary
            pos = ((pos / 2048) + 1) * 2048;
            continue;
        }
        if pos + rec_len > dir_buf.len() || rec_len < 34 {
            break;
        }
        let name_len = dir_buf[pos + 32] as usize;
        if pos + 33 + name_len <= dir_buf.len() && name_len > 0 {
            let raw = &dir_buf[pos + 33..pos + 33 + name_len];
            // Skip '.' (0x00) and '..' (0x01)
            if !(name_len == 1 && (raw[0] == 0 || raw[0] == 1)) {
                let name = String::from_utf8_lossy(raw);
                let clean = name.split(';').next().unwrap_or(&name).to_string();
                names.push(clean);
            }
        }
        pos += rec_len;
    }

    Some(names)
}

/// Best-effort scan of UDF Primary Volume Descriptor (typically at sector 32..64) for its volume label.
fn probe_udf_volume_label(file: &mut File, file_len: u64) -> Option<String> {
    let mut sector = [0u8; 2048];
    for idx in 32u64..80 {
        let offset = idx * 2048;
        if offset + 2048 > file_len {
            break;
        }
        if file.seek(SeekFrom::Start(offset)).is_err() || file.read_exact(&mut sector).is_err() {
            break;
        }
        // UDF Descriptor Tag ID == 1 (Primary Volume Descriptor)
        let tag_id = u16::from_le_bytes([sector[0], sector[1]]);
        if tag_id == 1 {
            // Volume Identifier is a 32-byte dstring at offset 24..56:
            // byte 0 = compression ID (8 = 8-bit ASCII, 16 = UTF-16BE), last byte = length
            let dstring = &sector[24..56];
            let comp_id = dstring[0];
            let len = dstring[31] as usize;
            if comp_id == 8 && len > 1 && len <= 31 {
                let s = String::from_utf8_lossy(&dstring[1..len]).trim().to_string();
                if !s.is_empty() {
                    return Some(s);
                }
            } else if comp_id == 16 && len > 2 && len <= 31 {
                let mut u16s = Vec::new();
                let mut i = 1;
                while i + 1 < len {
                    u16s.push(u16::from_be_bytes([dstring[i], dstring[i + 1]]));
                    i += 2;
                }
                let s = String::from_utf16_lossy(&u16s).trim().to_string();
                if !s.is_empty() {
                    return Some(s);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static ISO_TEST_ID: AtomicUsize = AtomicUsize::new(0);

    fn temp_iso_path(name: &str) -> std::path::PathBuf {
        let id = ISO_TEST_ID.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir().join(format!("flicker_iso_{}_{}_{}", std::process::id(), id, name))
    }

    #[test]
    fn test_detect_compression_magic_bytes() {
        let gz = temp_iso_path("image.img.gz");
        std::fs::write(&gz, [0x1F, 0x8B, 0x08, 0x00, 0x00, 0x00]).unwrap();
        let info = inspect_image(&gz).unwrap();
        let _ = std::fs::remove_file(&gz);
        assert_eq!(info.compression, Compression::Gzip);
        assert_eq!(info.kind, ImageKind::RawDiskImage);

        let xz = temp_iso_path("image.img.xz");
        std::fs::write(&xz, [0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00, 0x00]).unwrap();
        let info = inspect_image(&xz).unwrap();
        let _ = std::fs::remove_file(&xz);
        assert_eq!(info.compression, Compression::Xz);

        let zst = temp_iso_path("image.img.zst");
        std::fs::write(&zst, [0x28, 0xB5, 0x2F, 0xFD, 0x00, 0x00]).unwrap();
        let info = inspect_image(&zst).unwrap();
        let _ = std::fs::remove_file(&zst);
        assert_eq!(info.compression, Compression::Zstd);
    }

    #[test]
    fn test_inspect_linux_hybrid_iso() {
        let path = temp_iso_path("ubuntu.iso");
        let mut buf = vec![0u8; 20 * 2048];
        // MBR signature (isohybrid)
        buf[510] = 0x55;
        buf[511] = 0xAA;
        // ISO 9660 PVD at sector 16
        let pvd = &mut buf[16 * 2048..17 * 2048];
        pvd[0] = 1;
        pvd[1..6].copy_from_slice(b"CD001");
        pvd[40..52].copy_from_slice(b"UBUNTU_24_04");
        // El Torito at sector 17
        let et = &mut buf[17 * 2048..18 * 2048];
        et[0] = 0;
        et[1..6].copy_from_slice(b"CD001");
        et[7..30].copy_from_slice(b"EL TORITO SPECIFICATION");

        let mut f = File::create(&path).unwrap();
        f.write_all(&buf).unwrap();
        drop(f);

        let info = inspect_image(&path).unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(info.kind, ImageKind::LinuxHybridIso);
        assert_eq!(info.volume_label, "UBUNTU_24_04");
        assert!(info.is_hybrid);
        assert!(info.is_el_torito);
        assert_eq!(info.recommended_mode, WriteMode::Dd);
    }

    #[test]
    fn test_inspect_windows_iso() {
        let path = temp_iso_path("win11.iso");
        let mut buf = vec![0u8; 40 * 2048];
        // UDF marker at sector 17
        let udf = &mut buf[17 * 2048..18 * 2048];
        udf[1..6].copy_from_slice(b"BEA01");
        // UDF PVD at sector 32 with Windows volume ID
        let pvd = &mut buf[32 * 2048..33 * 2048];
        pvd[0..2].copy_from_slice(&1u16.to_le_bytes());
        let label = b"CCCOMA_X64FRE_EN-US_DV9";
        pvd[24] = 8; // 8-bit ASCII
        pvd[25..25 + label.len()].copy_from_slice(label);
        pvd[55] = (label.len() + 1) as u8;

        std::fs::write(&path, &buf).unwrap();
        let info = inspect_image(&path).unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(info.kind, ImageKind::WindowsIso);
        assert!(info.is_udf);
        assert_eq!(info.volume_label, "CCCOMA_X64FRE_EN-US_DV9");
        assert_eq!(info.recommended_mode, WriteMode::Iso);
        assert_eq!(WriteMode::parse("auto").unwrap(), WriteMode::Auto);
        assert_eq!(WriteMode::parse("dd").unwrap(), WriteMode::Dd);
        assert_eq!(WriteMode::parse("iso").unwrap(), WriteMode::Iso);
    }
}
