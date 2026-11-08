#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemStage {
    Early,
    Core,
    Late,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SituationalCounter {
    pub threat: &'static str,
    pub counter: &'static str,
    pub notes: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeroBuild {
    pub hero_name: &'static str,
    pub early_items: &'static [&'static str],
    pub core_items: &'static [&'static str],
    pub late_items: &'static [&'static str],
    pub counters: &'static [SituationalCounter],
}

impl HeroBuild {
    pub fn get_items(&self, stage: ItemStage) -> &'static [&'static str] {
        match stage {
            ItemStage::Early => self.early_items,
            ItemStage::Core => self.core_items,
            ItemStage::Late => self.late_items,
        }
    }
}

const COMMON_COUNTERS: [SituationalCounter; 4] = [
    SituationalCounter {
        threat: "Regen / Healing (Abrams, Infernus, Geist)",
        counter: "Healbane / Decay / Toxic Bullets",
        notes: "Cuts target healing by 40-50%",
    },
    SituationalCounter {
        threat: "Spirit Burst (Seven, Bebop, Vindicta)",
        counter: "Spirit Armor / Reactive Barrier",
        notes: "Absorbs burst spells & lowers magic damage",
    },
    SituationalCounter {
        threat: "Bullet DPS (Haze, Wraith, Ivy)",
        counter: "Metal Skin / Return Fire / Bullet Armor",
        notes: "Immunity or reflection against rapid gunfire",
    },
    SituationalCounter {
        threat: "Channeling / Mobility (Seven, Dynamo, Pocket)",
        counter: "Knockdown / Curse / Silence Glyph",
        notes: "Interrupts big ultimates and silences abilities",
    },
];

const GENERIC_BUILD: HeroBuild = HeroBuild {
    hero_name: "General / Adaptable",
    early_items: &["Extra Regen", "Sprint Boots", "Mystic Burst", "High-Velocity Mag"],
    core_items: &["Warp Stone", "Reactive Barrier", "Enduring Speed", "Torment Pulse"],
    late_items: &["Leech", "Boundless Spirit", "Lucky Shot", "Unstoppable"],
    counters: &COMMON_COUNTERS,
};

