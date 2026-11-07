use deadlock_overlay::timers::{
    ClockFilter, EventKind, EventStatus, MacroScheduler, MidBossTracker,
};

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

    // Before 10m (e.g. 570s), Urn is pending at 600s
    let events = scheduler.calculate_events(570);
    let urn = events.iter().find(|e| e.kind == EventKind::SoulUrn).expect("Soul Urn not found");
    assert_eq!(urn.target_time, 600);
    assert_eq!(urn.remaining_seconds(570), 30);
    assert_eq!(urn.status(570), EventStatus::Warning);

    // At 10m00s (600s), Urn is Active
    let events = scheduler.calculate_events(600);
    let urn = events.iter().find(|e| e.kind == EventKind::SoulUrn).expect("Soul Urn not found");
    assert_eq!(urn.status(600), EventStatus::Active);

    // At 10m15s (615s), Next Urn is at 15m00s (900s)
    let events = scheduler.calculate_events(615);
    let urn = events.iter().find(|e| e.kind == EventKind::SoulUrn).expect("Soul Urn not found");
    assert_eq!(urn.target_time, 900);
}

#[test]
fn test_sinners_sacrifice_schedule() {
    let scheduler = MacroScheduler::default();

    // At 480s (08:00), Sinner's Sacrifice is Active
    let events = scheduler.calculate_events(480);
    let sinner = events.iter().find(|e| e.kind == EventKind::SinnersSacrifice).expect("Sinner's Sacrifice not found");
    assert_eq!(sinner.status(480), EventStatus::Active);

    // At 489s, it is still active (within 10s grace period)
    let events = scheduler.calculate_events(489);
    assert!(events.iter().any(|e| e.kind == EventKind::SinnersSacrifice));

    // At 491s, it should no longer be present
    let events = scheduler.calculate_events(491);
    assert!(events.iter().all(|e| e.kind != EventKind::SinnersSacrifice));
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
fn test_simultaneous_events_at_ten_minutes() {
    let scheduler = MacroScheduler::default();
    let events = scheduler.calculate_events(600);

    let active_events: Vec<_> = events.iter().filter(|e| e.status(600) == EventStatus::Active).collect();
    assert!(active_events.iter().any(|e| e.kind == EventKind::BridgeBuffs));
    assert!(active_events.iter().any(|e| e.kind == EventKind::SoulUrn));
}

#[test]
fn test_clock_filter_noise_and_drift() {
    let mut filter = ClockFilter::new();
    assert_eq!(filter.current_seconds(), 0);

    // Initial detection of 01:00 (60s)
    filter.feed_ocr_reading(Some(60));
    assert_eq!(filter.current_seconds(), 60);

    // Interpolation (tick without OCR)
    filter.tick_second();
    assert_eq!(filter.current_seconds(), 61);

    // Single outlier OCR reading (e.g. noise reads 01:30 = 90s, difference > 3s)
    // Should be rejected until confirmed
    filter.feed_ocr_reading(Some(90));
    assert_eq!(filter.current_seconds(), 62); // Kept interpolating

    // Second consecutive read confirms 90s -> synchronizes to 90s
    filter.feed_ocr_reading(Some(90));
    assert_eq!(filter.current_seconds(), 90);

    // Reset clears everything
    filter.reset();
    assert_eq!(filter.current_seconds(), 0);
}
