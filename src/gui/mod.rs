use anyhow::Result;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{ChildStdin, Command, Stdio};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::checksum::{calculate_checksum_with_progress, ChecksumAlgorithm};
use crate::format::{FileSystem, PartitionScheme};
use crate::iso::{inspect_image, ImageKind, WriteMode};
use crate::usb::UsbDevice;
use crate::windows::WindowsOptions;

slint::include_modules!();

const MAX_LOG_LINES: usize = 300;

struct GuiState {
    devices: Vec<UsbDevice>,
    selected: Vec<bool>,
    iso_path: Option<PathBuf>,
    log_lines: VecDeque<String>,
    cancel_flag: Option<Arc<AtomicBool>>,
    helper_stdin: Option<ChildStdin>,
}

impl GuiState {
    fn new() -> Self {
        Self {
            devices: Vec::new(),
            selected: Vec::new(),
            iso_path: None,
            log_lines: VecDeque::with_capacity(MAX_LOG_LINES),
            cancel_flag: None,
            helper_stdin: None,
        }
    }

    fn push_log(&mut self, line: String) -> SharedString {
        if self.log_lines.len() >= MAX_LOG_LINES {
            self.log_lines.pop_front();
        }
        self.log_lines.push_back(line);
        let mut combined = String::new();
        for (i, l) in self.log_lines.iter().enumerate() {
            if i > 0 {
                combined.push('\n');
            }
            combined.push_str(l);
        }
        combined.into()
    }

    fn clear_logs(&mut self) {
        self.log_lines.clear();
    }

    fn selected_devices(&self) -> Vec<UsbDevice> {
        self.devices
            .iter()
            .zip(self.selected.iter())
            .filter_map(|(d, &sel)| if sel { Some(d.clone()) } else { None })
            .collect()
    }
}

#[derive(Clone)]
struct ActionConfig {
    is_format_only: bool,
    mode_arg: String,
    scheme_arg: String,
    fs_arg: String,
    label: String,
    verify: bool,
    resume: bool,
    win_bypass_tpm: bool,
    win_bypass_msa: bool,
    persistence_mb: u64,
}

fn read_action_config(app: &AppWindow) -> ActionConfig {
    let is_format_only = app.get_boot_selection().as_str() == "Non-bootable (Format only)";
    let mode_arg = match app.get_write_mode().as_str() {
        "DD (Raw)" => "dd",
        "ISO (Windows / Extract)" => "iso",
        _ => "auto",
    }
    .to_string();

    let scheme_arg = app.get_partition_scheme().to_string().to_ascii_lowercase();
    let fs_arg = app.get_file_system().to_string().to_ascii_lowercase();
    let label = app.get_volume_label().to_string();
    let persistence_mb = app
        .get_persistence_mb()
        .trim()
        .parse::<u64>()
        .unwrap_or(0);

    ActionConfig {
        is_format_only,
        mode_arg,
        scheme_arg,
        fs_arg,
        label,
        verify: app.get_opt_verify(),
        resume: app.get_opt_resume(),
        win_bypass_tpm: app.get_win_bypass_tpm(),
        win_bypass_msa: app.get_win_bypass_msa(),
        persistence_mb,
    }
}

fn sync_devices_to_ui(app: &AppWindow, state: &GuiState) {
    let items: Vec<DeviceItem> = state
        .devices
        .iter()
        .zip(state.selected.iter())
        .map(|(d, &sel)| {
            let mounted_tag = if d.is_mounted { " [MOUNTED]" } else { "" };
            DeviceItem {
                label: format!(
                    "/dev/{} - {} ({}){}",
                    d.name,
                    d.model.trim(),
                    crate::utils::format_size(d.size),
                    mounted_tag
                )
                .into(),
                selected: sel,
            }
        })
        .collect();

    let selected_count = state.selected.iter().filter(|&&s| s).count() as i32;
    let model = Rc::new(VecModel::from(items));
    app.set_device_items(ModelRc::from(model));
    app.set_has_devices(!state.devices.is_empty());
    app.set_selected_device_count(selected_count);
}

fn refresh_device_list(app: &AppWindow, state: &Arc<Mutex<GuiState>>) {
    let devices = crate::usb::list_usb_devices().unwrap_or_default();
    if let Ok(mut st) = state.lock() {
        st.devices = devices;
        st.selected = vec![false; st.devices.len()];
        if !st.selected.is_empty() {
            st.selected[0] = true;
        }
        sync_devices_to_ui(app, &st);
    }
}

