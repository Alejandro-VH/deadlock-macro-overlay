use std::ffi::OsStr;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

pub const APP_DIR_NAME: &str = "DeadlockMacroOverlay";
pub const CONFIG_FILE_NAME: &str = "deadlock_overlay.cfg";

/// Coordinates and dimensions for overlay positioning, OCR sampling, and preferences.
#[derive(Debug, Clone, PartialEq)]
pub struct OverlayConfig {
    pub win_x: i32,
    pub win_y: i32,
    pub roi_x: i32,
    pub roi_y: i32,
    pub roi_w: i32,
    pub roi_h: i32,
    pub last_hero: String,
    pub is_muted: bool,
    pub alert_30s_sound: bool,
    pub hud_scale: f32,
    pub bg_opacity: f32,
    pub key_box_timer: u16,
    pub key_midboss: u16,
    pub key_interactive: u16,
}

impl OverlayConfig {
    /// Computes default geometry relative to the primary display resolution.
    pub fn default_for_resolution(screen_width: i32, screen_height: i32) -> Self {
        let _ = screen_height;
        Self {
            win_x: screen_width - 300,
            win_y: 100,
            roi_x: (screen_width / 2) - 60,
            roi_y: 8,
            roi_w: 120,
            roi_h: 42,
            last_hero: String::new(),
            is_muted: false,
            alert_30s_sound: true,
            hud_scale: 1.0,
            bg_opacity: 0.92,
            key_box_timer: 0x77,   // VK_F8
            key_midboss: 0x78,     // VK_F9
            key_interactive: 0x79, // VK_F10
        }
    }

    /// Serializes configuration values to a plain-text key-value file.
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        let mut file = File::create(path)?;
        writeln!(file, "win_x={}", self.win_x)?;
        writeln!(file, "win_y={}", self.win_y)?;
        writeln!(file, "roi_x={}", self.roi_x)?;
        writeln!(file, "roi_y={}", self.roi_y)?;
        writeln!(file, "roi_w={}", self.roi_w)?;
        writeln!(file, "roi_h={}", self.roi_h)?;
        if !self.last_hero.is_empty() {
            writeln!(file, "last_hero={}", self.last_hero)?;
        }
        writeln!(file, "is_muted={}", self.is_muted)?;
        writeln!(file, "alert_30s_sound={}", self.alert_30s_sound)?;
        writeln!(file, "hud_scale={:.2}", self.hud_scale)?;
        writeln!(file, "bg_opacity={:.2}", self.bg_opacity)?;
        writeln!(file, "key_box_timer={}", key_to_display_name(self.key_box_timer))?;
        writeln!(file, "key_midboss={}", key_to_display_name(self.key_midboss))?;
        writeln!(file, "key_interactive={}", key_to_display_name(self.key_interactive))?;
        file.flush()
    }

    /// Deserializes configuration values from a file, falling back gracefully on missing values.
    pub fn load_from_file<P: AsRef<Path>>(path: P, fallback_width: i32, fallback_height: i32) -> Self {
        let mut config = Self::default_for_resolution(fallback_width, fallback_height);

        let file = match File::open(path) {
            Ok(f) => f,
            Err(_) => return config,
        };

        let reader = BufReader::new(file);
        for line in reader.lines().map_while(Result::ok) {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, val)) = line.split_once('=') {
                let key = key.trim();
                let val = val.trim();
                match key {
                    "win_x" => {
                        if let Ok(v) = val.parse::<i32>() {
                            config.win_x = v;
                        }
                    }
                    "win_y" => {
                        if let Ok(v) = val.parse::<i32>() {
                            config.win_y = v;
                        }
                    }
                    "roi_x" => {
                        if let Ok(v) = val.parse::<i32>() {
                            config.roi_x = v;
                        }
                    }
                    "roi_y" => {
                        if let Ok(v) = val.parse::<i32>() {
                            config.roi_y = v;
                        }
                    }
                    "roi_w" => {
                        if let Ok(v) = val.parse::<i32>() {
                            config.roi_w = v;
                        }
                    }
                    "roi_h" => {
                        if let Ok(v) = val.parse::<i32>() {
                            config.roi_h = v;
                        }
                    }
                    "last_hero" => {
                        if !val.is_empty() {
                            config.last_hero = val.to_string();
                        }
                    }
                    "is_muted" => {
                        if let Ok(v) = val.parse::<bool>() {
                            config.is_muted = v;
                        }
                    }
                    "alert_30s_sound" => {
                        if let Ok(v) = val.parse::<bool>() {
                            config.alert_30s_sound = v;
                        }
                    }
                    "hud_scale" => {
                        if let Ok(v) = val.parse::<f32>() {
                            config.hud_scale = v;
                        }
                    }
                    "bg_opacity" => {
                        if let Ok(v) = val.parse::<f32>() {
                            config.bg_opacity = v;
                        }
                    }
                    "key_box_timer" => {
                        if let Some(vk) = display_name_to_key(val) {
                            config.key_box_timer = vk;
                        }
                    }
                    "key_midboss" => {
                        if let Some(vk) = display_name_to_key(val) {
                            config.key_midboss = vk;
                        }
                    }
                    "key_interactive" => {
                        if let Some(vk) = display_name_to_key(val) {
                            config.key_interactive = vk;
                        }
                    }
                    _ => {}
                }
            }
        }

        config
    }

    /// Resolves configuration path relative to the provided APPDATA directory, or falls back to local file.
    pub fn resolve_config_path_from_env(appdata_var: Option<&OsStr>) -> PathBuf {
        match appdata_var {
            Some(dir) if !dir.is_empty() => {
                PathBuf::from(dir).join(APP_DIR_NAME).join(CONFIG_FILE_NAME)
            }
            _ => PathBuf::from(CONFIG_FILE_NAME),
        }
    }

    /// Resolves standard configuration path in %APPDATA%\DeadlockMacroOverlay\deadlock_overlay.cfg.
    pub fn resolve_config_path() -> PathBuf {
        Self::resolve_config_path_from_env(std::env::var_os("APPDATA").as_deref())
    }

    /// Clamps physical window coordinates given explicit physical window dimensions.
    pub fn clamp_to_screen_with_size(
        &mut self,
        screen_width: i32,
        screen_height: i32,
        win_phys_width: i32,
        win_phys_height: i32,
    ) {
        let max_x = (screen_width - win_phys_width.max(280)).max(0);
        let max_y = (screen_height - win_phys_height.max(120)).max(0);
        self.win_x = self.win_x.clamp(0, max_x);
        self.win_y = self.win_y.clamp(0, max_y);
    }

    /// Clamps physical window coordinates so the overlay remains visible inside the primary monitor.
    pub fn clamp_to_screen(&mut self, screen_width: i32, screen_height: i32) {
        self.clamp_to_screen_with_size(screen_width, screen_height, 280, 120);
    }

    /// Converts physical pixel coordinates (`win_x`, `win_y`) into egui logical points for a given DPI scale.
    pub fn logical_position(&self, pixels_per_point: f32) -> (f32, f32) {
        let ppp = pixels_per_point.max(0.5);
        (self.win_x as f32 / ppp, self.win_y as f32 / ppp)
    }

    /// Loads existing configuration from disk or automatically initializes default settings on first run.
    pub fn load_or_create<P: AsRef<Path>>(
        path: P,
        fallback_width: i32,
        fallback_height: i32,
    ) -> Self {
        let p = path.as_ref();
        if p.exists() {
            let mut config = Self::load_from_file(p, fallback_width, fallback_height);
            let orig_x = config.win_x;
            let orig_y = config.win_y;
            config.clamp_to_screen(fallback_width, fallback_height);
            if config.win_x != orig_x || config.win_y != orig_y {
                let _ = config.save_to_file(p);
            }
            config
        } else {
            if let Some(parent) = p.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let mut config = Self::default_for_resolution(fallback_width, fallback_height);
            config.clamp_to_screen(fallback_width, fallback_height);
            let _ = config.save_to_file(p);
            config
        }
    }
}

