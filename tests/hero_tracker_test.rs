use deadlock_overlay::hero_tracker::{
    get_all_heroes, map_internal_hero_id, parse_hero_from_log_line, HeroSelectionSource,
    HeroTracker,
};
use std::fs::File;
use std::io::Write;

#[test]
fn test_map_internal_hero_id_known() {
    assert_eq!(map_internal_hero_id("hero_hornet"), "Vindicta");
    assert_eq!(map_internal_hero_id("hero_inferno"), "Infernus");
    assert_eq!(map_internal_hero_id("hero_gigawatt"), "Seven");
    assert_eq!(map_internal_hero_id("hero_atlas"), "Abrams");
    assert_eq!(map_internal_hero_id("hero_werewolf"), "Shiv");
    assert_eq!(map_internal_hero_id("hero_ghost"), "Lady Geist");
}

#[test]
fn test_map_internal_hero_id_unknown() {
    assert_eq!(
        map_internal_hero_id("hero_experimental_guy"),
        "Experimental Guy"
    );
    assert_eq!(map_internal_hero_id("unknown_stranger"), "Unknown Stranger");
}

#[test]
fn test_parse_hero_from_spawn_line() {
    let line = "10/31 23:44:09 Attempting to spawn hero hero_hornet (3). GameState is 7";
    assert_eq!(parse_hero_from_log_line(line), Some("hero_hornet".to_string()));
}

#[test]
fn test_parse_hero_from_loaded_server_line() {
    let line = "10/31 23:44:10 [Server] Loaded hero 2838/hero_hornet";
    assert_eq!(parse_hero_from_log_line(line), Some("hero_hornet".to_string()));
}

#[test]
fn test_parse_hero_from_irrelevant_line() {
    let line = "10/31 23:44:10 C_BasePlayerPawn::OnPostPredictionError() visual prediction distance 0.000";
    assert_eq!(parse_hero_from_log_line(line), None);
}

#[test]
fn test_get_all_heroes_contains_core_roster() {
    let heroes = get_all_heroes();
    assert!(heroes.iter().any(|(id, name)| *id == "hero_hornet" && *name == "Vindicta"));
    assert!(heroes.iter().any(|(id, name)| *id == "hero_inferno" && *name == "Infernus"));
    assert!(heroes.iter().any(|(id, name)| *id == "hero_gigawatt" && *name == "Seven"));
}

#[test]
fn test_tracker_polls_appended_lines() {
    let dir = std::env::temp_dir().join("deadlock_test_log_poll");
    let _ = std::fs::create_dir_all(&dir);
    let log_file = dir.join("console.log");

    {
        let mut f = File::create(&log_file).unwrap();
        writeln!(f, "Log initialization").unwrap();
    }

    let mut tracker = HeroTracker::new(Some(log_file.clone()));
    assert_eq!(tracker.get_hero().display_name, "None");

    // Append spawn line
    {
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&log_file)
            .unwrap();
        writeln!(
            f,
            "10/31 23:44:09 Attempting to spawn hero hero_hornet (3). GameState is 7"
        )
        .unwrap();
    }

    let detected = tracker.poll_log();
    assert_eq!(detected, Some("Vindicta".to_string()));
    let hero = tracker.get_hero();
    assert_eq!(hero.display_name, "Vindicta");
    assert_eq!(hero.source, HeroSelectionSource::Auto);

    let _ = std::fs::remove_file(log_file);
}

#[test]
fn test_tracker_manual_override_persists() {
    let dir = std::env::temp_dir().join("deadlock_test_log_override");
    let _ = std::fs::create_dir_all(&dir);
    let log_file = dir.join("console.log");

    {
        let mut f = File::create(&log_file).unwrap();
        writeln!(f, "Initial line").unwrap();
    }

    let mut tracker = HeroTracker::new(Some(log_file.clone()));
    tracker.set_manual_hero("Seven");

    let hero = tracker.get_hero();
    assert_eq!(hero.display_name, "Seven");
    assert_eq!(hero.source, HeroSelectionSource::Manual);

    // Appending a new hero line should NOT overwrite manual selection
    {
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&log_file)
            .unwrap();
        writeln!(f, "Attempting to spawn hero hero_hornet (3).").unwrap();
    }

    let detected = tracker.poll_log();
    assert_eq!(detected, Some("Vindicta".to_string())); // detected parsed name
    let hero_after = tracker.get_hero();
    assert_eq!(hero_after.display_name, "Seven");
    assert_eq!(hero_after.source, HeroSelectionSource::Manual);

    let _ = std::fs::remove_file(log_file);
}

