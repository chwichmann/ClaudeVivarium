//! The themes of Claude Vivarium - one file per theme - and their registry.
use vivarium_core::scene::{Scene, SceneOpts, Stage};

pub mod meerkats;

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
    "meerkats" => meerkats::Meerkats,
}
