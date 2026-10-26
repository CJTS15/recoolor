//! Recoolor's palette engine: palettes in, GTK stylesheets out.

pub mod apply;
pub mod apps;
pub mod color;
pub mod css;
pub mod flatpak;
pub mod palette;
pub mod paths;
pub mod service;
pub mod shell;
pub mod state;
pub mod themes;
pub mod wallpaper;

pub use apply::{ApplyError, CssState, GtkConfig, RestoreOutcome, Status, Toolkit};
pub use color::Color;
pub use palette::{Palette, PaletteError, Variant};