#[test]
fn test_tracker_reset_to_auto() {
    let dir = std::env::temp_dir().join("deadlock_test_log_reset_auto");
    let _ = std::fs::create_dir_all(&dir);
    let log_file = dir.join("console.log");

    {
        let mut f = File::create(&log_file).unwrap();
        writeln!(f, "Start").unwrap();
    }

    let mut tracker = HeroTracker::new(Some(log_file.clone()));
    tracker.set_manual_hero("Abrams");
    assert_eq!(tracker.get_hero().source, HeroSelectionSource::Manual);

    // Reset back to auto
    tracker.reset_to_auto();
    assert_eq!(tracker.get_hero().source, HeroSelectionSource::Auto);

    // Now write a new hero line
    {
        let mut f = std::fs::OpenOptions::new().append(true).open(&log_file).unwrap();
        writeln!(f, "Attempting to spawn hero hero_hornet (3).").unwrap();
    }

    let detected = tracker.poll_log();
    assert_eq!(detected, Some("Vindicta".to_string()));
    assert_eq!(tracker.get_hero().display_name, "Vindicta");
    assert_eq!(tracker.get_hero().source, HeroSelectionSource::Auto);

    let _ = std::fs::remove_file(log_file);
}

#[test]
fn test_tracker_reads_tail_on_startup_for_existing_log() {
    let dir = std::env::temp_dir().join("deadlock_test_log_startup_tail");
    let _ = std::fs::create_dir_all(&dir);
    let log_file = dir.join("console.log");

    // Game already running and wrote spawn event before overlay was opened
    {
        let mut f = File::create(&log_file).unwrap();
        writeln!(f, "Game started earlier").unwrap();
        writeln!(f, "Attempting to spawn hero hero_gigawatt (3).").unwrap();
    }

    // Overlay starts now
    let mut tracker = HeroTracker::new(Some(log_file.clone()));
    let detected = tracker.poll_log();
    assert_eq!(detected, Some("Seven".to_string()));
    assert_eq!(tracker.get_hero().display_name, "Seven");

    let _ = std::fs::remove_file(log_file);
}

#[test]
fn test_tracker_handles_partial_line_without_advancing_offset() {
    let dir = std::env::temp_dir().join("deadlock_test_log_partial");
    let _ = std::fs::create_dir_all(&dir);
    let log_file = dir.join("console.log");

    {
        let mut f = File::create(&log_file).unwrap();
        writeln!(f, "Line 1").unwrap();
    }

    let mut tracker = HeroTracker::new(Some(log_file.clone()));
    tracker.poll_log();

    // Incomplete write: no newline at end
    {
        let mut f = std::fs::OpenOptions::new().append(true).open(&log_file).unwrap();
        write!(f, "Attempting to spawn hero hero_").unwrap();
        f.flush().unwrap();
    }

    // Should not consume incomplete line
    let detected = tracker.poll_log();
    assert_eq!(detected, None);

    // Finish writing the line
    {
        let mut f = std::fs::OpenOptions::new().append(true).open(&log_file).unwrap();
        writeln!(f, "hornet (3).").unwrap();
    }

    // Next poll should cleanly read full line
    let detected = tracker.poll_log();
    assert_eq!(detected, Some("Vindicta".to_string()));
    assert_eq!(tracker.get_hero().display_name, "Vindicta");

    let _ = std::fs::remove_file(log_file);
}

#[test]
fn test_tracker_handles_file_truncation() {
    let dir = std::env::temp_dir().join("deadlock_test_log_trunc");
    let _ = std::fs::create_dir_all(&dir);
    let log_file = dir.join("console.log");

    {
        let mut f = File::create(&log_file).unwrap();
        for _ in 0..10 {
            writeln!(f, "Initial long line of log text before truncation").unwrap();
        }
    }

    let mut tracker = HeroTracker::new(Some(log_file.clone()));
    tracker.poll_log();

    // Now recreate file shorter (truncation on restart)
    {
        let mut f = File::create(&log_file).unwrap();
        writeln!(f, "Attempting to spawn hero hero_inferno").unwrap();
    }

    let detected = tracker.poll_log();
    assert_eq!(detected, Some("Infernus".to_string()));

    let _ = std::fs::remove_file(log_file);
}
