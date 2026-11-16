//! Claude Vivarium core: everything that does not depend on a window or a GPU.
pub mod canvas;
pub mod config;
pub mod governor;
pub mod scene;
pub mod sessions;
pub mod show;
pub mod sysmon;
pub mod text;
pub mod util;

/// What a theme file needs, in one import.
pub mod prelude {
    pub use crate::canvas::{Affine, Canvas, FillRule, Gradient, LineCap, LineJoin, Picture};
    pub use crate::scene::{
        color_of, sorted_actors, ActorT, Base, Env, FloatStyle, Other, Scene, SceneOpts, Stage, Theme, WORKING_COLOR,
    };
    pub use crate::sessions::{Look, Session, State};
    pub use crate::text::{self, weight, Family};
    pub use crate::util::*;
    pub use std::f64::consts::PI;
}
