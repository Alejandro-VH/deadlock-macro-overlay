/// Event types tracked by the Deadlock macro overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EventKind {
    // Unique Fixed Events
    T1Camps,             // 120s (02:00): "Campamentos T1"
    BreakableCrates,     // 180s (03:00): "Cajas y Estatuas"
    T2Camps,             // 300s (05:00): "Campamentos T2"
    GuardiansZeroResist, // 360s (06:00): "Guardians caen a 0% Resistencia"
    SinnersSacrifice,    // 480s (08:00): "Campamentos T3 y Sinner's Sacrifice"
    WalkersZeroResist,   // 540s (09:00): "Walkers caen a 0% Resistencia"
    MidBossAvailable,    // 600s (10:00): "Mid Boss Disponible y Cajas de Mid"
    UnstableRift,        // 660s (11:00): "Ventana 1er Unstable Rift (11:00-13:00)"
    WalkersBonusDamage,  // 1080s (18:00): "Walkers reciben un +65% Daño"
    Waves25s,            // 1200s (20:00): "Oleadas cada 25s"
    GoldenStatuesT2,     // 1800s (30:00): "Estatuas doradas pasan a Tier 2"
    Waves20sAndHealth,   // 2100s (35:00): "Oleadas cada 20s y +50% Vida"
    BridgeBuffsMaxPower, // 2400s (40:00): "Bridge Buffs Poder Máximo"

    // Cyclic Events (every 300s)
    BridgeBuffs,         // 300s, 600s, 900s...: "Bridge Buffs"
    SoulUrnYellow,       // (next_urn_sec / 300) % 2 == 0: "Soul Urn Amarilla"
    SoulUrnGreen,        // (next_urn_sec / 300) % 2 == 1: "Soul Urn Verde"

    // Manual Event (F8)
    BoxRoute,            // 180s (3:00): "Ruta de Cajas"

    // Legacy variants for backwards compatibility
    MidBossSpawn,
    MidBossRespawn,
    SoulUrn,
}

