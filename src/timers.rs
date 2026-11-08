/// Event types tracked by the Deadlock macro overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EventKind {
    BridgeBuffs,
    SoulUrn,
    SinnersSacrifice,
    MidBossSpawn,  // Initial appearance at 10:00
    MidBossRespawn, // Respawn countdown after kill
}

impl EventKind {
    pub fn display_name(&self) -> &'static str {
        match self {
            EventKind::BridgeBuffs => "Bridge Buffs",
            EventKind::SoulUrn => "Soul Urn",
            EventKind::SinnersSacrifice => "Sinner's Sacrifice",
            EventKind::MidBossSpawn => "Mid Boss Spawn",
            EventKind::MidBossRespawn => "Mid Boss Respawn",
        }
    }
}

/// Dynamic alert status of an event based on remaining seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventStatus {
    Upcoming,
    Warning, // <= 30 seconds
    Active,  // Within 10 seconds of activation
}

/// A scheduled or active macro event in the match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroEvent {
    pub kind: EventKind,
    pub target_time: u32,
}

impl MacroEvent {
    pub fn remaining_seconds(&self, current_seconds: u32) -> i32 {
        self.target_time as i32 - current_seconds as i32
    }

    pub fn status(&self, current_seconds: u32) -> EventStatus {
        if current_seconds >= self.target_time && current_seconds <= self.target_time + 10 {
            EventStatus::Active
        } else {
            let rem = self.remaining_seconds(current_seconds);
            if (1..=30).contains(&rem) {
                EventStatus::Warning
            } else {
                EventStatus::Upcoming
            }
        }
    }
}

/// Tracks manual kills and dynamic respawn countdowns for the Mid Boss objective.
#[derive(Debug, Clone, Default)]
pub struct MidBossTracker {
    pub kills: u32,
    pub next_respawn_time: Option<u32>,
}

impl MidBossTracker {
    pub fn record_kill(&mut self, match_time: u32) {
        self.kills += 1;
        let cooldown = match self.kills {
            1 => 420, // 7 minutes
            2 => 360, // 6 minutes
            _ => 300, // 5 minutes
        };
        self.next_respawn_time = Some(match_time + cooldown);
    }

    pub fn cancel_kill(&mut self) {
        if self.kills > 0 {
            self.kills -= 1;
        }
        self.next_respawn_time = None;
    }

    pub fn reset(&mut self) {
        self.kills = 0;
        self.next_respawn_time = None;
    }

    /// Returns active respawn target only while event is pending or within the 10s active window.
    pub fn get_active_respawn(&self, current_seconds: u32) -> Option<u32> {
        match self.next_respawn_time {
            Some(respawn) if current_seconds <= respawn + 10 => Some(respawn),
            _ => None,
        }
    }
}

/// Computes the active and upcoming global events schedule given the match time.
#[derive(Debug, Clone, Default)]
pub struct MacroScheduler {
    pub midboss: MidBossTracker,
}

