use deadlock_overlay::timers::{
    BoxRouteTracker, ClockFilter, EventKind, EventStatus, MacroEvent, MacroScheduler,
    MidBossTracker,
};

#[test]
fn test_event_progress_calculation() {
    // 1. T1Camps (target: 120s, span: 120s)
    let t1_event = MacroEvent {
        kind: EventKind::T1Camps,
        target_time: 120,
    };
    assert_eq!(t1_event.progress(0), 0.0);
    assert_eq!(t1_event.progress(60), 0.5);
    assert_eq!(t1_event.progress(120), 1.0);

    // 2. BridgeBuffs (target: 300s, span: 300s)
    let bridge_event = MacroEvent {
        kind: EventKind::BridgeBuffs,
        target_time: 300,
    };
    assert_eq!(bridge_event.progress(0), 0.0);
    assert_eq!(bridge_event.progress(150), 0.5);
    assert_eq!(bridge_event.progress(300), 1.0);

    // 3. Warning boundary (at 270s, progress is 0.9 / 90%)
    assert!((bridge_event.progress(270) - 0.9).abs() < 0.01);
}

#[test]
fn test_macro_milestones_progression() {
    let scheduler = MacroScheduler::default();

    // 1. At 60s (01:00), the immediate next event is Jungla T1 at 120s (02:00)
    let next = scheduler.get_immediate_next_events(60);
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].kind, EventKind::T1Camps);
    assert_eq!(next[0].target_time, 120);
    assert_eq!(next[0].remaining_seconds(60), 60);

    // 2. At 135s (02:15), the immediate next event is Cajas at 180s (03:00)
    let next = scheduler.get_immediate_next_events(135);
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].kind, EventKind::BreakableCrates);
    assert_eq!(next[0].target_time, 180);

    // 3. At 200s (03:20), the immediate next events are at 300s (05:00): Bridge Buffs AND Campamentos T2
    let next = scheduler.get_immediate_next_events(200);
    assert_eq!(next.len(), 2);
    assert!(next.iter().any(|e| e.kind == EventKind::BridgeBuffs));
    assert!(next.iter().any(|e| e.kind == EventKind::T2Camps));
    assert_eq!(next[0].target_time, 300);

    // 4. At 315s (05:15), the immediate next event is Guardians at 360s (06:00)
    let next = scheduler.get_immediate_next_events(315);
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].kind, EventKind::GuardiansZeroResist);
    assert_eq!(next[0].target_time, 360);

    // 5. At 400s (06:40), next is Campamentos T3 & Sinner's Sacrifice at 480s (08:00)
    let next = scheduler.get_immediate_next_events(400);
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].kind, EventKind::SinnersSacrifice);
    assert_eq!(next[0].target_time, 480);

    // 6. At 500s (08:20), next is Walkers Weak at 540s (09:00)
    let next = scheduler.get_immediate_next_events(500);
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].kind, EventKind::WalkersZeroResist);
    assert_eq!(next[0].target_time, 540);

    // 7. At 555s (09:15), next events are at 600s (10:00): Bridge + Urn Yellow + MidBoss
    let next = scheduler.get_immediate_next_events(555);
    assert!(next.iter().all(|e| e.target_time == 600));
    assert!(next.iter().any(|e| e.kind == EventKind::BridgeBuffs));
    assert!(next.iter().any(|e| e.kind == EventKind::SoulUrnYellow));
    assert!(next.iter().any(|e| e.kind == EventKind::MidBossAvailable));

    // 8. At 615s (10:15), next is Unstable Rift at 660s (11:00)
    let next = scheduler.get_immediate_next_events(615);
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].kind, EventKind::UnstableRift);
    assert_eq!(next[0].target_time, 660);

    // 9. At 700s (11:40), next is 900s (15:00): Bridge + Urn Green
    let next = scheduler.get_immediate_next_events(700);
    assert!(next.iter().all(|e| e.target_time == 900));
    assert!(next.iter().any(|e| e.kind == EventKind::BridgeBuffs));
    assert!(next.iter().any(|e| e.kind == EventKind::SoulUrnGreen));
}