pub fn run_gui() -> Result<()> {
    let app = AppWindow::new()?;
    let app_weak = app.as_weak();
    let state = Arc::new(Mutex::new(GuiState::new()));

    refresh_device_list(&app, &state);

    // Hotplug USB watcher: auto-refresh device list when drives are plugged/unplugged
    {
        let app_weak_hotplug = app_weak.clone();
        let state_hotplug = Arc::clone(&state);
        crate::usb::spawn_usb_hotplug_watcher(move || {
            let app_u = app_weak_hotplug.clone();
            let st_u = Arc::clone(&state_hotplug);
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(app) = app_u.upgrade() {
                    if !app.get_is_flashing() {
                        refresh_device_list(&app, &st_u);
                    }
                }
            });
        });
    }

    // Refresh devices
    {
        let app_weak = app_weak.clone();
        let state = Arc::clone(&state);
        app.on_refresh_devices(move || {
            if let Some(app) = app_weak.upgrade() {
                refresh_device_list(&app, &state);
            }
        });
    }

    // Toggle device selection
    {
        let app_weak = app_weak.clone();
        let state = Arc::clone(&state);
        app.on_toggle_device(move |idx| {
            if let Some(app) = app_weak.upgrade() {
                if let Ok(mut st) = state.lock() {
                    if let Some(sel) = st.selected.get_mut(idx as usize) {
                        *sel = !*sel;
                    }
                    sync_devices_to_ui(&app, &st);
                }
            }
        });
    }

    // Select ISO/IMG file and auto-inspect it (Rufus-style smart defaults)
    {
        let app_weak = app_weak.clone();
        let state = Arc::clone(&state);
        app.on_select_iso(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Disk Images", &["iso", "img", "gz", "xz", "zst"])
                .pick_file()
            {
                let size_str = std::fs::metadata(&path)
                    .map(|m| format!("Image size: {}", crate::utils::format_size(m.len())))
                    .unwrap_or_default();

                let inspection = inspect_image(&path).ok();

                if let Ok(mut st) = state.lock() {
                    st.iso_path = Some(path.clone());
                }

                if let Some(app) = app_weak.upgrade() {
                    app.set_selected_iso(path.to_string_lossy().to_string().into());
                    app.set_has_valid_iso(true);
                    app.set_iso_size_label(size_str.into());
                    app.set_checksum_result("".into());

                    if let Some(info) = inspection {
                        let is_win = info.kind == ImageKind::WindowsIso;
                        let is_linux = info.kind == ImageKind::LinuxHybridIso;
                        app.set_detected_iso_summary(info.summary.into());
                        app.set_is_windows_iso(is_win);
                        app.set_is_linux_hybrid_iso(is_linux);
                        app.set_volume_label(info.volume_label.into());
                        app.set_write_mode("Auto".into());
                        if is_win {
                            app.set_partition_scheme("GPT".into());
                            app.set_file_system("NTFS".into());
                        } else {
                            app.set_file_system("FAT32".into());
                        }
                    } else {
                        app.set_detected_iso_summary("".into());
                        app.set_is_windows_iso(false);
                        app.set_is_linux_hybrid_iso(false);
                    }
                }
            }
        });
    }

    // Calculate / verify checksum in background
    {
        let app_weak = app_weak.clone();
        let state = Arc::clone(&state);
        app.on_calculate_checksum(move |algo_str, expected| {
            let iso_path = state.lock().ok().and_then(|st| st.iso_path.clone());
            let Some(iso_path) = iso_path else {
                return;
            };

            let algo = match ChecksumAlgorithm::parse(&algo_str) {
                Ok(a) => a,
                Err(e) => {
                    if let Some(app) = app_weak.upgrade() {
                        app.set_checksum_result(format!("❌ {}", e).into());
                    }
                    return;
                }
            };

            if let Some(app) = app_weak.upgrade() {
                app.set_is_hashing(true);
                app.set_checksum_result(format!("Calculating {}...", algo.label()).into());
            }

            let app_bg = app_weak.clone();
            let expected_trimmed = expected.trim().to_string();
            std::thread::spawn(move || {
                let res = calculate_checksum_with_progress(&iso_path, algo, |_, _| {});
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(app) = app_bg.upgrade() {
                        app.set_is_hashing(false);
                        match res {
                            Ok(hash) => {
                                let text = if expected_trimmed.is_empty() {
                                    format!("{}: {}", algo.label(), hash)
                                } else if expected_trimmed.eq_ignore_ascii_case(&hash) {
                                    format!("✅ {} MATCH: {}", algo.label(), hash)
                                } else {
                                    format!("❌ {} MISMATCH! Calculated: {}", algo.label(), hash)
                                };
                                app.set_checksum_result(text.into());
                            }
                            Err(e) => {
                                app.set_checksum_result(format!("❌ Error: {}", e).into());
                            }
                        }
                    }
                });
            });
        });
    }

    // Request Action -> pre-flight checks & open confirmation dialog
    {
        let app_weak = app_weak.clone();
        let state = Arc::clone(&state);
        app.on_request_action(move || {
            let Some(app) = app_weak.upgrade() else {
                return;
            };

            let cfg = read_action_config(&app);
            let (iso_path, targets) = match state.lock() {
                Ok(st) => (st.iso_path.clone(), st.selected_devices()),
                Err(_) => return,
            };

            if targets.is_empty() {
                return;
            }

            let target_lines: Vec<String> = targets
                .iter()
                .map(|d| {
                    format!(
                        "• /dev/{} — {} ({})",
                        d.name,
                        d.model.trim(),
                        crate::utils::format_size(d.size)
                    )
                })
                .collect();

            if cfg.is_format_only {
                let summary = format!(
                    "ALL DATA on the following device(s) will be permanently erased and formatted:\n\n{}\n\nScheme: {}  |  Filesystem: {}  |  Label: {}",
                    target_lines.join("\n"),
                    cfg.scheme_arg.to_uppercase(),
                    cfg.fs_arg.to_uppercase(),
                    cfg.label
                );
                app.set_confirm_summary(summary.into());
                app.set_show_confirm_dialog(true);
                return;
            }

            let Some(iso_path) = iso_path else {
                return;
            };

            if let Err(e) = crate::utils::validate_iso_file(&iso_path) {
                app.set_show_progress(true);
                app.set_status_text(format!("❌ {}", e).into());
                return;
            }

            let iso_size = match std::fs::metadata(&iso_path) {
                Ok(m) => m.len(),
                Err(e) => {
                    app.set_show_progress(true);
                    app.set_status_text(format!("❌ Failed to read image metadata: {}", e).into());
                    return;
                }
            };

            for dev in &targets {
                if iso_size > dev.size {
                    app.set_show_progress(true);
                    app.set_status_text(
                        format!(
                            "❌ Image ({}) exceeds /dev/{} ({})",
                            crate::utils::format_size(iso_size),
                            dev.name,
                            crate::utils::format_size(dev.size)
                        )
                        .into(),
                    );
                    return;
                }
            }

            let summary = format!(
                "ALL DATA on the following device(s) will be permanently erased:\n\n{}\n\nImage: {} ({})\nMode: {}",
                target_lines.join("\n"),
                iso_path.file_name().unwrap_or_default().to_string_lossy(),
                crate::utils::format_size(iso_size),
                app.get_write_mode()
            );

            app.set_confirm_summary(summary.into());
            app.set_show_confirm_dialog(true);
        });
    }

    // Confirm Action -> run directly if root, or via privileged pkexec helper
    {
        let app_weak = app_weak.clone();
        let state = Arc::clone(&state);
        app.on_confirm_action(move || {
            let Some(app) = app_weak.upgrade() else {
                return;
            };

            let cfg = read_action_config(&app);
            let (iso_path, targets) = match state.lock() {
                Ok(mut st) => {
                    st.clear_logs();
                    (st.iso_path.clone(), st.selected_devices())
                }
                Err(_) => return,
            };

            if targets.is_empty() {
                return;
            }
            if !cfg.is_format_only && iso_path.is_none() {
                return;
            }

            let device_paths: Vec<PathBuf> = targets.iter().map(|d| d.path.clone()).collect();

            app.set_is_flashing(true);
            app.set_show_progress(true);
            app.set_status_text(
                if cfg.is_format_only {
                    "Starting format..."
                } else {
                    "Starting write process..."
                }
                .into(),
            );
            app.set_progress_value(0.0);
            app.set_terminal_logs("".into());

            let cancel_flag = Arc::new(AtomicBool::new(false));
            if let Ok(mut st) = state.lock() {
                st.cancel_flag = Some(Arc::clone(&cancel_flag));
                st.helper_stdin = None;
            }

            let app_bg = app_weak.clone();
            let state_bg = Arc::clone(&state);

            std::thread::spawn(move || {
                let res = if cfg.is_format_only {
                    if crate::utils::check_root_privileges() {
                        run_direct_format(&device_paths, &cfg, &app_bg, &state_bg)
                    } else {
                        run_pkexec_helper_format(&device_paths, &cfg, &app_bg, &state_bg)
                    }
                } else {
                    let iso_p = iso_path.unwrap();
                    if crate::utils::check_root_privileges() {
                        run_direct_write(
                            &iso_p,
                            &device_paths,
                            &cfg,
                            &app_bg,
                            &state_bg,
                            cancel_flag,
                        )
                    } else {
                        run_pkexec_helper_write(&iso_p, &device_paths, &cfg, &app_bg, &state_bg)
                    }
                };

                if let Ok(mut st) = state_bg.lock() {
                    st.cancel_flag = None;
                    st.helper_stdin = None;
                }

                let state_ui = Arc::clone(&state_bg);
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(app) = app_bg.upgrade() {
                        app.set_is_flashing(false);
                        match res {
                            Ok(()) => {
                                app.set_progress_value(1.0);
                                app.set_status_text(
                                    if cfg.is_format_only {
                                        "✅ Format completed successfully!"
                                    } else {
                                        "✅ Write completed successfully!"
                                    }
                                    .into(),
                                );
                            }
                            Err(e) => {
                                app.set_status_text(format!("❌ {}", e).into());
                                if let Ok(mut st) = state_ui.lock() {
                                    let logs = st.push_log(format!("ERROR: {}", e));
                                    app.set_terminal_logs(logs);
                                }
                            }
                        }
                        refresh_device_list(&app, &state_ui);
                    }
                });
            });
        });
    }

    // Cancel Action
    {
        let app_weak = app_weak.clone();
        let state = Arc::clone(&state);
        app.on_cancel_action(move || {
            if let Ok(mut st) = state.lock() {
                if let Some(flag) = &st.cancel_flag {
                    flag.store(true, Ordering::SeqCst);
                }
                if let Some(mut stdin) = st.helper_stdin.take() {
                    let _ = stdin.write_all(b"CANCEL\n");
                    let _ = stdin.flush();
                }
            }
            if let Some(app) = app_weak.upgrade() {
                app.set_status_text("Cancelling operation safely...".into());
            }
        });
    }

    // Reset progress panel
    {
        let app_weak = app_weak.clone();
        let state = Arc::clone(&state);
        app.on_reset_progress(move || {
            if let Some(app) = app_weak.upgrade() {
                app.set_show_progress(false);
                app.set_progress_value(0.0);
                app.set_status_text("Ready".into());
                refresh_device_list(&app, &state);
            }
        });
    }

    app.run()?;
    Ok(())
}