impl EventKind {
    pub fn display_name(&self) -> &'static str {
        match self {
            EventKind::T1Camps => "Campamentos T1",
            EventKind::BreakableCrates => "Cajas y Estatuas",
            EventKind::T2Camps => "Campamentos T2",
            EventKind::GuardiansZeroResist => "Guardians caen a 0% Resistencia",
            EventKind::SinnersSacrifice => "Campamentos T3 y Sinner's Sacrifice",
            EventKind::WalkersZeroResist => "Walkers caen a 0% Resistencia",
            EventKind::MidBossAvailable => "Mid Boss Disponible y Cajas de Mid",
            EventKind::UnstableRift => "Ventana 1er Unstable Rift (11:00-13:00)",
            EventKind::WalkersBonusDamage => "Walkers reciben un +65% Daño",
            EventKind::Waves25s => "Oleadas cada 25s",
            EventKind::GoldenStatuesT2 => "Estatuas doradas pasan a Tier 2",
            EventKind::Waves20sAndHealth => "Oleadas cada 20s y +50% Vida",
            EventKind::BridgeBuffsMaxPower => "Bridge Buffs Poder Máximo",
            EventKind::BridgeBuffs => "Bridge Buffs",
            EventKind::SoulUrnYellow => "Soul Urn Amarilla",
            EventKind::SoulUrnGreen => "Soul Urn Verde",
            EventKind::BoxRoute => "Ruta de Cajas",
            EventKind::MidBossSpawn => "Mid Boss Disponible y Cajas de Mid",
            EventKind::MidBossRespawn => "Mid Boss Respawn",
            EventKind::SoulUrn => "Soul Urn",
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
            EventKind::T2Camps => 120,
            EventKind::GuardiansZeroResist => 60,
            EventKind::SinnersSacrifice => 120,
            EventKind::WalkersZeroResist => 60,
            EventKind::MidBossAvailable | EventKind::MidBossSpawn => 60,
            EventKind::UnstableRift => 60,
            EventKind::WalkersBonusDamage => 120,
            EventKind::Waves25s => 120,
            EventKind::GoldenStatuesT2 => 120,
            EventKind::Waves20sAndHealth => 120,
            EventKind::BridgeBuffsMaxPower => 120,
            EventKind::BridgeBuffs => 300,
            EventKind::SoulUrnYellow | EventKind::SoulUrnGreen | EventKind::SoulUrn => 300,
            EventKind::BoxRoute => 180,
            _ => 120,
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

    /// Activates or restarts the 180s (3m) box route countdown.
    pub fn start_or_restart(&mut self, match_time: u32) {
        self.active_target = Some(match_time + 180);
    }

    /// Activates or restarts the 180s (3m) box route countdown.
    /// Returns true if started/restarted.
    pub fn toggle(&mut self, match_time: u32) -> bool {
        self.start_or_restart(match_time);
        true
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

        // --- B. Eventos Fijos Cíclicos (cada 300s / 5:00) ---
        // 1. Bridge Buffs: Primer spawn a los 300s (05:00), luego cada 300s
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
        events.push(MacroEvent {
            kind: EventKind::BridgeBuffs,
            target_time: bridge_target + 300,
        });

        // 2. Soul Urn: Primer spawn a los 600s (10:00), luego cada 300s (Amarilla en pares, Verde en impares)
        let urn_target = if current_seconds < 600 {
            600
        } else {
            let cycle_start = (current_seconds / 300) * 300;
            if current_seconds <= cycle_start + 10 {
                cycle_start
            } else {
                cycle_start + 300
            }
        };
        let urn_kind = if (urn_target / 300) % 2 == 0 {
            EventKind::SoulUrnYellow
        } else {
            EventKind::SoulUrnGreen
        };
        events.push(MacroEvent {
            kind: urn_kind,
            target_time: urn_target,
        });
        let next_urn_target = urn_target + 300;
        let next_urn_kind = if (next_urn_target / 300) % 2 == 0 {
            EventKind::SoulUrnYellow
        } else {
            EventKind::SoulUrnGreen
        };
        events.push(MacroEvent {
            kind: next_urn_kind,
            target_time: next_urn_target,
        });

        // --- A. Eventos Fijos Únicos (visibles hasta 10s después de ocurrir) ---
        // 120s (02:00): "Campamentos T1"
        if current_seconds <= 130 {
            events.push(MacroEvent {
                kind: EventKind::T1Camps,
                target_time: 120,
            });
        }

        // 180s (03:00): "Cajas y Estatuas"
        if current_seconds <= 190 {
            events.push(MacroEvent {
                kind: EventKind::BreakableCrates,
                target_time: 180,
            });
        }

        // 300s (05:00): "Campamentos T2"
        if current_seconds <= 310 {
            events.push(MacroEvent {
                kind: EventKind::T2Camps,
                target_time: 300,
            });
        }

        // 360s (06:00): "Guardians caen a 0% Resistencia"
        if current_seconds <= 370 {
            events.push(MacroEvent {
                kind: EventKind::GuardiansZeroResist,
                target_time: 360,
            });
        }

        // 480s (08:00): "Campamentos T3 y Sinner's Sacrifice"
        if current_seconds <= 490 {
            events.push(MacroEvent {
                kind: EventKind::SinnersSacrifice,
                target_time: 480,
            });
        }

        // 540s (09:00): "Walkers caen a 0% Resistencia"
        if current_seconds <= 550 {
            events.push(MacroEvent {
                kind: EventKind::WalkersZeroResist,
                target_time: 540,
            });
        }

        // 600s (10:00): "Mid Boss Disponible y Cajas de Mid"
        if current_seconds <= 610 {
            events.push(MacroEvent {
                kind: EventKind::MidBossAvailable,
                target_time: 600,
            });
        }

        // 660s (11:00): "Ventana 1er Unstable Rift (11:00-13:00)"
        if current_seconds <= 670 {
            events.push(MacroEvent {
                kind: EventKind::UnstableRift,
                target_time: 660,
            });
        }

        // 1080s (18:00): "Walkers reciben un +65% Daño"
        if current_seconds <= 1090 {
            events.push(MacroEvent {
                kind: EventKind::WalkersBonusDamage,
                target_time: 1080,
            });
        }

        // 1200s (20:00): "Oleadas cada 25s"
        if current_seconds <= 1210 {
            events.push(MacroEvent {
                kind: EventKind::Waves25s,
                target_time: 1200,
            });
        }

        // 1800s (30:00): "Estatuas doradas pasan a Tier 2"
        if current_seconds <= 1810 {
            events.push(MacroEvent {
                kind: EventKind::GoldenStatuesT2,
                target_time: 1800,
            });
        }

        // 2100s (35:00): "Oleadas cada 20s y +50% Vida"
        if current_seconds <= 2110 {
            events.push(MacroEvent {
                kind: EventKind::Waves20sAndHealth,
                target_time: 2100,
            });
        }

        // 2400s (40:00): "Bridge Buffs Poder Máximo"
        if current_seconds <= 2410 {
            events.push(MacroEvent {
                kind: EventKind::BridgeBuffsMaxPower,
                target_time: 2400,
            });
        }

        // --- C. Evento Manual con F8 (Ruta de Cajas) ---
        if let Some(target) = self.boxes.active_target {
            if current_seconds <= target + 10 {
                events.push(MacroEvent {
                    kind: EventKind::BoxRoute,
                    target_time: target,
                });
            }
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
            if !self.is_active {
                self.is_active = true;
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
