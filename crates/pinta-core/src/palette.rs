//! Palettes: the bundled set, plus any the user drops into their palettes folder.
//!
//! A palette is a TOML file with a handful of semantic roles rather than a theme's
//! full color list, so every theme family maps onto the same CSS generator. See
//! `palettes/*.toml` for the format.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::color::Color;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Variant {
    Light,
    Dark,
}

/// Semantic color roles, from the window background down to the accents.
#[derive(Clone, Debug, PartialEq)]
pub struct Colors {
    /// Window background.
    pub base: Color,
    /// Slightly offset background: header bars, sidebars, content views.
    pub mantle: Color,
    /// The furthest-offset background: tooltips, deep wells.
    pub crust: Color,
    /// Raised elements: buttons, cards, popovers.
    pub surface: Color,
    /// Muted elements such as borders and placeholders.
    pub overlay: Color,
    pub text: Color,
    /// Secondary text.
    pub subtext: Color,
    pub accent: Color,
    pub red: Color,
    pub orange: Color,
    pub yellow: Color,
    pub green: Color,
    pub blue: Color,
    pub teal: Color,
    pub purple: Color,
    pub pink: Color,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Palette {
    /// Stable identifier: lowercase ASCII letters, digits and dashes.
    pub id: String,
    pub name: String,
    /// Theme family for grouping, e.g. "Catppuccin".
    pub family: String,
    pub variant: Variant,
    pub colors: Colors,
    pub apps: Apps,
}

/// The same theme as shipped for other apps, used instead of a generated one
/// when it is installed, so colors match upstream exactly.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Apps {
    /// A Ptyxis built-in palette (generated from Gogh), by its id.
    pub ptyxis: Option<String>,
    /// fish themes to try in order (`fish_config theme list`).
    #[serde(default)]
    pub fish: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum PaletteError {
    #[error("{}: {source}", path.display())]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{origin}: {source}")]
    Parse {
        origin: String,
        source: toml::de::Error,
    },
    #[error("{origin}: invalid id {id:?} (use lowercase letters, digits and dashes)")]
    InvalidId { origin: String, id: String },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPalette {
    id: String,
    name: String,
    family: String,
    variant: Variant,
    colors: RawColors,
    #[serde(default)]
    apps: Apps,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawColors {
    base: Color,
    mantle: Color,
    crust: Color,
    surface: Color,
    overlay: Color,
    text: Color,
    subtext: Option<Color>,
    accent: Color,
    red: Color,
    orange: Color,
    yellow: Color,
    green: Color,
    blue: Color,
    teal: Option<Color>,
    purple: Color,
    pink: Option<Color>,
}

const BUNDLED: &[(&str, &str)] = &[
    (
        "catppuccin-latte.toml",
        include_str!("../palettes/catppuccin-latte.toml"),
    ),
    (
        "catppuccin-frappe.toml",
        include_str!("../palettes/catppuccin-frappe.toml"),
    ),
    (
        "catppuccin-macchiato.toml",
        include_str!("../palettes/catppuccin-macchiato.toml"),
    ),
    (
        "catppuccin-mocha.toml",
        include_str!("../palettes/catppuccin-mocha.toml"),
    ),
    ("dracula.toml", include_str!("../palettes/dracula.toml")),
    (
        "tokyo-night.toml",
        include_str!("../palettes/tokyo-night.toml"),
    ),
    (
        "tokyo-night-storm.toml",
        include_str!("../palettes/tokyo-night-storm.toml"),
    ),
    (
        "tokyo-night-moon.toml",
        include_str!("../palettes/tokyo-night-moon.toml"),
    ),
    (
        "tokyo-night-day.toml",
        include_str!("../palettes/tokyo-night-day.toml"),
    ),
    (
        "gruvbox-dark.toml",
        include_str!("../palettes/gruvbox-dark.toml"),
    ),
    (
        "gruvbox-light.toml",
        include_str!("../palettes/gruvbox-light.toml"),
    ),
    ("rose-pine.toml", include_str!("../palettes/rose-pine.toml")),
    (
        "rose-pine-moon.toml",
        include_str!("../palettes/rose-pine-moon.toml"),
    ),
    (
        "rose-pine-dawn.toml",
        include_str!("../palettes/rose-pine-dawn.toml"),
    ),
    (
        "everforest-dark.toml",
        include_str!("../palettes/everforest-dark.toml"),
    ),
    (
        "everforest-light.toml",
        include_str!("../palettes/everforest-light.toml"),
    ),
    ("nord.toml", include_str!("../palettes/nord.toml")),
    ("one-dark.toml", include_str!("../palettes/one-dark.toml")),
    (
        "solarized-dark.toml",
        include_str!("../palettes/solarized-dark.toml"),
    ),
    (
        "solarized-light.toml",
        include_str!("../palettes/solarized-light.toml"),
    ),
    (
        "kanagawa-wave.toml",
        include_str!("../palettes/kanagawa-wave.toml"),
    ),
];

impl Palette {
    /// Parses a palette. `origin` names the source in error messages.
    pub fn from_toml(source: &str, origin: &str) -> Result<Self, PaletteError> {
        let raw: RawPalette = toml::from_str(source).map_err(|source| PaletteError::Parse {
            origin: origin.to_owned(),
            source,
        })?;

        // The id ends up in generated CSS comments and file contents, so keep it plain.
        let id_ok = !raw.id.is_empty()
            && raw
                .id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        if !id_ok {
            return Err(PaletteError::InvalidId {
                origin: origin.to_owned(),
                id: raw.id,
            });
        }

        let c = raw.colors;
        Ok(Self {
            id: raw.id,
            name: raw.name,
            family: raw.family,
            variant: raw.variant,
            apps: raw.apps,
            colors: Colors {
                subtext: c.subtext.unwrap_or_else(|| c.text.mix(c.base, 0.3)),
                base: c.base,
                mantle: c.mantle,
                crust: c.crust,
                surface: c.surface,
                overlay: c.overlay,
                text: c.text,
                accent: c.accent,
                red: c.red,
                orange: c.orange,
                yellow: c.yellow,
                teal: c.teal.unwrap_or_else(|| c.blue.mix(c.green, 0.5)),
                pink: c.pink.unwrap_or_else(|| c.red.mix(c.purple, 0.5)),
                green: c.green,
                blue: c.blue,
                purple: c.purple,
            },
        })
    }

    /// This palette with its accent replaced by the color matching one of GNOME's
    /// named accents (`blue`, `teal`, … the `accent-color` setting), so the accent
    /// picker in Settings chooses among the palette's own colors. `None` or an
    /// unknown name keeps the palette's accent.
    pub fn with_accent(&self, gnome_accent: Option<&str>) -> Palette {
        let c = &self.colors;
        let accent = match gnome_accent {
            Some("blue") => c.blue,
            Some("teal") => c.teal,
            Some("green") => c.green,
            Some("yellow") => c.yellow,
            Some("orange") => c.orange,
            Some("red") => c.red,
            Some("pink") => c.pink,
            Some("purple") => c.purple,
            Some("slate") => c.overlay,
            _ => c.accent,
        };
        let mut palette = self.clone();
        palette.colors.accent = accent;
        palette
    }

    pub fn from_file(path: &Path) -> Result<Self, PaletteError> {
        let source = fs::read_to_string(path).map_err(|source| PaletteError::Read {
            path: path.to_owned(),
            source,
        })?;
        Self::from_toml(&source, &path.display().to_string())
    }
}

/// The palettes compiled into Pinta.
pub fn bundled() -> Vec<Palette> {
    BUNDLED
        .iter()
        .map(|(file, source)| {
            // Covered by tests, so a failure here is a build mistake, not user input.
            Palette::from_toml(source, file).unwrap_or_else(|e| panic!("bundled palette: {e}"))
        })
        .collect()
}

/// Every `*.toml` palette in `dir`, sorted by file name. A missing folder yields nothing.
pub fn load_dir(dir: &Path) -> Vec<Result<Palette, PaletteError>> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(source) => {
            return vec![Err(PaletteError::Read {
                path: dir.to_owned(),
                source,
            })];
        }
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|ext| ext == "toml") && p.is_file())
        .collect();
    paths.sort();
    paths.iter().map(|p| Palette::from_file(p)).collect()
}

