mod cli;
mod usb;
mod utils;
mod writer;

use cli::{Cli, Commands};
use anyhow::Result;

slint::include_modules!();

fn main() -> Result<()> {
    // Parse command line arguments
    let cli = Cli::parse_args();

    // Execute the appropriate command or run GUI
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
                
                verify_iso(&iso, checksum.as_deref(), &algorithm)?;
            }
        }
    } else {
        run_gui()?;
    }

    Ok(())
}

fn require_root_or_pkexec() {
    if unsafe { libc::geteuid() } != 0 {
        println!("Elevating privileges via pkexec...");
        
        let exe_path = match std::env::current_exe() {
            Ok(path) => path,
            Err(_) => {
                eprintln!("Failed to get current executable path.");
                std::process::exit(1);
            }
        };
        
        let mut args: Vec<String> = std::env::args().collect();
        if !args.is_empty() {
            args[0] = exe_path.to_string_lossy().into_owned();
        } else {
            args.push(exe_path.to_string_lossy().into_owned());
        }

        let display = std::env::var("DISPLAY").unwrap_or_default();
        let xauthority = std::env::var("XAUTHORITY").unwrap_or_default();
        let wayland_display = std::env::var("WAYLAND_DISPLAY").unwrap_or_default();
        let xdg_runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_default();
        let dbus_session = std::env::var("DBUS_SESSION_BUS_ADDRESS").unwrap_or_default();
        
        let mut cmd = std::process::Command::new("pkexec");
        cmd.arg("env");
        if !display.is_empty() {
            cmd.arg(format!("DISPLAY={}", display));
        }
        if !xauthority.is_empty() {
            cmd.arg(format!("XAUTHORITY={}", xauthority));
        } else if let Ok(home) = std::env::var("HOME") {
            // Default XAUTHORITY location
            cmd.arg(format!("XAUTHORITY={}/.Xauthority", home));
        }
        if !wayland_display.is_empty() {
            cmd.arg(format!("WAYLAND_DISPLAY={}", wayland_display));
        }
        if !xdg_runtime_dir.is_empty() {
            cmd.arg(format!("XDG_RUNTIME_DIR={}", xdg_runtime_dir));
        }
        if !dbus_session.is_empty() {
            cmd.arg(format!("DBUS_SESSION_BUS_ADDRESS={}", dbus_session));
        }
        
        cmd.args(args);
        
        let status = cmd.status();
        
        if let Ok(st) = status {
            std::process::exit(st.code().unwrap_or(1));
        } else {
            eprintln!("Failed to elevate privileges. Please run as root.");
            std::process::exit(1);
        }
    }
}

fn run_gui() -> Result<()> {
    require_root_or_pkexec();

    let app = AppWindow::new()?;
    let app_weak = app.as_weak();

    update_devices(&app);

    let app_weak_refresh = app_weak.clone();
    app.on_refresh_devices(move || {
        if let Some(app) = app_weak_refresh.upgrade() {
            update_devices(&app);
        }
    });

    let app_weak_iso = app_weak.clone();
    app.on_select_iso(move || {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("ISO Image", &["iso"])
            .pick_file() {
            if let Some(app) = app_weak_iso.upgrade() {
                app.set_selected_iso(path.to_string_lossy().to_string().into());
            }
        }
    });

    let app_weak_flash = app_weak.clone();
    app.on_flash(move |verify, resume| {
        let app = app_weak_flash.unwrap();
        let iso = app.get_selected_iso().to_string();
        let dev_str = app.get_selected_device().to_string();
        
        if iso == "No ISO selected" || dev_str.is_empty() || dev_str == "Loading devices..." { return; }
        
        let device_name = dev_str.split(" -").next().unwrap_or(&dev_str).replace("/dev/", "");
        let device_path = std::path::PathBuf::from(format!("/dev/{}", device_name));
        let iso_path = std::path::PathBuf::from(&iso);
        
        app.set_is_flashing(true);
        app.set_status_text("Starting write process...".into());
        app.set_progress_value(0.0);
        app.set_terminal_logs("".into());
        
        let (tx, rx) = std::sync::mpsc::channel();
        
        // Receiver thread for GUI updates
        let app_rx = app_weak_flash.clone();
        std::thread::spawn(move || {
            while let Ok(msg) = rx.recv() {
                let app_update = app_rx.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(app) = app_update.upgrade() {
                        match msg {
                            crate::writer::ProgressMsg::Progress(p) => app.set_progress_value(p),
                            crate::writer::ProgressMsg::Status(s) => app.set_status_text(s.into()),
                            crate::writer::ProgressMsg::Log(l) => {
                                let mut logs = app.get_terminal_logs().to_string();
                                logs.push_str(&l);
                                logs.push('\n');
                                app.set_terminal_logs(logs.into());
                            }
                        }
                    }
                });
            }
        });

        let app_bg = app_weak_flash.clone();
        std::thread::spawn(move || {
            let _ = crate::utils::unmount_device(&device_name);

            let res = crate::writer::write_iso_to_devices(&iso_path, &[device_path], verify, resume, Some(tx));
            
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(app) = app_bg.upgrade() {
                    app.set_is_flashing(false);
                    match res {
                        Ok(_) => {
                            app.set_progress_value(1.0);
                            app.set_status_text("✅ Write completed successfully!".into());
                        }
                        Err(e) => {
                            app.set_status_text(format!("❌ Error: {}", e).into());
                            let mut logs = app.get_terminal_logs().to_string();
                            logs.push_str(&format!("\nERROR: {}\n", e));
                            app.set_terminal_logs(logs.into());
                        }
                    }
                }
            });
        });
    });

    app.run()?;
    Ok(())
}