fn push_gui_log(app_weak: &slint::Weak<AppWindow>, state: &Arc<Mutex<GuiState>>, line: String) {
    let logs = match state.lock() {
        Ok(mut st) => st.push_log(line),
        Err(_) => return,
    };
    let app_weak = app_weak.clone();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(app) = app_weak.upgrade() {
            app.set_terminal_logs(logs);
        }
    });
}

fn spawn_progress_bridge(
    rx: std::sync::mpsc::Receiver<crate::writer::ProgressMsg>,
    app_bg: &slint::Weak<AppWindow>,
    state_bg: &Arc<Mutex<GuiState>>,
) -> std::thread::JoinHandle<()> {
    let app_rx = app_bg.clone();
    let state_rx = Arc::clone(state_bg);
    std::thread::spawn(move || {
        while let Ok(msg) = rx.recv() {
            match msg {
                crate::writer::ProgressMsg::Progress(p) => {
                    let app_u = app_rx.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(app) = app_u.upgrade() {
                            app.set_progress_value(p);
                        }
                    });
                }
                crate::writer::ProgressMsg::Status(s) => {
                    let app_u = app_rx.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(app) = app_u.upgrade() {
                            app.set_status_text(s.into());
                        }
                    });
                }
                crate::writer::ProgressMsg::Log(l) => {
                    push_gui_log(&app_rx, &state_rx, l);
                }
            }
        }
    })
}

