use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeroSelectionSource {
    Auto,
    Manual,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeroInfo {
    pub id: String,
    pub display_name: String,
    pub source: HeroSelectionSource,
}

impl Default for HeroInfo {
    fn default() -> Self {
        Self {
            id: String::new(),
            display_name: "None".to_string(),
            source: HeroSelectionSource::Auto,
        }
    }
}

pub static HERO_ROSTER: &[(&str, &str)] = &[
    ("hero_hornet", "Vindicta"),
    ("hero_inferno", "Infernus"),
    ("hero_gigawatt", "Seven"),
    ("hero_atlas", "Abrams"),
    ("hero_werewolf", "Shiv"),
    ("hero_chrono", "Paradox"),
    ("hero_yamato", "Yamato"),
    ("hero_haze", "Haze"),
    ("hero_kelvin_v2", "Kelvin"),
    ("hero_dynamo", "Dynamo"),
    ("hero_ghost", "Lady Geist"),
    ("hero_digger", "Mo & Krill"),
    ("hero_bebop", "Bebop"),
    ("hero_warden", "Warden"),
    ("hero_wraith", "Wraith"),
    ("hero_mcginnis", "McGinnis"),
    ("hero_viscous", "Viscous"),
    ("hero_mirage", "Mirage"),
    ("hero_pocket", "Pocket"),
    ("hero_lash", "Lash"),
];

pub fn get_all_heroes() -> &'static [(&'static str, &'static str)] {
    HERO_ROSTER
}

pub fn map_internal_hero_id(internal_id: &str) -> String {
    for (id, display) in HERO_ROSTER {
        if *id == internal_id {
            return display.to_string();
        }
    }

    // Fallback: strip "hero_" prefix and title case words
    let cleaned = internal_id.strip_prefix("hero_").unwrap_or(internal_id);
    cleaned
        .split('_')
        .map(|word| {
            let mut c = word.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect::<Vec<String>>()
        .join(" ")
}

pub fn parse_hero_from_log_line(line: &str) -> Option<String> {
    // Pattern 1: Attempting to spawn hero hero_<name>
    if let Some(pos) = line.find("Attempting to spawn hero ") {
        let remainder = &line[pos + "Attempting to spawn hero ".len()..];
        let token = remainder.split_whitespace().next().unwrap_or("");
        let hero_id = token.trim_matches(|c: char| !c.is_alphanumeric() && c != '_');
        if !hero_id.is_empty() {
            return Some(hero_id.to_string());
        }
    }

    // Pattern 2: [Server] Loaded hero <id>/hero_<name>
    if let Some(pos) = line.find("[Server] Loaded hero ") {
        let remainder = &line[pos + "[Server] Loaded hero ".len()..];
        let token = remainder.split_whitespace().next().unwrap_or("");
        let hero_part = token.split('/').last().unwrap_or(token);
        let hero_id = hero_part.trim_matches(|c: char| !c.is_alphanumeric() && c != '_');
        if !hero_id.is_empty() {
            return Some(hero_id.to_string());
        }
    }

    None
}

pub struct HeroTracker {
    current_hero: HeroInfo,
    log_path: Option<PathBuf>,
    last_offset: u64,
    last_created: Option<SystemTime>,
}

impl HeroTracker {
    pub fn new(custom_log_path: Option<PathBuf>) -> Self {
        let path = custom_log_path.or_else(Self::find_default_deadlock_log_path);
        let (initial_offset, initial_created) = if let Some(ref p) = path {
            if let Ok(m) = std::fs::metadata(p) {
                // Read last 64 KB so games already in progress are detected immediately on startup
                (m.len().saturating_sub(64 * 1024), m.created().ok())
            } else {
                (0, None)
            }
        } else {
            (0, None)
        };

        Self {
            current_hero: HeroInfo::default(),
            log_path: path,
            last_offset: initial_offset,
            last_created: initial_created,
        }
    }

    pub fn get_hero(&self) -> HeroInfo {
        self.current_hero.clone()
    }

    pub fn set_manual_hero(&mut self, hero_name: &str) {
        self.current_hero = HeroInfo {
            id: String::new(),
            display_name: hero_name.to_string(),
            source: HeroSelectionSource::Manual,
        };
    }

    pub fn reset_to_auto(&mut self) {
        self.current_hero.source = HeroSelectionSource::Auto;
    }

    pub fn poll_log(&mut self) -> Option<String> {
        let path = self.log_path.as_ref()?;
        let file = File::open(path).ok()?;
        let metadata = file.metadata().ok()?;
        let file_len = metadata.len();
        let created = metadata.created().ok();

        if (self.last_created.is_some() && created != self.last_created)
            || file_len < self.last_offset
        {
            // File was truncated or recreated on game restart
            self.last_offset = 0;
            self.last_created = created;
        }

        let mut reader = BufReader::new(file);
        reader.seek(SeekFrom::Start(self.last_offset)).ok()?;

        let mut latest_detected = None;
        let mut line = String::new();
        let mut current_pos = self.last_offset;

        while let Ok(bytes_read) = reader.read_line(&mut line) {
            if bytes_read == 0 {
                break;
            }

            // Do not consume incomplete lines written mid-flight
            if !line.ends_with('\n') && !line.ends_with('\r') {
                break;
            }

            current_pos += bytes_read as u64;

            if let Some(hero_id) = parse_hero_from_log_line(&line) {
                let display = map_internal_hero_id(&hero_id);
                if self.current_hero.source != HeroSelectionSource::Manual {
                    self.current_hero = HeroInfo {
                        id: hero_id,
                        display_name: display.clone(),
                        source: HeroSelectionSource::Auto,
                    };
                }
                latest_detected = Some(display);
            }
            line.clear();
        }

        self.last_offset = current_pos;
        latest_detected
    }

    pub fn find_default_deadlock_log_path() -> Option<PathBuf> {
        let candidate_paths = [
            r"C:\Program Files (x86)\Steam\steamapps\common\Deadlock\game\citadel\console.log",
            r"D:\SteamLibrary\steamapps\common\Deadlock\game\citadel\console.log",
            r"E:\SteamLibrary\steamapps\common\Deadlock\game\citadel\console.log",
            r"C:\SteamLibrary\steamapps\common\Deadlock\game\citadel\console.log",
        ];

        for path in candidate_paths {
            let p = PathBuf::from(path);
            if p.exists() {
                return Some(p);
            }
        }

        None
    }
}
