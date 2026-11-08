use deadlock_overlay::timers::{
    ClockFilter, EventKind, EventStatus, MacroScheduler, MidBossTracker,
};

#[test]
fn test_macro_milestones_progression() {
    let scheduler = MacroScheduler::default();

    // 1. At 60s (01:00), the immediate next event is Cajas & Jungla T1 at 120s (02:00)
    let next = scheduler.get_immediate_next_events(60);
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].kind, EventKind::T1Camps);
    assert_eq!(next[0].target_time, 120);
    assert_eq!(next[0].remaining_seconds(60), 60);

    // 2. At 135s (02:15), the immediate next event is Bridge Buffs at 300s (05:00)
    let next = scheduler.get_immediate_next_events(135);
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].kind, EventKind::BridgeBuffs);
    assert_eq!(next[0].target_time, 300);

    // 3. At 315s (05:15), the immediate next event is Jungla T2 (Mediana) at 420s (07:00)
    let next = scheduler.get_immediate_next_events(315);
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].kind, EventKind::T2Camps);
    assert_eq!(next[0].target_time, 420);

    // 4. At 435s (07:15), the immediate next event is Sinner's & Jungla T3 at 480s (08:00)
    let next = scheduler.get_immediate_next_events(435);
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].kind, EventKind::SinnersSacrifice);
    assert_eq!(next[0].target_time, 480);

    // 5. At 500s (08:20), the immediate next events are at 600s (10:00): Bridge + Urn + MidBoss
    let next = scheduler.get_immediate_next_events(500);
    assert!(next.iter().all(|e| e.target_time == 600));
    assert!(next.iter().any(|e| e.kind == EventKind::BridgeBuffs));
    assert!(next.iter().any(|e| e.kind == EventKind::SoulUrn));
    assert!(next.iter().any(|e| e.kind == EventKind::MidBossSpawn));

    // 6. At 615s (10:15), next events are at 900s (15:00)
    let next = scheduler.get_immediate_next_events(615);
    assert!(next.iter().all(|e| e.target_time == 900));
    assert!(next.iter().any(|e| e.kind == EventKind::BridgeBuffs));
    assert!(next.iter().any(|e| e.kind == EventKind::SoulUrn));
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
