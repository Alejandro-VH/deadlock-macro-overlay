/// Event types tracked by the Deadlock macro overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EventKind {
    T1Camps,          // Initial small jungle camps unlock at 02:00
    BreakableCrates,  // Wooden boxes and golden statues spawn at 03:00
    BridgeBuffs,      // Bridge powerup runes at 05:00 and every 5:00 thereafter
    T2Camps,          // Medium jungle unlock at 07:00
    SinnersSacrifice, // Sinner's Sacrifice slot machines & hard jungle (T3) unlock at 08:00
    SoulUrn,          // Soul Urn spawn at 10:00 and every 5:00 thereafter
    MidBossSpawn,     // Mid Boss initial appearance in the central pit at 10:00
    MidBossRespawn,   // Dynamic Mid Boss respawn countdown following a kill (F9)
}

impl EventKind {
    pub fn display_name(&self) -> &'static str {
        match self {
            EventKind::T1Camps => "Jungla T1 (Pequeña)",
            EventKind::BreakableCrates => "Cajas & Estatuas",
            EventKind::BridgeBuffs => "Bridge Buffs",
            EventKind::T2Camps => "Jungla T2 (Mediana)",
            EventKind::SinnersSacrifice => "Sinner's & Jungla T3",
            EventKind::SoulUrn => "Soul Urn",
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

    /// Approximate total duration span of this milestone for proportional progress display.
    pub fn interval_span(&self) -> u32 {
        match self.kind {
            EventKind::T1Camps => 120,
            EventKind::BreakableCrates => 60,
            EventKind::BridgeBuffs => 300,
            EventKind::T2Camps => 120,
            EventKind::SinnersSacrifice => 60,
            EventKind::SoulUrn => 300,
            EventKind::MidBossSpawn => 120,
            EventKind::MidBossRespawn => 300,
        }
    }

    /// Computes progress normalized between 0.0 and 1.0 leading up to the target spawn.
    pub fn progress(&self, current_seconds: u32) -> f32 {
        if current_seconds >= self.target_time {
            return 1.0;
        }
        let span = self.interval_span() as f32;
        let rem = self.remaining_seconds(current_seconds) as f32;
        let elapsed = span - rem;
        (elapsed / span).clamp(0.0, 1.0)
    }
}

/// A collection of simultaneous macro events occurring at the exact same match time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MilestoneGroup {
    pub target_time: u32,
    pub remaining_seconds: i32,
    pub status: EventStatus,
    pub events: Vec<MacroEvent>,
}

impl MilestoneGroup {
    pub fn progress(&self, current_seconds: u32) -> f32 {
        if let Some(first) = self.events.first() {
            first.progress(current_seconds)
        } else {
            0.0
        }
    }
}

/// Tracks manual breakable box / golden statue farm routes (180s cycle).
#[derive(Debug, Clone, Default)]
pub struct BoxRouteTracker {
    pub active_target: Option<u32>,
}

impl BoxRouteTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Toggles the 180s (3m) box route countdown.
    /// Returns true if started, false if cancelled.
    pub fn toggle(&mut self, match_time: u32) -> bool {
        if self.is_active(match_time) {
            self.active_target = None;
            false
        } else {
            self.active_target = Some(match_time + 180);
            true
        }
    }

    pub fn is_active(&self, match_time: u32) -> bool {
        match self.active_target {
            Some(target) => match_time <= target + 10,
            None => false,
        }
    }

    pub fn get_remaining(&self, match_time: u32) -> Option<u32> {
        match self.active_target {
            Some(target) if match_time <= target + 10 => {
                Some(target.saturating_sub(match_time))
            }
            _ => None,
        }
    }

    pub fn cancel(&mut self) {
        self.active_target = None;
    }

    pub fn reset(&mut self) {
        self.active_target = None;
    }
}

/// Tracks manual kills and dynamic respawn countdowns for the Mid Boss objective.
#[derive(Debug, Clone, Default)]
pub struct MidBossTracker {
    pub kills: u32,
    pub next_respawn_time: Option<u32>,
}

impl MidBossTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_active(&self, current_seconds: u32) -> bool {
        self.get_active_respawn(current_seconds).is_some()
    }

    /// Toggles Mid Boss kill timer with a single key.
    /// Returns true if started, false if cancelled.
    pub fn toggle_kill(&mut self, match_time: u32) -> bool {
        if self.is_active(match_time) {
            self.cancel_kill();
            false
        } else {
            self.record_kill(match_time);
            true
        }
    }

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
    pub boxes: BoxRouteTracker,
}