/// Bundled palettes merged with the user's; a user palette replaces a bundled one
/// with the same id. Broken user files are returned alongside rather than failing
/// the whole catalog.
pub fn catalog(user_dir: Option<&Path>) -> (Vec<Palette>, Vec<PaletteError>) {
    let mut by_id: BTreeMap<String, Palette> =
        bundled().into_iter().map(|p| (p.id.clone(), p)).collect();
    let mut errors = Vec::new();
    for result in user_dir.map(load_dir).unwrap_or_default() {
        match result {
            Ok(p) => {
                by_id.insert(p.id.clone(), p);
            }
            Err(e) => errors.push(e),
        }
    }
    let mut palettes: Vec<Palette> = by_id.into_values().collect();
    palettes.sort_by(|a, b| (&a.family, &a.name).cmp(&(&b.family, &b.name)));
    (palettes, errors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_palettes_parse_with_unique_ids() {
        let palettes = bundled();
        assert_eq!(palettes.len(), BUNDLED.len());
        let mut ids: Vec<_> = palettes.iter().map(|p| p.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), palettes.len());
    }

    #[test]
    fn bundled_file_names_match_ids() {
        for (file, source) in BUNDLED {
            let p = Palette::from_toml(source, file).unwrap();
            assert_eq!(format!("{}.toml", p.id), *file);
        }
    }

    #[test]
    fn variant_matches_background_lightness() {
        for p in bundled() {
            let light = p.colors.base.luminance() > 0.5;
            assert_eq!(light, p.variant == Variant::Light, "{}", p.id);
        }
    }

    #[test]
    fn text_is_readable_on_base() {
        for p in bundled() {
            let ratio = p.colors.text.contrast(p.colors.base);
            assert!(ratio >= 4.5, "{}: text contrast {ratio}", p.id);
        }
    }

    #[test]
    fn gnome_accent_picks_palette_colors() {
        let mocha = bundled()
            .into_iter()
            .find(|p| p.id == "catppuccin-mocha")
            .unwrap();
        let accent = |name| mocha.with_accent(name).colors.accent.to_string();
        assert_eq!(accent(None), "#cba6f7");
        assert_eq!(accent(Some("blue")), "#89b4fa");
        assert_eq!(accent(Some("teal")), "#94e2d5");
        assert_eq!(accent(Some("pink")), "#f5c2e7");
        assert_eq!(accent(Some("nonsense")), "#cba6f7");
    }

    #[test]
    fn teal_and_pink_are_optional() {
        let without = bundled_source("dracula.toml")
            .lines()
            .filter(|l| !l.starts_with("teal") && !l.starts_with("pink"))
            .collect::<Vec<_>>()
            .join("\n");
        let p = Palette::from_toml(&without, "t").unwrap();
        assert_eq!(p.colors.pink, p.colors.red.mix(p.colors.purple, 0.5));
    }

    #[test]
    fn rejects_unsafe_ids_and_unknown_keys() {
        let good = bundled_source("dracula.toml");
        let bad_id = good.replace("id = \"dracula\"", "id = \"x */ body {}\"");
        assert!(matches!(
            Palette::from_toml(&bad_id, "t"),
            Err(PaletteError::InvalidId { .. })
        ));
        let extra = good.replace("[colors]", "[colors]\npink = \"#ff79c6\"");
        assert!(matches!(
            Palette::from_toml(&extra, "t"),
            Err(PaletteError::Parse { .. })
        ));
    }

    #[test]
    fn user_palette_overrides_bundled_and_errors_are_collected() {
        let dir = tempfile::tempdir().unwrap();
        let custom =
            bundled_source("dracula.toml").replace("name = \"Dracula\"", "name = \"My Dracula\"");
        fs::write(dir.path().join("dracula.toml"), custom).unwrap();
        fs::write(dir.path().join("broken.toml"), "id = ").unwrap();
        fs::write(dir.path().join("notes.txt"), "ignored").unwrap();

        let (palettes, errors) = catalog(Some(dir.path()));
        assert_eq!(palettes.len(), BUNDLED.len());
        assert_eq!(
            palettes.iter().find(|p| p.id == "dracula").unwrap().name,
            "My Dracula"
        );
        assert_eq!(errors.len(), 1);
    }

    fn bundled_source(file: &str) -> &'static str {
        BUNDLED.iter().find(|(f, _)| *f == file).unwrap().1
    }
}
