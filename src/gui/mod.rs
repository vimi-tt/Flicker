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
use crate::usb::UsbDevice;

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

    // Select ISO or IMG file
    {
        let app_weak = app_weak.clone();
        let state = Arc::clone(&state);
        app.on_select_iso(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Disk Images", &["iso", "img"])
                .pick_file()
            {
                let size_str = std::fs::metadata(&path)
                    .map(|m| format!("Image size: {}", crate::utils::format_size(m.len())))
                    .unwrap_or_default();

                if let Ok(mut st) = state.lock() {
                    st.iso_path = Some(path.clone());
                }

                if let Some(app) = app_weak.upgrade() {
                    app.set_selected_iso(path.to_string_lossy().to_string().into());
                    app.set_has_valid_iso(true);
                    app.set_iso_size_label(size_str.into());
                    app.set_checksum_result("".into());
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

    // Request Flash -> validate inputs & open confirmation dialog
    {
        let app_weak = app_weak.clone();
        let state = Arc::clone(&state);
        app.on_request_flash(move || {
            let Some(app) = app_weak.upgrade() else {
                return;
            };

            let (iso_path, targets) = match state.lock() {
                Ok(st) => (st.iso_path.clone(), st.selected_devices()),
                Err(_) => return,
            };

            let Some(iso_path) = iso_path else {
                return;
            };
            if targets.is_empty() {
                return;
            }

            // Pre-flight validation before showing confirmation dialog
            if let Err(e) = crate::utils::validate_iso_file(&iso_path) {
                app.set_show_progress(true);
                app.set_status_text(format!("❌ {}", e).into());
                return;
            }

            let iso_size = match std::fs::metadata(&iso_path) {
                Ok(m) => m.len(),
                Err(e) => {
                    app.set_show_progress(true);
                    app.set_status_text(format!("❌ Failed to read ISO metadata: {}", e).into());
                    return;
                }
            };

            for dev in &targets {
                if iso_size > dev.size {
                    app.set_show_progress(true);
                    app.set_status_text(
                        format!(
                            "❌ ISO ({}) exceeds /dev/{} ({})",
                            crate::utils::format_size(iso_size),
                            dev.name,
                            crate::utils::format_size(dev.size)
                        )
                        .into(),
                    );
                    return;
                }
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

            let summary = format!(
                "ALL DATA on the following device(s) will be permanently erased:\n\n{}\n\nImage: {} ({})",
                target_lines.join("\n"),
                iso_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy(),
                crate::utils::format_size(iso_size),
            );

            app.set_confirm_summary(summary.into());
            app.set_show_confirm_dialog(true);
        });
    }

    // Confirm Flash -> execute directly if root, or via privileged helper (`pkexec`)
    {
        let app_weak = app_weak.clone();
        let state = Arc::clone(&state);
        app.on_confirm_flash(move |verify, resume| {
            let Some(app) = app_weak.upgrade() else {
                return;
            };

            let (iso_path, targets) = match state.lock() {
                Ok(mut st) => {
                    st.clear_logs();
                    (st.iso_path.clone(), st.selected_devices())
                }
                Err(_) => return,
            };

            let Some(iso_path) = iso_path else {
                return;
            };
            if targets.is_empty() {
                return;
            }

            let device_paths: Vec<PathBuf> = targets.iter().map(|d| d.path.clone()).collect();

            app.set_is_flashing(true);
            app.set_show_progress(true);
            app.set_status_text("Starting write process...".into());
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
                let res = if crate::utils::check_root_privileges() {
                    run_direct_write(
                        &iso_path,
                        &device_paths,
                        verify,
                        resume,
                        &app_bg,
                        &state_bg,
                        cancel_flag,
                    )
                } else {
                    run_pkexec_helper_write(
                        &iso_path,
                        &device_paths,
                        verify,
                        resume,
                        &app_bg,
                        &state_bg,
                    )
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
                                app.set_status_text("✅ Write completed successfully!".into());
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

    // Cancel Flash
    {
        let app_weak = app_weak.clone();
        let state = Arc::clone(&state);
        app.on_cancel_flash(move || {
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
                app.set_status_text("Cancelling write safely...".into());
            }
        });
    }

    // Reset progress panel so user can flash again without restarting the app
    {
        let app_weak = app_weak.clone();
        let state = Arc::clone(&state);
        app.on_reset_progress(move || {
            if let Some(app) = app_weak.upgrade() {
                app.set_show_progress(false);
                app.set_progress_value(0.0);
                app.set_status_text("Ready to flash".into());
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

fn run_direct_write(
    iso_path: &std::path::Path,
    device_paths: &[PathBuf],
    verify: bool,
    resume: bool,
    app_bg: &slint::Weak<AppWindow>,
    state_bg: &Arc<Mutex<GuiState>>,
    cancel_flag: Arc<AtomicBool>,
) -> Result<()> {
    // Validate and unmount every target device (never ignore unmount errors!)
    crate::utils::validate_iso_file(iso_path)?;
    for dev in device_paths {
        crate::utils::validate_device_path(dev)?;
        let dev_name = dev
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid device path: {:?}", dev))?;
        crate::utils::unmount_device(dev_name)?;
    }

    let (tx, rx) = std::sync::mpsc::channel();
    let app_rx = app_bg.clone();
    let state_rx = Arc::clone(state_bg);

    let rx_thread = std::thread::spawn(move || {
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
    });

    let res = crate::writer::write_iso_to_devices(
        iso_path,
        device_paths,
        verify,
        resume,
        Some(tx),
        Some(cancel_flag),
    );
    let _ = rx_thread.join();
    res
}

fn run_pkexec_helper_write(
    iso_path: &std::path::Path,
    device_paths: &[PathBuf],
    verify: bool,
    resume: bool,
    app_bg: &slint::Weak<AppWindow>,
    state_bg: &Arc<Mutex<GuiState>>,
) -> Result<()> {
    let exe_path = std::env::current_exe()?;

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

    let mut cmd = Command::new("pkexec");
    cmd.arg(&exe_path)
        .arg("internal-write")
        .arg("--iso")
        .arg(iso_path)
        .arg("--device");

    for dev in device_paths {
        cmd.arg(dev);
    }
    if verify {
        cmd.arg("--verify");
    }
    if resume {
        cmd.arg("--resume");
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
            anyhow::bail!("Privileged write helper exited with status {}", status);
        } else {
            anyhow::bail!("{}", detail);
        }
    }
}