#[test]
fn test_bridge_buffs_schedule() {
    let scheduler = MacroScheduler::default();

    // At 4m30s (270s), Bridge Buffs should be pending in 30s
    let events = scheduler.calculate_events(270);
    let bridge = events.iter().find(|e| e.kind == EventKind::BridgeBuffs).expect("Bridge Buffs not found");
    assert_eq!(bridge.target_time, 300);
    assert_eq!(bridge.remaining_seconds(270), 30);
    assert_eq!(bridge.status(270), EventStatus::Warning);

    // At 5m00s (300s), Bridge Buffs should be Active
    let events = scheduler.calculate_events(300);
    let bridge = events.iter().find(|e| e.kind == EventKind::BridgeBuffs).expect("Bridge Buffs not found");
    assert_eq!(bridge.status(300), EventStatus::Active);

    // At 5m11s (311s), Next Bridge Buffs should be at 10m00s (600s)
    let events = scheduler.calculate_events(311);
    let bridge = events.iter().find(|e| e.kind == EventKind::BridgeBuffs).expect("Bridge Buffs not found");
    assert_eq!(bridge.target_time, 600);
    assert_eq!(bridge.remaining_seconds(311), 289);
}

#[test]
fn test_soul_urn_schedule() {
    let scheduler = MacroScheduler::default();

    // Before 10m (e.g. 570s), Urn is pending at 600s (Yellow)
    let events = scheduler.calculate_events(570);
    let urn = events.iter().find(|e| e.kind == EventKind::SoulUrnYellow).expect("Soul Urn Yellow not found");
    assert_eq!(urn.target_time, 600);
    assert_eq!(urn.remaining_seconds(570), 30);
    assert_eq!(urn.status(570), EventStatus::Warning);

    // At 10m00s (600s), Urn is Active
    let events = scheduler.calculate_events(600);
    let urn = events.iter().find(|e| e.kind == EventKind::SoulUrnYellow).expect("Soul Urn Yellow not found");
    assert_eq!(urn.status(600), EventStatus::Active);

    // At 10m15s (615s), Next Urn is at 15m00s (900s) (Green)
    let events = scheduler.calculate_events(615);
    let urn = events.iter().find(|e| e.kind == EventKind::SoulUrnGreen).expect("Soul Urn Green not found");
    assert_eq!(urn.target_time, 900);
}

#[test]
fn test_mid_boss_tracker_progression_and_cancel() {
    let mut tracker = MidBossTracker::default();
    assert_eq!(tracker.get_active_respawn(500), None);

    // 1st Kill at 720s (12m) -> 7m (420s) respawn -> 1140s
    tracker.record_kill(720);
    assert_eq!(tracker.kills, 1);
    assert_eq!(tracker.get_active_respawn(720), Some(1140));

    // Cancel erroneous kill
    tracker.cancel_kill();
    assert_eq!(tracker.kills, 0);
    assert_eq!(tracker.get_active_respawn(720), None);

    // 1st Kill again at 720s
    tracker.record_kill(720);
    assert_eq!(tracker.get_active_respawn(720), Some(1140));

    // Check expiration: at 1145s (active window: 1140..=1150)
    assert_eq!(tracker.get_active_respawn(1145), Some(1140));

    // At 1151s (grace period expired), should return None
    assert_eq!(tracker.get_active_respawn(1151), None);

    // 2nd Kill at 1200s (20m) -> 6m (360s) respawn -> 1560s
    tracker.record_kill(1200);
    assert_eq!(tracker.kills, 2);
    assert_eq!(tracker.get_active_respawn(1200), Some(1560));

    // 3rd Kill at 1800s (30m) -> 5m (300s) respawn -> 2100s
    tracker.record_kill(1800);
    assert_eq!(tracker.kills, 3);
    assert_eq!(tracker.get_active_respawn(1800), Some(2100));

    // 4th Kill at 2400s (40m) -> 5m (300s) respawn -> 2700s
    tracker.record_kill(2400);
    assert_eq!(tracker.kills, 4);
    assert_eq!(tracker.get_active_respawn(2400), Some(2700));

    // Reset
    tracker.reset();
    assert_eq!(tracker.kills, 0);
    assert_eq!(tracker.get_active_respawn(2400), None);
}