fn run_direct_format(
    device_paths: &[PathBuf],
    cfg: &ActionConfig,
    app_bg: &slint::Weak<AppWindow>,
    state_bg: &Arc<Mutex<GuiState>>,
) -> Result<()> {
    let scheme = PartitionScheme::parse(&cfg.scheme_arg)?;
    let fs = FileSystem::parse(&cfg.fs_arg)?;
    let (tx, rx) = std::sync::mpsc::channel();
    let rx_thread = spawn_progress_bridge(rx, app_bg, state_bg);

    let mut res = Ok(());
    for dev in device_paths {
        if let Err(e) = crate::format::format_device(dev, scheme, fs, &cfg.label, Some(tx.clone()))
        {
            res = Err(e);
            break;
        }
    }
    drop(tx);
    let _ = rx_thread.join();
    res
}

fn run_direct_write(
    iso_path: &std::path::Path,
    device_paths: &[PathBuf],
    cfg: &ActionConfig,
    app_bg: &slint::Weak<AppWindow>,
    state_bg: &Arc<Mutex<GuiState>>,
    cancel_flag: Arc<AtomicBool>,
) -> Result<()> {
    crate::utils::validate_iso_file(iso_path)?;
    let iso_info = inspect_image(iso_path)?;
    let effective_mode = match WriteMode::parse(&cfg.mode_arg)? {
        WriteMode::Auto => iso_info.recommended_mode,
        other => other,
    };

    for dev in device_paths {
        crate::utils::validate_device_path(dev)?;
        let dev_name = dev
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid device path: {:?}", dev))?;
        crate::utils::unmount_device(dev_name)?;
    }

    let (tx, rx) = std::sync::mpsc::channel();
    let rx_thread = spawn_progress_bridge(rx, app_bg, state_bg);

    let res = match effective_mode {
        WriteMode::Iso => {
            let scheme = PartitionScheme::parse(&cfg.scheme_arg)?;
            let fs = FileSystem::parse(&cfg.fs_arg)?;
            let win_opts = WindowsOptions {
                bypass_tpm_ram: cfg.win_bypass_tpm,
                bypass_msa: cfg.win_bypass_msa,
                disable_telemetry: cfg.win_bypass_tpm || cfg.win_bypass_msa,
            };
            let label = if cfg.label.trim().is_empty() {
                iso_info.volume_label.as_str()
            } else {
                cfg.label.as_str()
            };
            let mut last = Ok(());
            for dev in device_paths {
                if let Err(e) = crate::windows::write_iso_mode(
                    iso_path,
                    dev,
                    scheme,
                    fs,
                    label,
                    &win_opts,
                    Some(tx.clone()),
                    Some(Arc::clone(&cancel_flag)),
                ) {
                    last = Err(e);
                    break;
                }
            }
            drop(tx);
            last
        }
        WriteMode::Dd | WriteMode::Auto => {
            let iso_size = std::fs::metadata(iso_path)?.len();
            let w_res = crate::writer::write_iso_to_devices(
                iso_path,
                device_paths,
                cfg.verify,
                cfg.resume,
                Some(tx.clone()),
                Some(Arc::clone(&cancel_flag)),
            );
            if w_res.is_ok() && cfg.persistence_mb > 0 {
                for dev in device_paths {
                    crate::windows::create_persistence_partition(
                        dev,
                        iso_size,
                        cfg.persistence_mb,
                        Some(tx.clone()),
                    )?;
                }
            }
            drop(tx);
            w_res
        }
    };

    let _ = rx_thread.join();
    res
}

