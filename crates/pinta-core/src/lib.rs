//! Pinta's palette engine: palettes in, GTK stylesheets out.

pub mod apply;
pub mod color;
pub mod css;
pub mod palette;
pub mod paths;

pub use apply::{ApplyError, CssState, GtkConfig, RestoreOutcome, Status, Toolkit};
pub use color::Color;
pub use palette::{Palette, PaletteError, Variant};
