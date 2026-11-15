//! The themes of Claude Vivarium - one file per theme - and their registry.
use vivarium_core::scene::{Scene, SceneOpts, Stage};

pub mod aquarium;
pub mod capricorns;
pub mod archipelago;
pub mod espresso;
pub mod forest;
pub mod gorillas;
pub mod guppies;
pub mod halloween;
pub mod meerkats;
pub mod pasture;
pub mod penguins;
pub mod seabed;
pub mod space;
pub mod synthwave;
pub mod terrarium;
pub mod workshop;

/// Seasonal themes that are out of season today. They are left out of the "all" rotation but can
/// always be named in the settings (themes = all, halloween, workshop).
pub fn resting() -> Vec<&'static str> {
    let (month, day) = vivarium_core::util::local_month_day();
    let halloween = month == 10 || (month == 11 && day <= 2); // October to All Souls' Day
    let christmas = month == 12 || (month == 1 && day <= 6); // Advent to Epiphany
    let mut out = Vec::new();
    if !halloween {
        out.push("halloween");
    }
    if !christmas {
        out.push("workshop");
    }
    out
}

macro_rules! registry {
    ($( $name:literal => $ty:ty ),+ $(,)?) => {
        /// All theme names, in the order of the settings file.
        pub const NAMES: &[&str] = &[$($name),+];

        /// Build the scene of a theme.
        pub fn make(name: &str, opts: SceneOpts) -> Option<Box<dyn Scene>> {
            match name {
                $( $name => Some(Stage::<$ty>::boxed(opts)), )+
                _ => None,
            }
        }
    };
}

registry! {
    "aquarium" => aquarium::Aquarium,
    "space" => space::Space,
    "synthwave" => synthwave::Synthwave,
    "pasture" => pasture::Pasture,
    "seabed" => seabed::Seabed,
    "forest" => forest::Forest,
    "guppies" => guppies::Guppies,
    "terrarium" => terrarium::Terrarium,
    "meerkats" => meerkats::Meerkats,
    "espresso" => espresso::Espresso,
    "archipelago" => archipelago::Archipelago,
    "gorillas" => gorillas::Gorillas,
    "penguins" => penguins::Penguins,
    "capricorns" => capricorns::Capricorns,
    "halloween" => halloween::Halloween,
    "workshop" => workshop::Workshop,
}
