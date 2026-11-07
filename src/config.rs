use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

/// Coordinates and dimensions for overlay positioning and OCR sampling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayConfig {
    pub win_x: i32,
    pub win_y: i32,
    pub roi_x: i32,
    pub roi_y: i32,
    pub roi_w: i32,
    pub roi_h: i32,
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
                    _ => {}
                }
            }
        }

        config
    }
}