impl MacroScheduler {
    pub fn calculate_events(&self, current_seconds: u32) -> Vec<MacroEvent> {
        let mut events = Vec::new();

        // 1. Jungla T1 / Pequeña (Hito inicial a los 120s / 02:00, visible hasta 10s después)
        if current_seconds <= 130 {
            events.push(MacroEvent {
                kind: EventKind::T1Camps,
                target_time: 120,
            });
        }

        // 2. Cajas & Estatuas de Almas (Primer spawn a los 180s / 03:00, visible hasta 10s después)
        if current_seconds <= 190 {
            events.push(MacroEvent {
                kind: EventKind::BreakableCrates,
                target_time: 180,
            });
        }

        // 3. Bridge Buffs (Primer spawn a los 300s / 05:00, luego cada 300s: 10:00, 15:00, 20:00...)
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
        // Include subsequent recurring cycle for horizon projection
        events.push(MacroEvent {
            kind: EventKind::BridgeBuffs,
            target_time: bridge_target + 300,
        });

        // 4. Jungla T2 / Mediana (Hito inicial a los 420s / 07:00, visible hasta 10s después)
        if current_seconds <= 430 {
            events.push(MacroEvent {
                kind: EventKind::T2Camps,
                target_time: 420,
            });
        }

        // 5. Sinner's Sacrifice y Jungla T3 (Hito a los 480s / 08:00, visible hasta 10s después)
        if current_seconds <= 490 {
            events.push(MacroEvent {
                kind: EventKind::SinnersSacrifice,
                target_time: 480,
            });
        }

        // 6. Soul Urn (Primer spawn a los 600s / 10:00, luego cada 300s: 15:00, 20:00, 25:00...)
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
        // Include subsequent recurring cycle for horizon projection
        events.push(MacroEvent {
            kind: EventKind::SoulUrn,
            target_time: urn_target + 300,
        });

        // 7. Mid Boss Spawn Inicial a los 10:00 (600s) si nunca ha muerto aún
        if self.midboss.kills == 0 && current_seconds <= 610 {
            events.push(MacroEvent {
                kind: EventKind::MidBossSpawn,
                target_time: 600,
            });
        }

        // Ordenar cronológicamente por tiempo objetivo
        events.sort_by_key(|e| e.target_time);
        events
    }

    /// Returns both the immediate milestone group and the subsequent milestone group.
    /// Each group contains all events sharing that specific target time.
    pub fn get_grouped_horizons(
        &self,
        current_seconds: u32,
    ) -> (Option<MilestoneGroup>, Option<MilestoneGroup>) {
        let all_events = self.calculate_events(current_seconds);
        if let Some(first) = all_events.first() {
            let earliest_time = first.target_time;
            let immediate_events: Vec<MacroEvent> = all_events
                .iter()
                .filter(|e| e.target_time == earliest_time)
                .cloned()
                .collect();

            let immediate_status = immediate_events
                .first()
                .map(|e| e.status(current_seconds))
                .unwrap_or(EventStatus::Upcoming);

            let immediate_remaining = first.remaining_seconds(current_seconds);

            let immediate_group = MilestoneGroup {
                target_time: earliest_time,
                remaining_seconds: immediate_remaining,
                status: immediate_status,
                events: immediate_events,
            };

            let next_target = all_events
                .iter()
                .find(|e| e.target_time > earliest_time)
                .map(|e| e.target_time);

            let next_group = next_target.map(|nxt_time| {
                let nxt_events: Vec<MacroEvent> = all_events
                    .iter()
                    .filter(|e| e.target_time == nxt_time)
                    .cloned()
                    .collect();

                let nxt_status = nxt_events
                    .first()
                    .map(|e| e.status(current_seconds))
                    .unwrap_or(EventStatus::Upcoming);

                let nxt_remaining = nxt_events
                    .first()
                    .map(|e| e.remaining_seconds(current_seconds))
                    .unwrap_or(0);

                MilestoneGroup {
                    target_time: nxt_time,
                    remaining_seconds: nxt_remaining,
                    status: nxt_status,
                    events: nxt_events,
                }
            });

            (Some(immediate_group), next_group)
        } else {
            (None, None)
        }
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

    /// Returns both the immediate milestone(s) and a preview of the subsequent milestone.
    pub fn get_immediate_and_next_events(
        &self,
        current_seconds: u32,
    ) -> (Vec<MacroEvent>, Option<MacroEvent>) {
        let all_events = self.calculate_events(current_seconds);
        if let Some(first) = all_events.first() {
            let earliest_time = first.target_time;
            let immediate: Vec<MacroEvent> = all_events
                .iter()
                .filter(|e| e.target_time == earliest_time)
                .cloned()
                .collect();

            let next = all_events
                .iter()
                .find(|e| e.target_time > earliest_time)
                .cloned();

            (immediate, next)
        } else {
            (Vec::new(), None)
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

            let prev = self.current_seconds;
            let diff = (raw as i32 - prev as i32).abs();
            if diff <= 3 {
                self.current_seconds = raw;
                self.pending_candidate = None;
                self.consecutive_reads = 0;
                return raw != prev;
            }

            if let Some(pending) = self.pending_candidate {
                if (raw as i32 - pending as i32).abs() <= 2 {
                    self.consecutive_reads += 1;
                    if self.consecutive_reads >= 2 {
                        self.current_seconds = raw;
                        self.pending_candidate = None;
                        self.consecutive_reads = 0;
                        return raw != prev;
                    }
                } else {
                    self.pending_candidate = Some(raw);
                    self.consecutive_reads = 1;
                }
            } else {
                self.pending_candidate = Some(raw);
                self.consecutive_reads = 1;
            }
        }
        false
    }

    /// Increments the local clock by 1 second during OCR occlusions.
    pub fn tick_second(&mut self) {
        if self.is_active {
            self.current_seconds = self.current_seconds.saturating_add(1);
        }
    }

    pub fn adjust_seconds(&mut self, delta: i32) {
        if delta < 0 {
            self.current_seconds = self.current_seconds.saturating_sub(delta.unsigned_abs());
        } else {
            self.current_seconds = self.current_seconds.saturating_add(delta as u32);
        }
    }

    pub fn reset(&mut self) {
        self.current_seconds = 0;
        self.pending_candidate = None;
        self.consecutive_reads = 0;
        self.is_active = false;
    }
}

pub fn format_time_mmss(total_seconds: u32) -> String {
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    format!("{:02}:{:02}", minutes, seconds)
}
