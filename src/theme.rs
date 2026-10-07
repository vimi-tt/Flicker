use std::process::Command;

#[derive(Debug, Clone)]
pub struct SystemTheme {
    pub is_dark: bool,
    pub accent_color: slint::Color,
}

impl Default for SystemTheme {
    fn default() -> Self {
        Self {
            is_dark: false,
            // Default GNOME Blue (#3584e4)
            accent_color: slint::Color::from_argb_u8(255, 53, 132, 228),
        }
    }
}

/// Detects the system color scheme and accent color via xdg-desktop-portal / gsettings.
pub fn detect_system_theme() -> SystemTheme {
    let mut theme = SystemTheme::default();

    // 1. Try querying xdg-desktop-portal via busctl
    if let Some((is_dark, accent)) = query_portal_settings() {
        theme.is_dark = is_dark;
        if let Some(acc) = accent {
            theme.accent_color = acc;
        }
        return theme;
    }

    // 2. Fallback: try querying GNOME gsettings
    if let Some(is_dark) = query_gsettings_color_scheme() {
        theme.is_dark = is_dark;
    }
    if let Some(accent) = query_gsettings_accent_color() {
        theme.accent_color = accent;
    }

    theme
}

fn query_portal_settings() -> Option<(bool, Option<slint::Color>)> {
    // Read color-scheme from org.freedesktop.portal.Settings
    let color_scheme_output = Command::new("busctl")
        .args([
            "--user",
            "call",
            "org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.Settings",
            "Read",
            "ss",
            "org.freedesktop.appearance",
            "color-scheme",
        ])
        .output()
        .ok()?;

    if !color_scheme_output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&color_scheme_output.stdout);
    // Format is usually: "v v u 1" (1 = dark, 2 = light, 0 = no-preference)
    let is_dark = stdout.contains(" 1");

    // Read accent-color if available
    let mut accent = None;
    if let Ok(accent_output) = Command::new("busctl")
        .args([
            "--user",
            "call",
            "org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.Settings",
            "Read",
            "ss",
            "org.freedesktop.appearance",
            "accent-color",
        ])
        .output()
    {
        if accent_output.status.success() {
            let acc_str = String::from_utf8_lossy(&accent_output.stdout);
            if let Some(parsed) = parse_portal_accent_color(&acc_str) {
                accent = Some(parsed);
            }
        }
    }

    Some((is_dark, accent))
}

fn parse_portal_accent_color(raw: &str) -> Option<slint::Color> {
    // Expected output format: "v v (ddd) 0.901961 0.176471 0.258824"
    if let Some(pos) = raw.find("(ddd)") {
        let nums_part = &raw[pos + 5..];
        let parts: Vec<&str> = nums_part.split_whitespace().collect();
        if parts.len() >= 3 {
            let r: f64 = parts[0].parse().ok()?;
            let g: f64 = parts[1].parse().ok()?;
            let b: f64 = parts[2].parse().ok()?;
            let r_u8 = (r * 255.0).round().clamp(0.0, 255.0) as u8;
            let g_u8 = (g * 255.0).round().clamp(0.0, 255.0) as u8;
            let b_u8 = (b * 255.0).round().clamp(0.0, 255.0) as u8;
            return Some(slint::Color::from_argb_u8(255, r_u8, g_u8, b_u8));
        }
    }
    None
}

fn query_gsettings_color_scheme() -> Option<bool> {
    let output = Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "color-scheme"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let val = String::from_utf8_lossy(&output.stdout).to_lowercase();
    Some(val.contains("dark"))
}

fn query_gsettings_accent_color() -> Option<slint::Color> {
    let output = Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "accent-color"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let val = String::from_utf8_lossy(&output.stdout)
        .trim()
        .trim_matches('\'')
        .to_lowercase();

    match val.as_str() {
        "blue" => Some(slint::Color::from_argb_u8(255, 53, 132, 228)),
        "teal" => Some(slint::Color::from_argb_u8(255, 33, 144, 164)),
        "green" => Some(slint::Color::from_argb_u8(255, 58, 148, 74)),
        "yellow" => Some(slint::Color::from_argb_u8(255, 200, 136, 0)),
        "orange" => Some(slint::Color::from_argb_u8(255, 237, 91, 0)),
        "red" => Some(slint::Color::from_argb_u8(255, 224, 27, 36)),
        "pink" => Some(slint::Color::from_argb_u8(255, 213, 97, 153)),
        "purple" => Some(slint::Color::from_argb_u8(255, 145, 65, 172)),
        "slate" => Some(slint::Color::from_argb_u8(255, 111, 131, 150)),
        _ => None,
    }
}
