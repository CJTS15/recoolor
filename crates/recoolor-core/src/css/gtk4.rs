//! GTK4 and libadwaita.
//!
//! libadwaita styles everything from named colors, so overriding them recolors
//! libadwaita apps completely. Each color is emitted twice: as a CSS variable
//! (`--window-bg-color`, libadwaita 1.6+) and as `@define-color` for older releases.
//!
//! A light/dark pair puts the dark palette inside `@media (prefers-color-scheme: dark)`
//! (GTK 4.16+), so apps follow GNOME's style switch without Recoolor running.

use std::fmt::Write;

use super::{PAIR_SEPARATOR, Roles, header};
use crate::palette::Palette;

pub fn gtk4(palette: &Palette) -> String {
    header(&palette.id) + &colors(palette) + PLAIN_GTK4
}

pub fn gtk4_pair(light: &Palette, dark: &Palette) -> String {
    format!(
        "{}{}\n@media (prefers-color-scheme: dark) {{\n{}}}\n{PLAIN_GTK4}",
        header(&format!("{}{PAIR_SEPARATOR}{}", light.id, dark.id)),
        colors(light),
        colors(dark)
    )
}

/// GTK's own built-in theme (what plain GTK4 apps and Chrome's GTK mode use) has
/// its colors baked into its rules rather than named, so set the main surfaces
/// directly (only what Chrome's GTK mode samples: windows, not every `.background`,
/// which popovers and tooltips also carry, and not `.view`, which libadwaita often
/// draws transparent). They read the same variables libadwaita does, so libadwaita apps look
/// the same, and a pair still switches with the media query. Header bars use the
/// `background` shorthand because the built-in theme draws them with a gradient,
/// and only window title bars, since libadwaita's own header bars are often meant
/// to be transparent (and a user stylesheet outranks libadwaita's regardless of
/// specificity).
const PLAIN_GTK4: &str = "
window.background { background-color: var(--window-bg-color); color: var(--window-fg-color); }
headerbar.titlebar { background: var(--headerbar-bg-color); color: var(--headerbar-fg-color); }
";

fn colors(palette: &Palette) -> String {
    let colors = named_colors(palette);
    let mut css = String::new();

    for (name, value) in &colors {
        let _ = writeln!(css, "@define-color {name} {value};");
    }

    css.push_str("\n:root {\n");
    for (name, value) in &colors {
        let _ = writeln!(css, "  --{}: {value};", name.replace('_', "-"));
    }
    css.push_str("}\n");
    css
}

fn named_colors(palette: &Palette) -> Vec<(&'static str, String)> {
    let c = &palette.colors;
    let r = Roles::new(palette);
    let shade = c.crust.alpha(r.shade_alpha);
    let hex = |color: crate::color::Color| color.to_string();

    vec![
        ("accent_color", hex(c.accent)),
        ("accent_bg_color", hex(c.accent)),
        ("accent_fg_color", hex(r.accent_fg)),
        ("destructive_color", hex(c.red)),
        ("destructive_bg_color", hex(c.red)),
        ("destructive_fg_color", hex(r.destructive_fg)),
        ("success_color", hex(c.green)),
        ("success_bg_color", hex(c.green)),
        ("success_fg_color", hex(r.success_fg)),
        ("warning_color", hex(c.yellow)),
        ("warning_bg_color", hex(c.yellow)),
        ("warning_fg_color", hex(r.warning_fg)),
        ("error_color", hex(c.red)),
        ("error_bg_color", hex(c.red)),
        ("error_fg_color", hex(r.destructive_fg)),
        ("window_bg_color", hex(c.base)),
        ("window_fg_color", hex(c.text)),
        ("view_bg_color", hex(c.mantle)),
        ("view_fg_color", hex(c.text)),
        ("headerbar_bg_color", hex(c.mantle)),
        ("headerbar_fg_color", hex(c.text)),
        ("headerbar_border_color", hex(c.text)),
        ("headerbar_backdrop_color", hex(c.base)),
        ("headerbar_shade_color", shade.clone()),
        (
            "headerbar_darker_shade_color",
            c.crust.alpha(r.shade_alpha * 2.0),
        ),
        ("sidebar_bg_color", hex(c.mantle)),
        ("sidebar_fg_color", hex(c.text)),
        ("sidebar_backdrop_color", hex(c.base)),
        ("sidebar_border_color", shade.clone()),
        ("sidebar_shade_color", shade.clone()),
        ("secondary_sidebar_bg_color", hex(c.base)),
        ("secondary_sidebar_fg_color", hex(c.text)),
        ("secondary_sidebar_backdrop_color", hex(c.base)),
        ("secondary_sidebar_border_color", shade.clone()),
        ("secondary_sidebar_shade_color", shade.clone()),
        ("card_bg_color", hex(c.surface)),
        ("card_fg_color", hex(c.text)),
        ("card_shade_color", shade.clone()),
        ("thumbnail_bg_color", hex(c.surface)),
        ("thumbnail_fg_color", hex(c.text)),
        ("dialog_bg_color", hex(c.base)),
        ("dialog_fg_color", hex(c.text)),
        ("popover_bg_color", hex(c.surface)),
        ("popover_fg_color", hex(c.text)),
        ("popover_shade_color", shade.clone()),
        ("shade_color", shade),
        ("scrollbar_outline_color", c.crust.alpha(0.5)),
        // Pre-libadwaita names, still read by plain GTK4 apps and some widgets.
        ("theme_bg_color", hex(c.base)),
        ("theme_fg_color", hex(c.text)),
        ("theme_base_color", hex(c.mantle)),
        ("theme_text_color", hex(c.text)),
        ("theme_selected_bg_color", hex(c.accent)),
        ("theme_selected_fg_color", hex(r.accent_fg)),
        ("borders", hex(r.border)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::bundled;

    #[test]
    fn mocha_sets_window_and_accent() {
        let mocha = bundled()
            .into_iter()
            .find(|p| p.id == "catppuccin-mocha")
            .unwrap();
        let css = gtk4(&mocha);
        assert!(css.contains("@define-color window_bg_color #1e1e2e;"));
        assert!(css.contains("  --window-bg-color: #1e1e2e;"));
        assert!(css.contains("@define-color accent_bg_color #cba6f7;"));
        assert!(css.contains("@define-color accent_fg_color #11111b;"));
    }

    #[test]
    fn pair_puts_dark_palette_in_media_query() {
        let [latte, mocha] = ["catppuccin-latte", "catppuccin-mocha"]
            .map(|id| bundled().into_iter().find(|p| p.id == id).unwrap());
        let css = gtk4_pair(&latte, &mocha);
        let (light_part, dark_part) = css
            .split_once("@media (prefers-color-scheme: dark) {")
            .unwrap();
        assert!(light_part.contains("--window-bg-color: #eff1f5;"));
        assert!(dark_part.contains("--window-bg-color: #1e1e2e;"));
        assert!(!light_part.contains("#1e1e2e"));
    }
}
