use deadlock_overlay::config::OverlayConfig;
use std::fs;

#[test]
fn test_default_config_for_resolution() {
    let cfg = OverlayConfig::default_for_resolution(1920, 1080);
    assert_eq!(cfg.win_x, 1920 - 300);
    assert_eq!(cfg.win_y, 100);
    assert_eq!(cfg.roi_x, 900); // 1920/2 - 60
    assert_eq!(cfg.roi_y, 8);
    assert_eq!(cfg.roi_w, 120);
    assert_eq!(cfg.roi_h, 42);
}

#[test]
fn test_save_and_load_round_trip() {
    let mut test_path = std::env::temp_dir();
    test_path.push("test_deadlock_config.cfg");
    let mut original = OverlayConfig::default_for_resolution(2560, 1440);
    original.win_x = 2100;
    original.win_y = 150;
    original.roi_x = 1220;
    original.roi_y = 12;

    original.save_to_file(&test_path).expect("Failed to save config");

    let loaded = OverlayConfig::load_from_file(&test_path, 1920, 1080);
    assert_eq!(loaded.win_x, 2100);
    assert_eq!(loaded.win_y, 150);
    assert_eq!(loaded.roi_x, 1220);
    assert_eq!(loaded.roi_y, 12);
    assert_eq!(loaded.roi_w, 120);
    assert_eq!(loaded.roi_h, 42);

    let _ = fs::remove_file(test_path);
}

#[test]
fn test_save_and_load_hero_persistence() {
    let mut test_path = std::env::temp_dir();
    test_path.push("test_deadlock_config_hero.cfg");
    let mut original = OverlayConfig::default_for_resolution(1920, 1080);
    original.last_hero = "Vindicta".to_string();

    original.save_to_file(&test_path).expect("Failed to save config");

    let loaded = OverlayConfig::load_from_file(&test_path, 1920, 1080);
    assert_eq!(loaded.last_hero, "Vindicta");

    let _ = fs::remove_file(test_path);
}

#[test]
fn test_save_and_load_mute_persistence() {
    let mut test_path = std::env::temp_dir();
    test_path.push("test_deadlock_config_mute.cfg");
    let mut original = OverlayConfig::default_for_resolution(1920, 1080);
    original.is_muted = true;

    original.save_to_file(&test_path).expect("Failed to save config");

    let loaded = OverlayConfig::load_from_file(&test_path, 1920, 1080);
    assert!(loaded.is_muted);

    let _ = fs::remove_file(test_path);
}

#[test]
fn test_load_fallback_on_missing_file() {
    let non_existent = "non_existent_config.cfg";
    let loaded = OverlayConfig::load_from_file(non_existent, 1920, 1080);
    let expected = OverlayConfig::default_for_resolution(1920, 1080);
    assert_eq!(loaded.win_x, expected.win_x);
    assert_eq!(loaded.roi_x, expected.roi_x);
}