/// Converts a Win32 virtual key code into a human-readable display string.
pub fn key_to_display_name(vk: u16) -> String {
    match vk {
        0x01 => "Mouse1".to_string(),
        0x02 => "Mouse2".to_string(),
        0x04 => "Mouse3".to_string(),
        0x05 => "Mouse4".to_string(),
        0x06 => "Mouse5".to_string(),
        0x09 => "Tab".to_string(),
        0x10 => "Shift".to_string(),
        0x11 => "Ctrl".to_string(),
        0x12 => "Alt".to_string(),
        0x14 => "CapsLock".to_string(),
        0x1B => "Esc".to_string(),
        0x20 => "Space".to_string(),
        0x30..=0x39 => format!("{}", (vk - 0x30) as u8 as char),
        0x41..=0x5A => format!("{}", vk as u8 as char),
        0x70..=0x7B => format!("F{}", vk - 0x6F),
        _ => format!("0x{:02X}", vk),
    }
}

/// Parses a human-readable key string (or hex string) into a Win32 virtual key code.
pub fn display_name_to_key(name: &str) -> Option<u16> {
    let lower = name.trim().to_lowercase();
    match lower.as_str() {
        "mouse1" => Some(0x01),
        "mouse2" => Some(0x02),
        "mouse3" => Some(0x04),
        "mouse4" => Some(0x05),
        "mouse5" => Some(0x06),
        "tab" => Some(0x09),
        "shift" => Some(0x10),
        "ctrl" => Some(0x11),
        "alt" => Some(0x12),
        "capslock" => Some(0x14),
        "esc" | "escape" => Some(0x1B),
        "space" => Some(0x20),
        s if s.starts_with('f') && s.len() >= 2 => {
            if let Ok(num) = s[1..].parse::<u16>() {
                if (1..=12).contains(&num) {
                    return Some(0x6F + num);
                }
            }
            None
        }
        s if s.len() == 1 => {
            let ch = s.chars().next().unwrap();
            if ch.is_ascii_alphabetic() {
                Some(ch.to_ascii_uppercase() as u16)
            } else if ch.is_ascii_digit() {
                Some(ch as u16)
            } else {
                None
            }
        }
        s if s.starts_with("0x") => u16::from_str_radix(&s[2..], 16).ok(),
        _ => None,
    }
}
