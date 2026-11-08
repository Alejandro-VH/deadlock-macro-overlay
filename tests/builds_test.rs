use deadlock_overlay::builds::{get_hero_build, ItemStage};

#[test]
fn test_get_build_for_known_hero() {
    let abrams_build = get_hero_build("Abrams");
    assert_eq!(abrams_build.hero_name, "Abrams");
    assert!(!abrams_build.get_items(ItemStage::Early).is_empty());
    assert!(!abrams_build.get_items(ItemStage::Core).is_empty());
    assert!(!abrams_build.get_items(ItemStage::Late).is_empty());
    assert!(!abrams_build.counters.is_empty());

    // Check specific iconic Abrams core items
    let core_items = abrams_build.get_items(ItemStage::Core);
    assert!(
        core_items.iter().any(|item| item.contains("Melee") || item.contains("Torment") || item.contains("Heal")),
        "Abrams should recommend melee or sustain items in core"
    );
}

#[test]
fn test_get_build_for_unknown_hero_returns_generic_fallback() {
    let fallback = get_hero_build("UnknownHeroX");
    assert_eq!(fallback.hero_name, "General / Adaptable");
    assert!(!fallback.get_items(ItemStage::Early).is_empty());
    assert!(!fallback.get_items(ItemStage::Core).is_empty());
    assert!(!fallback.counters.is_empty());
}

#[test]
fn test_situational_counters_cover_standard_threats() {
    let build = get_hero_build("Seven");
    let has_anti_heal = build.counters.iter().any(|c| c.threat.to_lowercase().contains("heal") || c.counter.to_lowercase().contains("decay") || c.counter.to_lowercase().contains("toxic"));
    let has_defensive = build.counters.iter().any(|c| c.threat.to_lowercase().contains("spirit") || c.threat.to_lowercase().contains("bullet") || c.counter.to_lowercase().contains("barrier") || c.counter.to_lowercase().contains("armor"));

    assert!(has_anti_heal, "Should include anti-healing counter");
    assert!(has_defensive, "Should include defensive situational counter");
}

#[test]
fn test_roster_coverage_for_builds() {
    let sample_heroes = ["Haze", "Infernus", "Vindicta", "Ivy", "Grey Talon", "Yamato", "Lady Geist"];
    for hero in sample_heroes {
        let build = get_hero_build(hero);
        assert_eq!(build.hero_name, hero, "Hero {} should have a dedicated build", hero);
        assert!(!build.get_items(ItemStage::Core).is_empty());
    }
}