pub fn get_hero_build(hero_name: &str) -> HeroBuild {
    let normalized = hero_name.trim().to_lowercase();
    match normalized.as_str() {
        "abrams" => HeroBuild {
            hero_name: "Abrams",
            early_items: &["Extra Regen", "Restorative Shot", "Melee Charge", "Extra Stamina"],
            core_items: &["Torment Pulse", "Lifestrike", "Healbane", "Warp Stone"],
            late_items: &["Colossus", "Phantom Strike", "Leech", "Unstoppable"],
            counters: &COMMON_COUNTERS,
        },
        "bebop" => HeroBuild {
            hero_name: "Bebop",
            early_items: &["Mystic Burst", "Extra Spirit", "Infuser", "Sprint Boots"],
            core_items: &["Echo Shard", "Superior Cooldown", "Warp Stone", "Majestic Leap"],
            late_items: &["Boundless Spirit", "Diviner's Kevlar", "Refresher", "Curse"],
            counters: &COMMON_COUNTERS,
        },
        "dynamo" => HeroBuild {
            hero_name: "Dynamo",
            early_items: &["Extra Stamina", "Enduring Spirit", "Restorative Shot", "Sprint Boots"],
            core_items: &["Warp Stone", "Quantum Entanglement", "Majestic Leap", "Superior Cooldown"],
            late_items: &["Refresher", "Diviner's Kevlar", "Curse", "Colossus"],
            counters: &COMMON_COUNTERS,
        },
        "grey talon" | "greytalon" => HeroBuild {
            hero_name: "Grey Talon",
            early_items: &["High-Velocity Mag", "Extra Spirit", "Mystic Burst", "Sprint Boots"],
            core_items: &["Pristine Emblem", "Surge of Power", "Heroic Aura", "Enduring Speed"],
            late_items: &["Ricochet", "Lucky Shot", "Boundless Spirit", "Glass Cannon"],
            counters: &COMMON_COUNTERS,
        },
        "haze" => HeroBuild {
            hero_name: "Haze",
            early_items: &["Basic Magazine", "Restorative Shot", "Swift Striker", "Sprint Boots"],
            core_items: &["Tesla Bullets", "Quicksilver Reload", "Bullet Resist Shredder", "Warp Stone"],
            late_items: &["Ricochet", "Lucky Shot", "Silencer", "Unstoppable"],
            counters: &COMMON_COUNTERS,
        },
        "infernus" => HeroBuild {
            hero_name: "Infernus",
            early_items: &["Toxic Bullets", "Enduring Speed", "Extra Spirit", "Restorative Shot"],
            core_items: &["Ricochet", "Mystic Slow", "Spirit Vulnerability", "Torment Pulse"],
            late_items: &["Leech", "Crippling Headshot", "Boundless Spirit", "Slowing Hex"],
            counters: &COMMON_COUNTERS,
        },
        "ivy" => HeroBuild {
            hero_name: "Ivy",
            early_items: &["Rapid Rounds", "Enduring Speed", "Mystic Burst", "Extra Stamina"],
            core_items: &["Tesla Bullets", "Heroic Aura", "Alchemical Fire", "Warp Stone"],
            late_items: &["Ricochet", "Boundless Spirit", "Colossus", "Diviner's Kevlar"],
            counters: &COMMON_COUNTERS,
        },
        "kelvin" => HeroBuild {
            hero_name: "Kelvin",
            early_items: &["Extra Spirit", "Enduring Speed", "Mystic Burst", "Restorative Shot"],
            core_items: &["Arctic Beam", "Majestic Leap", "Decay", "Superior Duration"],
            late_items: &["Boundless Spirit", "Diviner's Kevlar", "Curse", "Colossus"],
            counters: &COMMON_COUNTERS,
        },
        "lady geist" | "ladygeist" => HeroBuild {
            hero_name: "Lady Geist",
            early_items: &["Extra Spirit", "Restorative Shot", "Mystic Burst", "Extra Stamina"],
            core_items: &["Torment Pulse", "Soul Rebirth", "Decay", "Superior Cooldown"],
            late_items: &["Boundless Spirit", "Leech", "Diviner's Kevlar", "Curse"],
            counters: &COMMON_COUNTERS,
        },
        "lash" => HeroBuild {
            hero_name: "Lash",
            early_items: &["Mystic Burst", "Extra Stamina", "Enduring Speed", "Restorative Shot"],
            core_items: &["Warp Stone", "Improved Burst", "Majestic Leap", "Torment Pulse"],
            late_items: &["Boundless Spirit", "Escalating Exposure", "Refresher", "Leech"],
            counters: &COMMON_COUNTERS,
        },
        "mcginnis" => HeroBuild {
            hero_name: "McGinnis",
            early_items: &["High-Velocity Mag", "Extra Spirit", "Restorative Shot", "Sprint Boots"],
            core_items: &["Heroic Aura", "Titanic Magazine", "Echo Shard", "Bullet Resist Shredder"],
            late_items: &["Ricochet", "Boundless Spirit", "Spiritual Overflow", "Lucky Shot"],
            counters: &COMMON_COUNTERS,
        },
        "mirage" => HeroBuild {
            hero_name: "Mirage",
            early_items: &["High-Velocity Mag", "Extra Stamina", "Mystic Burst", "Sprint Boots"],
            core_items: &["Kinetic Dash", "Pristine Emblem", "Warp Stone", "Surge of Power"],
            late_items: &["Lucky Shot", "Ricochet", "Phantom Strike", "Glass Cannon"],
            counters: &COMMON_COUNTERS,
        },
        "mo & krill" | "mo and krill" => HeroBuild {
            hero_name: "Mo & Krill",
            early_items: &["Melee Charge", "Enduring Speed", "Extra Spirit", "Torment Pulse"],
            core_items: &["Torment Pulse", "Phantom Strike", "Majestic Leap", "Healbane"],
            late_items: &["Colossus", "Diviner's Kevlar", "Curse", "Leech"],
            counters: &COMMON_COUNTERS,
        },
        "paradox" => HeroBuild {
            hero_name: "Paradox",
            early_items: &["High-Velocity Mag", "Mystic Burst", "Extra Spirit", "Sprint Boots"],
            core_items: &["Warp Stone", "Superior Cooldown", "Kinetic Dash", "Surge of Power"],
            late_items: &["Silence Glyph", "Curse", "Boundless Spirit", "Diviner's Kevlar"],
            counters: &COMMON_COUNTERS,
        },
        "pocket" => HeroBuild {
            hero_name: "Pocket",
            early_items: &["Mystic Burst", "Enduring Speed", "Extra Stamina", "Restorative Shot"],
            core_items: &["Majestic Leap", "Torment Pulse", "Warp Stone", "Superior Cooldown"],
            late_items: &["Escalating Exposure", "Leech", "Boundless Spirit", "Curse"],
            counters: &COMMON_COUNTERS,
        },
        "seven" => HeroBuild {
            hero_name: "Seven",
            early_items: &["High-Velocity Mag", "Mystic Burst", "Enduring Speed", "Extra Spirit"],
            core_items: &["Tesla Bullets", "Surge of Power", "Superior Duration", "Warp Stone"],
            late_items: &["Boundless Spirit", "Curse", "Ricochet", "Diviner's Kevlar"],
            counters: &COMMON_COUNTERS,
        },
        "shiv" => HeroBuild {
            hero_name: "Shiv",
            early_items: &["Restorative Shot", "Extra Stamina", "Enduring Speed", "Melee Charge"],
            core_items: &["Decay", "Torment Pulse", "Healbane", "Warp Stone"],
            late_items: &["Leech", "Phantom Strike", "Blood Tribute", "Colossus"],
            counters: &COMMON_COUNTERS,
        },
        "vindicta" => HeroBuild {
            hero_name: "Vindicta",
            early_items: &["High-Velocity Mag", "Basic Magazine", "Extra Spirit", "Sprint Boots"],
            core_items: &["Pristine Emblem", "Long Range", "Warp Stone", "Headshot Booster"],
            late_items: &["Lucky Shot", "Ricochet", "Glass Cannon", "Boundless Spirit"],
            counters: &COMMON_COUNTERS,
        },
        "viscous" => HeroBuild {
            hero_name: "Viscous",
            early_items: &["Mystic Burst", "Extra Spirit", "Enduring Speed", "Restorative Shot"],
            core_items: &["Restorative Locket", "Majestic Leap", "Superior Cooldown", "Torment Pulse"],
            late_items: &["Boundless Spirit", "Diviner's Kevlar", "Phantom Strike", "Curse"],
            counters: &COMMON_COUNTERS,
        },
        "warden" => HeroBuild {
            hero_name: "Warden",
            early_items: &["High-Velocity Mag", "Enduring Speed", "Mystic Burst", "Restorative Shot"],
            core_items: &["Alchemical Fire", "Fleetfoot", "Bullet Resist Shredder", "Healbane"],
            late_items: &["Colossus", "Crippling Headshot", "Leech", "Unstoppable"],
            counters: &COMMON_COUNTERS,
        },
        "wraith" => HeroBuild {
            hero_name: "Wraith",
            early_items: &["Rapid Rounds", "Basic Magazine", "Mystic Burst", "Sprint Boots"],
            core_items: &["Tesla Bullets", "Quicksilver Reload", "Warp Stone", "Bullet Resist Shredder"],
            late_items: &["Lucky Shot", "Ricochet", "Silencer", "Unstoppable"],
            counters: &COMMON_COUNTERS,
        },
        "yamato" => HeroBuild {
            hero_name: "Yamato",
            early_items: &["Mystic Burst", "Restorative Shot", "Extra Stamina", "Sprint Boots"],
            core_items: &["Torment Pulse", "Warp Stone", "Superior Cooldown", "Improved Burst"],
            late_items: &["Refresher", "Boundless Spirit", "Leech", "Phantom Strike"],
            counters: &COMMON_COUNTERS,
        },
        "calico" | "kali" => HeroBuild {
            hero_name: "Calico",
            early_items: &["High-Velocity Mag", "Extra Stamina", "Mystic Burst", "Sprint Boots"],
            core_items: &["Warp Stone", "Decay", "Kinetic Dash", "Surge of Power"],
            late_items: &["Phantom Strike", "Leech", "Lucky Shot", "Boundless Spirit"],
            counters: &COMMON_COUNTERS,
        },
        "holliday" | "astro" => HeroBuild {
            hero_name: "Holliday",
            early_items: &["High-Velocity Mag", "Restorative Shot", "Extra Stamina", "Sprint Boots"],
            core_items: &["Pristine Emblem", "Headshot Booster", "Warp Stone", "Kinetic Dash"],
            late_items: &["Lucky Shot", "Ricochet", "Glass Cannon", "Unstoppable"],
            counters: &COMMON_COUNTERS,
        },
        _ => GENERIC_BUILD,
    }
}
