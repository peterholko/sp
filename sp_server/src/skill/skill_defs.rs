use bevy::prelude::*;

#[derive(Debug, Reflect, Clone, Hash, PartialEq, Eq)]
pub enum Skill {
    Mining,
    Stonecutting,
    Logging,
    Hunting,
    Foraging,
    Fishing,
    Farming,
    Smelting,
    Masonry,
    Woodcutting,
    Butchery,
    Processing,
    Weaponsmithing,
    Armorsmithing,
    Tanning,
    Toolmaking,
    Cooking,
    Construction,
    Axe,
    Spear,
    Carpentry,
}

impl Skill {
    pub fn from_str(name: &str) -> Option<Skill> {
        match name {
            "Armorsmithing" => Some(Skill::Armorsmithing),
            "Axe" => Some(Skill::Axe),
            "Butchery" => Some(Skill::Butchery),
            "Carpentry" => Some(Skill::Carpentry),
            "Construction" => Some(Skill::Construction),
            "Cooking" => Some(Skill::Cooking),
            "Farming" => Some(Skill::Farming),
            "Fishing" => Some(Skill::Fishing),
            "Foraging" => Some(Skill::Foraging),
            "Hunting" => Some(Skill::Hunting),
            "Logging" => Some(Skill::Logging),
            "Masonry" => Some(Skill::Masonry),
            "Mining" => Some(Skill::Mining),
            "Processing" => Some(Skill::Processing),
            "Smelting" => Some(Skill::Smelting),
            "Spear" => Some(Skill::Spear),
            "Stonecutting" => Some(Skill::Stonecutting),
            "Tanning" => Some(Skill::Tanning),
            "Toolmaking" => Some(Skill::Toolmaking),
            "Weaponsmithing" => Some(Skill::Weaponsmithing),
            "Woodcutting" => Some(Skill::Woodcutting),
            _ => None,
        }
    }

    pub fn to_str(&self) -> &str {
        match self {
            Skill::Armorsmithing => "Armorsmithing",
            Skill::Axe => "Axe",
            Skill::Butchery => "Butchery",
            Skill::Carpentry => "Carpentry",
            Skill::Construction => "Construction",
            Skill::Cooking => "Cooking",
            Skill::Farming => "Farming",
            Skill::Fishing => "Fishing",
            Skill::Foraging => "Foraging",
            Skill::Hunting => "Hunting",
            Skill::Logging => "Logging",
            Skill::Masonry => "Masonry",
            Skill::Mining => "Mining",
            Skill::Processing => "Processing",
            Skill::Smelting => "Smelting",
            Skill::Spear => "Spear",
            Skill::Stonecutting => "Stonecutting",
            Skill::Tanning => "Tanning",
            Skill::Toolmaking => "Toolmaking",
            Skill::Weaponsmithing => "Weaponsmithing",
            Skill::Woodcutting => "Woodcutting",
        }
    }
}