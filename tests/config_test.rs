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


#[test]
fn test_config_keybind_defaults_and_persistence() {
    use deadlock_overlay::config::{display_name_to_key, key_to_display_name};

    let mut test_path = std::env::temp_dir();
    test_path.push("test_deadlock_config_keys.cfg");

    // 1. Check default keys: F8 (0x77), F9 (0x78), F10 (0x79)
    let def = OverlayConfig::default_for_resolution(1920, 1080);
    assert_eq!(def.key_box_timer, 0x77);
    assert_eq!(def.key_midboss, 0x78);
    assert_eq!(def.key_interactive, 0x79);
    assert_eq!(key_to_display_name(def.key_box_timer), "F8");
    assert_eq!(key_to_display_name(def.key_midboss), "F9");

    // 2. Set custom keys: Mouse4 (0x05) and V key (0x56)
    let mut custom = def.clone();
    custom.key_box_timer = 0x05; // Mouse4
    custom.key_midboss = 0x56;   // V key
    custom.save_to_file(&test_path).expect("Failed to save config with custom keys");

    // 3. Load and verify
    let loaded = OverlayConfig::load_from_file(&test_path, 1920, 1080);
    assert_eq!(loaded.key_box_timer, 0x05);
    assert_eq!(loaded.key_midboss, 0x56);
    assert_eq!(key_to_display_name(loaded.key_box_timer), "Mouse4");
    assert_eq!(key_to_display_name(loaded.key_midboss), "V");

    // 4. Test resolver conversions
    assert_eq!(display_name_to_key("Mouse4"), Some(0x05));
    assert_eq!(display_name_to_key("Mouse5"), Some(0x06));
    assert_eq!(display_name_to_key("F8"), Some(0x77));
    assert_eq!(display_name_to_key("V"), Some(0x56));

    let _ = fs::remove_file(test_path);
}

#[test]
fn test_config_extended_settings_defaults_and_persistence() {
    let mut test_path = std::env::temp_dir();
    test_path.push("test_deadlock_config_extended.cfg");

    // 1. Defaults
    let def = OverlayConfig::default_for_resolution(1920, 1080);
    assert!(def.alert_30s_sound);
    assert!((def.hud_scale - 1.0).abs() < 0.001);
    assert!((def.bg_opacity - 0.92).abs() < 0.001);

    // 2. Custom values
    let mut custom = def;
    custom.alert_30s_sound = false;
    custom.hud_scale = 1.15;
    custom.bg_opacity = 0.75;
    custom.save_to_file(&test_path).expect("Failed to save extended config");

    // 3. Load & verify round trip
    let loaded = OverlayConfig::load_from_file(&test_path, 1920, 1080);
    assert!(!loaded.alert_30s_sound);
    assert!((loaded.hud_scale - 1.15).abs() < 0.001);
    assert!((loaded.bg_opacity - 0.75).abs() < 0.001);

    let _ = fs::remove_file(test_path);
}


#[test]
fn test_resolve_config_path_uses_appdata_when_present() {
    use std::ffi::OsStr;
    let fake_appdata = OsStr::new(r"C:\Users\TestUser\AppData\Roaming");
    let path = OverlayConfig::resolve_config_path_from_env(Some(fake_appdata));
    assert_eq!(
        path,
        std::path::PathBuf::from(r"C:\Users\TestUser\AppData\Roaming\DeadlockMacroOverlay\deadlock_overlay.cfg")
    );
}

#[test]
fn test_resolve_config_path_falls_back_when_appdata_missing() {
    let path = OverlayConfig::resolve_config_path_from_env(None);
    assert_eq!(path, std::path::PathBuf::from("deadlock_overlay.cfg"));
}

#[test]
fn test_load_or_create_generates_file_in_nested_directory_on_first_run() {
    let temp_dir = std::env::temp_dir().join("dtmo_test_first_run");
    let _ = std::fs::remove_dir_all(&temp_dir);
    let cfg_path = temp_dir.join("DeadlockMacroOverlay").join("deadlock_overlay.cfg");

    assert!(!cfg_path.exists());
    let cfg = OverlayConfig::load_or_create(&cfg_path, 2560, 1440);
    assert!(cfg_path.exists(), "Config file must be auto-created on first launch");
    assert_eq!(cfg.win_x, 2560 - 300);
    assert_eq!(cfg.roi_x, (2560 / 2) - 60);

    let _ = std::fs::remove_dir_all(&temp_dir);
}
