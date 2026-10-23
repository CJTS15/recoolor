//! Spotifast: a `Pinta.json` custom theme in its themes folder. On Linux Spotifast
//! notices changes to that folder by itself, so a selected Pinta theme updates live.
//! Selecting it is left to the user (Settings → Appearance → Theme).
//!
//! The format allows no comment, so the file is Pinta's by name.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use crate::apply::{ApplyError, io_err, write_atomic};
use crate::css::Roles;
use crate::palette::{Palette, Variant};

const FILE: &str = "Pinta.json";

/// Spotifast's config folder: the Flatpak's, or a native install's.
pub fn config_dir(home: &Path, config_home: &Path) -> Option<PathBuf> {
    [
        home.join(".var/app/rocks.spotifast.Spotifast/config/spotifast"),
        config_home.join("spotifast"),
    ]
    .into_iter()
    .find(|d| d.is_dir())
}

pub fn path(config_dir: &Path) -> PathBuf {
    config_dir.join("themes").join(FILE)
}

pub fn render(palette: &Palette) -> String {
    let c = &palette.colors;
    let r = Roles::new(palette);
    let hex = |color: crate::color::Color| Value::String(color.to_string());
    let alpha = |color: crate::color::Color, a: u8| Value::String(format!("{color}{a:02x}"));
    let mut colors = Map::new();
    for (key, value) in [
        ("window", hex(c.base)),
        ("panel", hex(c.mantle)),
        ("surface", hex(c.surface)),
        ("surface_hover", hex(r.hover)),
        ("surface_active", hex(r.active)),
        ("outline", hex(r.border)),
        ("text", hex(c.text)),
        ("secondary", hex(c.subtext)),
        ("dim", hex(c.overlay)),
        ("accent", hex(c.accent)),
        ("accent_hover", hex(c.accent.mix(c.text, 0.15))),
        ("on_accent", hex(r.accent_fg)),
        ("danger", hex(c.red)),
        ("warning", hex(c.yellow)),
        ("overlay", alpha(c.crust, 0xcc)),
        ("shadow", alpha(c.crust, 0x66)),
        ("tint", hex(c.accent)),
    ] {
        colors.insert(key.to_owned(), value);
    }
    let base = match palette.variant {
        Variant::Dark => "dark",
        Variant::Light => "light",
    };
    serde_json::to_string_pretty(&json!({ "base": base, "colors": colors })).expect("serializes")
        + "\n"
}

pub fn write(config_dir: &Path, palette: &Palette) -> Result<(), ApplyError> {
    write_atomic(&path(config_dir), &render(palette))
}

pub fn remove(config_dir: &Path) -> Result<(), ApplyError> {
    let path = path(config_dir);
    match fs::remove_file(&path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(io_err(&path)(e)),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::bundled;

    #[test]
    fn theme_has_every_documented_key() {
        let latte = bundled()
            .into_iter()
            .find(|p| p.id == "catppuccin-latte")
            .unwrap();
        let theme: Value = serde_json::from_str(&render(&latte)).unwrap();
        assert_eq!(theme["base"], "light");
        let colors = theme["colors"].as_object().unwrap();
        assert_eq!(colors.len(), 17);
        assert_eq!(colors["window"], "#eff1f5");
        assert!(colors["overlay"].as_str().unwrap().len() == 9);
    }
}
