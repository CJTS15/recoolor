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
use crate::palette::{Palette, Variant};

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
/// specificity). libadwaita's toasts are a fixed grey bubble with white text in
/// both styles, no named color involved, so they get Recoolor's own variables.
const PLAIN_GTK4: &str = "
window.background { background-color: var(--window-bg-color); color: var(--window-fg-color); }
headerbar.titlebar { background: var(--headerbar-bg-color); color: var(--headerbar-fg-color); }
toast { background: var(--recoolor-toast-bg-color); color: var(--recoolor-toast-fg-color); }
";

/// libadwaita's named colors as `@define-color`, which adw-gtk3 reads in GTK3 too.
pub(crate) fn define_colors(palette: &Palette) -> String {
    let mut css = String::new();
    for (name, value) in named_colors(palette) {
        let _ = writeln!(css, "@define-color {name} {value};");
    }
    css
}

fn colors(palette: &Palette) -> String {
    let colors = named_colors(palette);
    let mut css = define_colors(palette);

    css.push_str("\n:root {\n");
    // libadwaita derives the text colors from the fills itself, also inside OSDs and
    // toasts; setting them here would replace that with Recoolor's fixed values.
    for (name, value) in colors.iter().filter(|(name, _)| !DERIVED.contains(name)) {
        let _ = writeln!(css, "  --{}: {value};", name.replace('_', "-"));
    }
    // A dark bubble in both styles, as GNOME's toasts are: the palette's text in
    // light style, its surface (a step above the window) in dark style.
    let c = &palette.colors;
    let (toast_bg, toast_fg) = match palette.variant {
        Variant::Light => (c.text, c.base),
        Variant::Dark => (c.surface, c.text),
    };
    let _ = writeln!(css, "  --recoolor-toast-bg-color: {toast_bg};");
    let _ = writeln!(css, "  --recoolor-toast-fg-color: {toast_fg};");
    css.push_str("}\n");
    css
}

/// The text colors libadwaita 1.6+ derives from their `*_bg_color` fills.
const DERIVED: [&str; 5] = [
    "accent_color",
    "destructive_color",
    "success_color",
    "warning_color",
    "error_color",
];

fn named_colors(palette: &Palette) -> Vec<(&'static str, String)> {
    let c = &palette.colors;
    let r = Roles::new(palette);
    let shade = r.shade.alpha(r.shade_alpha);
    let hex = |color: crate::color::Color| color.to_string();
    // As libadwaita derives them, for releases and adw-gtk3 that read these names.
    let text = |color: crate::color::Color| hex(color.standalone(palette.variant == Variant::Dark));

    vec![
        ("accent_color", text(c.accent)),
        ("accent_bg_color", hex(c.accent)),
        ("accent_fg_color", hex(r.accent_fg)),
        ("destructive_color", text(c.red)),
        ("destructive_bg_color", hex(c.red)),
        ("destructive_fg_color", hex(r.destructive_fg)),
        ("success_color", text(c.green)),
        ("success_bg_color", hex(c.green)),
        ("success_fg_color", hex(r.success_fg)),
        ("warning_color", text(c.yellow)),
        ("warning_bg_color", hex(c.yellow)),
        ("warning_fg_color", hex(r.warning_fg)),
        ("error_color", text(c.red)),
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
            r.shade.alpha(r.shade_alpha * 2.0),
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
        // Text colors: libadwaita derives the variables; the names get the same value.
        assert!(!css.contains("--accent-color:") && !css.contains("--warning-color:"));
        let accent = mocha.colors.accent.standalone(true);
        assert!(css.contains(&format!("@define-color accent_color {accent};")));
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
        // Toasts: a dark bubble in both, from each half's palette.
        assert!(light_part.contains("--recoolor-toast-bg-color: #4c4f69;"));
        assert!(dark_part.contains("--recoolor-toast-bg-color: #313244;"));
        assert!(css.contains("toast { background: var(--recoolor-toast-bg-color);"));
    }
}