#[test]
fn test_clock_filter_noise_and_drift() {
    let mut filter = ClockFilter::new();
    assert_eq!(filter.current_seconds(), 0);

    // Initial detection of 01:00 (60s)
    let updated = filter.feed_ocr_reading(Some(60));
    assert!(updated);
    assert_eq!(filter.current_seconds(), 60);

    // Interpolation (tick without OCR)
    filter.tick_second();
    assert_eq!(filter.current_seconds(), 61);

    // Single outlier OCR reading (e.g. noise reads 01:30 = 90s, difference > 3s)
    let updated = filter.feed_ocr_reading(Some(90));
    assert!(!updated);
    assert_eq!(filter.current_seconds(), 61);

    // Second consecutive read confirms 90s -> synchronizes to 90s
    let updated = filter.feed_ocr_reading(Some(90));
    assert!(updated);
    assert_eq!(filter.current_seconds(), 90);

    // Adjust seconds test
    filter.adjust_seconds(5);
    assert_eq!(filter.current_seconds(), 95);
    filter.adjust_seconds(-10);
    assert_eq!(filter.current_seconds(), 85);

    // Reset clears everything
    filter.reset();
    assert_eq!(filter.current_seconds(), 0);
}

#[test]
fn test_clock_filter_ignores_duplicate_second_resets() {
    let mut filter = ClockFilter::new();

    // First reading initializes the clock -> returns true
    assert!(filter.feed_ocr_reading(Some(252))); // 04:12
    assert_eq!(filter.current_seconds(), 252);

    // Duplicate reading with same second (e.g. 400ms later during same second) -> returns false
    assert!(!filter.feed_ocr_reading(Some(252)));
    assert_eq!(filter.current_seconds(), 252);

    // Second changes to 253 -> returns true
    assert!(filter.feed_ocr_reading(Some(253)));
    assert_eq!(filter.current_seconds(), 253);

    // Duplicate 253 -> returns false
    assert!(!filter.feed_ocr_reading(Some(253)));
}

#[test]
fn test_milestone_warning_status_at_30s_boundary() {
    let event = MacroEvent {
        kind: EventKind::BridgeBuffs,
        target_time: 300,
    };
    // At 269s (31s remaining), still upcoming
    assert_eq!(event.status(269), EventStatus::Upcoming);
    // At 270s (30s remaining), enters Warning status
    assert_eq!(event.status(270), EventStatus::Warning);
    // At 299s (1s remaining), remains in Warning status
    assert_eq!(event.status(299), EventStatus::Warning);
    // At 300s (0s remaining), enters Active status
    assert_eq!(event.status(300), EventStatus::Active);
}

#[test]
fn test_box_route_tracker_toggle() {
    let mut tracker = BoxRouteTracker::new();
    assert_eq!(tracker.get_remaining(300), None);

    // 1. First toggle starts 180s (3m) timer
    let started = tracker.toggle(300);
    assert!(started);
    assert_eq!(tracker.get_remaining(300), Some(180));
    assert_eq!(tracker.get_remaining(360), Some(120));
    assert_eq!(tracker.get_remaining(479), Some(1));
    assert_eq!(tracker.get_remaining(480), Some(0));
    // After 10s grace period past 0, expires
    assert_eq!(tracker.get_remaining(491), None);

    // 2. Second toggle while active restarts the 180s countdown from current match time
    let active_after_toggle = tracker.toggle(400);
    assert!(active_after_toggle);
    assert_eq!(tracker.get_remaining(400), Some(180));
    assert_eq!(tracker.get_remaining(450), Some(130));
}