impl MacroScheduler {
    pub fn calculate_events(&self, current_seconds: u32) -> Vec<MacroEvent> {
        let mut events = Vec::new();

        // 1. Bridge Buffs (First spawn at 300s / 05:00, then every 300s: 10:00, 15:00, 20:00...)
        let bridge_target = if current_seconds < 300 {
            300
        } else {
            let cycle_start = (current_seconds / 300) * 300;
            if current_seconds <= cycle_start + 10 {
                cycle_start
            } else {
                cycle_start + 300
            }
        };
        events.push(MacroEvent {
            kind: EventKind::BridgeBuffs,
            target_time: bridge_target,
        });

        // 2. Soul Urn (First spawn at 600s / 10:00, then every 300s: 15:00, 20:00, 25:00...)
        let urn_target = if current_seconds < 600 {
            600
        } else {
            let cycle_start = 600 + ((current_seconds - 600) / 300) * 300;
            if current_seconds <= cycle_start + 10 {
                cycle_start
            } else {
                cycle_start + 300
            }
        };
        events.push(MacroEvent {
            kind: EventKind::SoulUrn,
            target_time: urn_target,
        });

        // 3. Sinner's Sacrifice (Single initial event at 480s / 08:00, vanishes after 10s grace period)
        if current_seconds <= 490 {
            events.push(MacroEvent {
                kind: EventKind::SinnersSacrifice,
                target_time: 480,
            });
        }

        // 4. Mid Boss Initial Spawn at 10:00 (600s) if never killed yet
        if self.midboss.kills == 0 && current_seconds <= 610 {
            events.push(MacroEvent {
                kind: EventKind::MidBossSpawn,
                target_time: 600,
            });
        }

        // 5. Mid Boss Respawn (if manually recorded with F9 and actively counting down)
        if let Some(respawn) = self.midboss.get_active_respawn(current_seconds) {
            events.push(MacroEvent {
                kind: EventKind::MidBossRespawn,
                target_time: respawn,
            });
        }

        // Sort events chronologically by target_time
        events.sort_by_key(|e| e.target_time);
        events
    }

    /// Returns only the immediate next active or upcoming event(s) sharing the earliest target time.
    pub fn get_immediate_next_events(&self, current_seconds: u32) -> Vec<MacroEvent> {
        let all_events = self.calculate_events(current_seconds);
        if let Some(first) = all_events.first() {
            let earliest_time = first.target_time;
            all_events
                .into_iter()
                .filter(|e| e.target_time == earliest_time)
                .collect()
        } else {
            Vec::new()
        }
    }
}

/// Noise-filtering and drift-compensating match clock.
#[derive(Debug, Clone, Default)]
pub struct ClockFilter {
    current_seconds: u32,
    pending_candidate: Option<u32>,
    consecutive_reads: u32,
    is_active: bool,
}

impl ClockFilter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn current_seconds(&self) -> u32 {
        self.current_seconds
    }

    pub fn is_active(&self) -> bool {
        self.is_active
    }

    /// Feeds raw OCR reading. Returns true if the clock was updated directly from OCR.
    pub fn feed_ocr_reading(&mut self, raw_time: Option<u32>) -> bool {
        if let Some(raw) = raw_time {
            self.is_active = true;
            if self.current_seconds == 0 {
                self.current_seconds = raw;
                self.pending_candidate = None;
                self.consecutive_reads = 0;
                return true;
            }

            let diff = (raw as i32 - self.current_seconds as i32).abs();
            if diff <= 3 {
                self.current_seconds = raw;
                self.pending_candidate = None;
                self.consecutive_reads = 0;
                return true;
            } else if Some(raw) == self.pending_candidate {
                self.consecutive_reads += 1;
                if self.consecutive_reads >= 1 {
                    self.current_seconds = raw;
                    self.pending_candidate = None;
                    self.consecutive_reads = 0;
                    return true;
                }
            } else {
                self.pending_candidate = Some(raw);
                self.consecutive_reads = 0;
            }
        }
        false
    }

    pub fn tick_second(&mut self) {
        if self.is_active {
            self.current_seconds += 1;
        }
    }

    pub fn set_manual_time(&mut self, seconds: u32) {
        self.current_seconds = seconds;
        self.is_active = true;
    }

    pub fn adjust_seconds(&mut self, delta: i32) {
        let new_val = (self.current_seconds as i32 + delta).max(0);
        self.current_seconds = new_val as u32;
        self.is_active = true;
    }

    pub fn toggle_active(&mut self) {
        self.is_active = !self.is_active;
    }

    pub fn reset(&mut self) {
        self.current_seconds = 0;
        self.pending_candidate = None;
        self.consecutive_reads = 0;
        self.is_active = false;
    }
}

/// Helper to format raw seconds into standard MM:SS string.
pub fn format_time_mmss(total_seconds: u32) -> String {
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    format!("{:02}:{:02}", minutes, seconds)
}