fn stream_pkexec_command(
    mut cmd: Command,
    app_bg: &slint::Weak<AppWindow>,
    state_bg: &Arc<Mutex<GuiState>>,
) -> Result<()> {
    push_gui_log(
        app_bg,
        state_bg,
        "🔐 Requesting administrator privileges via pkexec...".to_string(),
    );
    {
        let app_u = app_bg.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(app) = app_u.upgrade() {
                app.set_status_text("Waiting for authentication (pkexec)...".into());
            }
        });
    }

    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| {
        anyhow::anyhow!(
            "Failed to launch pkexec helper ({}). Ensure polkit/pkexec is installed or run CLI with sudo.",
            e
        )
    })?;

    if let Some(stdin) = child.stdin.take() {
        if let Ok(mut st) = state_bg.lock() {
            st.helper_stdin = Some(stdin);
        }
    }

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("Failed to capture helper stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow::anyhow!("Failed to capture helper stderr"))?;

    let stderr_handle = std::thread::spawn(move || {
        let mut err_text = String::new();
        let mut reader = BufReader::new(stderr);
        let _ = std::io::Read::read_to_string(&mut reader, &mut err_text);
        err_text
    });

    let reader = BufReader::new(stdout);
    for line in reader.lines().map_while(Result::ok) {
        if let Some(val) = line.strip_prefix("FLICKER_PROGRESS:") {
            if let Ok(p) = val.trim().parse::<f32>() {
                let app_u = app_bg.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(app) = app_u.upgrade() {
                        app.set_progress_value(p);
                    }
                });
            }
        } else if let Some(status) = line.strip_prefix("FLICKER_STATUS:") {
            let status_owned = status.to_string();
            let app_u = app_bg.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(app) = app_u.upgrade() {
                    app.set_status_text(status_owned.into());
                }
            });
        } else if let Some(log) = line.strip_prefix("FLICKER_LOG:") {
            push_gui_log(app_bg, state_bg, log.to_string());
        } else if !line.trim().is_empty() {
            push_gui_log(app_bg, state_bg, line);
        }
    }

    let status = child.wait()?;
    let stderr_output = stderr_handle.join().unwrap_or_default();

    if status.success() {
        Ok(())
    } else if status.code() == Some(126) || status.code() == Some(127) {
        anyhow::bail!("Authentication cancelled or pkexec denied.");
    } else {
        let detail = stderr_output.trim();
        if detail.is_empty() {
            anyhow::bail!("Privileged helper exited with status {}", status);
        } else {
            anyhow::bail!("{}", detail);
        }
    }
}