fn update_devices(app: &AppWindow) {
    let mut slint_devices = Vec::new();
    if let Ok(devices) = crate::usb::list_usb_devices() {
        for d in devices {
            slint_devices.push(format!("/dev/{} - {} ({})", d.name, d.model, crate::utils::format_size(d.size)).into());
        }
    }
    if slint_devices.is_empty() {
        slint_devices.push("No USB devices found".into());
    }
    
    use slint::Model;
    let model = std::rc::Rc::new(slint::VecModel::from(slint_devices));
    app.set_device_list(model.clone().into());
    if model.row_count() > 0 {
        app.set_selected_device(model.row_data(0).unwrap());
    }
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

    let usb_devices = usb::list_usb_devices()?;

    for device in devices {
        // 3. Validate device
        println!("\n💾 Validating device: {}...", device.display());
        utils::validate_device_path(device)?;
        println!("✓ Device valid: {}", device.display());

        // 4. Check if device is really a USB
        let device_name = device.file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid device name for {}", device.display()))?;
        
        let is_usb = usb_devices.iter().any(|d| d.name == device_name);
        
        if !is_usb {
            println!("\n⚠️  WARNING: {} does not appear to be a removable USB device!", device.display());
            println!("   This might be an internal disk!");
            
            if !skip_confirm {
                print!("\n❓ Are you ABSOLUTELY SURE you want to continue with {}? (type 'YES' in capitals): ", device.display());
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
                    "❌ ISO file ({}) is larger than device {} ({})!\n   ISO: {}\n   Device: {}",
                    utils::format_size(iso_size),
                    device.display(),
                    utils::format_size(usb_dev.size),
                    iso_size,
                    usb_dev.size
                );
            }
            
            println!("📊 Space check for {}:", device.display());
            println!("   ISO size:    {}", utils::format_size(iso_size));
            println!("   Device size: {}", utils::format_size(usb_dev.size));
            println!("   ✓ Sufficient space available");
            
            // 6. Unmount if necessary
            if usb_dev.is_mounted {
                println!("🔓 Device {} is mounted, unmounting...", device.display());
                utils::unmount_device(device_name)?;
                println!("✓ Device unmounted successfully");
            }
        }
    }

    // 7. Final confirmation
    if !skip_confirm {
        println!("\n{}", "=".repeat(60));
        println!("⚠️  FINAL WARNING ⚠️");
        println!("{}", "=".repeat(60));
        println!();
        println!("This will COMPLETELY ERASE all data on the following devices:");
        for device in devices {
            let device_name = device.file_name().unwrap().to_str().unwrap();
            let size = usb_devices.iter()
                .find(|d| d.name == device_name)
                .map(|d| d.size)
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
    
    writer::write_iso_to_devices(iso, devices, verify, resume, None)?;
    
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

fn verify_iso(iso: &std::path::Path, expected: Option<&str>, algorithm: &str) -> Result<()> {
    use std::fs::File;
    use std::io::Read;
    use sha2::{Sha256, Digest};
    use md5::Md5;
    use indicatif::{ProgressBar, ProgressStyle};

    if !iso.exists() {
        anyhow::bail!("❌ ISO file not found: {:?}", iso);
    }

    println!("\n🔐 Calculating {} checksum...", algorithm.to_uppercase());
    
    let mut file = File::open(iso)?;
    let iso_size = file.metadata()?.len();
    
    let progress = ProgressBar::new(iso_size);
    progress.set_style(
        ProgressStyle::default_bar()
            .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .expect("Failed to create progress bar template")
            .progress_chars("#>-")
    );
    progress.set_message(format!("Calculating {}...", algorithm.to_uppercase()));

    let mut buffer = vec![0u8; 4 * 1024 * 1024]; // 4MB buffer
    let mut total_read = 0u64;

    let calculated = match algorithm.to_lowercase().as_str() {
        "sha256" => {
            let mut hasher = Sha256::new();
            loop {
                let bytes_read = file.read(&mut buffer)?;
                if bytes_read == 0 {
                    break;
                }
                hasher.update(&buffer[..bytes_read]);
                total_read += bytes_read as u64;
                progress.set_position(total_read);
            }
            hex::encode(hasher.finalize())
        },
        "md5" => {
            let mut hasher = Md5::new();
            loop {
                let bytes_read = file.read(&mut buffer)?;
                if bytes_read == 0 {
                    break;
                }
                hasher.update(&buffer[..bytes_read]);
                total_read += bytes_read as u64;
                progress.set_position(total_read);
            }
            hex::encode(hasher.finalize())
        },
        _ => {
            progress.finish_and_clear();
            anyhow::bail!("❌ Unsupported algorithm: {}. Please use 'sha256' or 'md5'.", algorithm);
        },
    };

    progress.finish_with_message("✓ Checksum calculated");

    if let Some(hash) = expected {
        println!("\n   Expected:   {}", hash);
        println!("   Calculated: {}", calculated);
        if hash.eq_ignore_ascii_case(&calculated) {
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