#[test]
fn test_corrected_cajas_and_jungle_spawns() {
    let scheduler = MacroScheduler::default();

    // At 60s (01:00), immediate next event is Jungla T1 at 120s (02:00)
    let next = scheduler.get_immediate_next_events(60);
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].kind, EventKind::T1Camps);
    assert_eq!(next[0].target_time, 120);

    // At 135s (02:15), immediate next event is Cajas & Breakables at 180s (03:00)
    let next = scheduler.get_immediate_next_events(135);
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].kind, EventKind::BreakableCrates);
    assert_eq!(next[0].target_time, 180);

    // At 200s (03:20), immediate next events are Bridge Buffs & Campamentos T2 at 300s (05:00)
    let next = scheduler.get_immediate_next_events(200);
    assert_eq!(next.len(), 2);
    assert!(next.iter().any(|e| e.kind == EventKind::BridgeBuffs));
    assert!(next.iter().any(|e| e.kind == EventKind::T2Camps));
    assert_eq!(next[0].target_time, 300);
}

#[test]
fn test_dual_horizon_events() {
    let scheduler = MacroScheduler::default();

    // At 285s (04:45), immediate are Bridge Buffs & Campamentos T2 at 300s (05:00)
    // and subsequent milestone is Guardians at 360s (06:00)
    let (immediate, next) = scheduler.get_immediate_and_next_events(285);
    assert_eq!(immediate.len(), 2);
    assert!(immediate.iter().any(|e| e.kind == EventKind::BridgeBuffs));
    assert!(immediate.iter().any(|e| e.kind == EventKind::T2Camps));
    assert_eq!(immediate[0].target_time, 300);

    assert!(next.is_some());
    let next_event = next.unwrap();
    assert_eq!(next_event.kind, EventKind::GuardiansZeroResist);
    assert_eq!(next_event.target_time, 360);
}

#[test]
fn test_grouped_horizons_simultaneous_events() {
    let scheduler = MacroScheduler::default();

    // At 579s (09:39), 21s before 10:00 (600s)
    let (immediate, next) = scheduler.get_grouped_horizons(579);

    assert!(immediate.is_some(), "Immediate group must exist at 09:39");
    let imm = immediate.unwrap();
    assert_eq!(imm.target_time, 600);
    assert_eq!(imm.remaining_seconds, 21);
    assert_eq!(imm.status, EventStatus::Warning);
    // At 10:00 we have Bridge Buffs, Soul Urn Yellow, and Mid Boss Spawn
    assert_eq!(imm.events.len(), 3);
    assert!(imm.events.iter().any(|e| e.kind == EventKind::BridgeBuffs));
    assert!(imm.events.iter().any(|e| e.kind == EventKind::SoulUrnYellow));
    assert!(imm.events.iter().any(|e| e.kind == EventKind::MidBossAvailable));

    // Next milestone is at 11:00 (660s) with Unstable Rift
    assert!(next.is_some(), "Next group must exist");
    let nxt = next.unwrap();
    assert_eq!(nxt.target_time, 660);
    assert_eq!(nxt.remaining_seconds, 81);
    assert_eq!(nxt.events.len(), 1);
    assert_eq!(nxt.events[0].kind, EventKind::UnstableRift);
}

#[test]
fn test_midboss_respawn_excluded_from_global_schedule() {
    let mut scheduler = MacroScheduler::default();
    scheduler.midboss.record_kill(650); // Mid Boss killed at 10:50, respawns at 1070 (17:50)

    let (immediate, next) = scheduler.get_grouped_horizons(700);
    if let Some(imm) = immediate {
        assert!(!imm.events.iter().any(|e| e.kind == EventKind::MidBossRespawn));
    }
    if let Some(nxt) = next {
        assert!(!nxt.events.iter().any(|e| e.kind == EventKind::MidBossRespawn));
    }
}