fn run_pkexec_helper_format(
    device_paths: &[PathBuf],
    cfg: &ActionConfig,
    app_bg: &slint::Weak<AppWindow>,
    state_bg: &Arc<Mutex<GuiState>>,
) -> Result<()> {
    let exe_path = std::env::current_exe()?;
    for dev in device_paths {
        let mut cmd = Command::new("pkexec");
        cmd.arg(&exe_path)
            .arg("internal-format")
            .arg("--device")
            .arg(dev)
            .arg("--scheme")
            .arg(&cfg.scheme_arg)
            .arg("--fs")
            .arg(&cfg.fs_arg)
            .arg("--label")
            .arg(&cfg.label);
        stream_pkexec_command(cmd, app_bg, state_bg)?;
    }
    Ok(())
}

fn run_pkexec_helper_write(
    iso_path: &std::path::Path,
    device_paths: &[PathBuf],
    cfg: &ActionConfig,
    app_bg: &slint::Weak<AppWindow>,
    state_bg: &Arc<Mutex<GuiState>>,
) -> Result<()> {
    let exe_path = std::env::current_exe()?;
    let mut cmd = Command::new("pkexec");
    cmd.arg(&exe_path)
        .arg("internal-write")
        .arg("--iso")
        .arg(iso_path)
        .arg("--device");

    for dev in device_paths {
        cmd.arg(dev);
    }
    cmd.arg("--mode")
        .arg(&cfg.mode_arg)
        .arg("--scheme")
        .arg(&cfg.scheme_arg)
        .arg("--fs")
        .arg(&cfg.fs_arg);

    if !cfg.label.trim().is_empty() {
        cmd.arg("--label").arg(&cfg.label);
    }
    if cfg.verify {
        cmd.arg("--verify");
    }
    if cfg.resume {
        cmd.arg("--resume");
    }
    if cfg.win_bypass_tpm {
        cmd.arg("--win-bypass-tpm");
    }
    if cfg.win_bypass_msa {
        cmd.arg("--win-bypass-msa");
    }
    if cfg.persistence_mb > 0 {
        cmd.arg("--persistence").arg(cfg.persistence_mb.to_string());
    }

    stream_pkexec_command(cmd, app_bg, state_bg)
}
