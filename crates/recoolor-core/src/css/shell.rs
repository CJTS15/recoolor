//! GNOME Shell.
//!
//! Rather than hand-written overrides that drift between Shell releases, this
//! recolors the Shell's own default stylesheet (`gnome-shell-dark.css` or
//! `gnome-shell-light.css`, which the app reads from the installed Shell), so
//! coverage always matches the running version:
//!
//! - neutral greys map onto the palette's base-to-text ramp by lightness,
//! - saturated colors map to the palette color with the nearest hue,
//! - `-st-accent-color` becomes the palette's accent.
//!
//! Only property values outside strings are touched, so `#dad` stays an id and
//! `url("…#bad.svg")` stays a path.

use super::{Roles, header};
use crate::color::Color;
use crate::palette::{Palette, Variant};

/// Lightness of Adwaita's Shell greys: light style's text and popovers (`#222226`,
/// `#fafafb`), and dark style's popovers and cards (`#36363a`, `#47474c`).
const ADWAITA_DARK: f32 = 0.1386;
const ADWAITA_LIGHT: f32 = 0.9817;
const ADWAITA_DARK_POPOVER: f32 = 0.2170;
const ADWAITA_DARK_CARD: f32 = 0.2850;
const WHITE: Color = Color::rgb(255, 255, 255);
const BLACK: Color = Color::rgb(0, 0, 0);

/// `default_css` must be the Shell stylesheet for `palette.variant`.
pub fn shell(palette: &Palette, default_css: &str) -> String {
    header(&palette.id)
        + &recolor(palette, pristine(default_css))
        + &osd_level(palette)
        + &light_switcher(palette)
        + &switcher_labels(palette)
        + "\n"
}

/// The Alt+Tab switcher's app and window titles in the palette's text. Inheriting
/// it from the switcher item does not reach them (they came out a stray warm
/// white), so they are set directly. Also in Recoolor's extension stylesheet,
/// which outranks the theme.
pub fn switcher_labels(palette: &Palette) -> String {
    format!(
        ".switcher-list .item-box StLabel {{ color: {} !important; }}",
        palette.colors.text
    )
}

/// GNOME keeps the Alt+Tab switcher dark in light style too; Recoolor makes it
/// follow the style, with the palette's text and a highlight to match.
/// Important, to outrank the switcher's own rules for each item state.
fn light_switcher(palette: &Palette) -> String {
    if palette.variant != Variant::Light {
        return String::new();
    }
    let c = &palette.colors;
    let t = c.text;
    format!(
        ".switcher-list {{ background-color: {} !important; }}\n\
         .switcher-list, .switcher-list .item-box, .switcher-arrow {{ color: {} !important; }}\n\
         .switcher-list .item-box:hover, .switcher-list .item-box:focus, \
         .switcher-list .item-box:selected {{ background-color: rgba({}, {}, {}, 0.12) !important; }}\n",
        c.base, t, t.r, t.g, t.b
    )
}

/// The volume and brightness OSD's bar in the accent, as quick settings' sliders
/// are, over a track in the OSD's light text (GNOME keeps the OSD dark in both
/// styles). Important, to outrank Blur my Shell's light-surface rule that
/// `shell_blur` moves in, which assumes a light OSD.
fn osd_level(palette: &Palette) -> String {
    let c = &palette.colors;
    let fg = match palette.variant {
        Variant::Light => c.base,
        Variant::Dark => c.text,
    };
    format!(
        "\n.osd-window .level {{ -barlevel-active-background-color: {} !important; \
         -barlevel-background-color: rgba({}, {}, {}, 0.3) !important; }}\n",
        c.accent, fg.r, fg.g, fg.b
    )
}

/// Blur my Shell's "Preserve shell theme" mode keeps the theme's own solid colors
/// inside its blurred popups; its other mode makes the inner elements (calendar
/// days, cards, quick toggles) translucent so the blur shows through. This takes
/// those rules from the installed Blur my Shell's `stylesheet.css`, moves them to
/// the classes it uses in preserve mode, and recolors them through the palette,
/// with accent fills translucent too. Without Blur my Shell the classes never
/// apply, so the result is harmless.
pub fn shell_blur(palette: &Palette, bms_css: &str) -> String {
    let mut css = String::new();
    for variant in ["dark", "light"] {
        let (from, to) = (
            format!(".bms-popup-background-{variant} "),
            format!(".bms-popup-surface-{variant} "),
        );
        // BMS already styles these in preserve mode; leave them to it.
        let own: Vec<&str> = rules(bms_css)
            .flat_map(|(sels, _)| sels.split(','))
            .filter_map(|s| s.trim().strip_prefix(to.as_str()))
            .collect();
        for (sels, body) in rules(bms_css) {
            let moved: Vec<String> = sels
                .split(',')
                .filter_map(|s| s.trim().strip_prefix(from.as_str()))
                .filter(|inner| !own.contains(inner))
                .map(|inner| format!("{to}{inner}"))
                .collect();
            if !moved.is_empty() {
                let body = body.replace(
                    "background-color: -st-accent-color",
                    "background-color: st-transparentize(-st-accent-color, 0.25)",
                );
                css.push_str(&format!("{} {{{body}}}\n", moved.join(",\n")));
            }
        }
    }
    if css.is_empty() {
        return css;
    }
    format!(
        "\n/* Blur my Shell, preserve-theme mode: translucent inner elements. */\n{}",
        recolor(palette, &css)
    )
}

/// Blur my Shell's "Preserve shell theme" mode paints its popups and cards
/// translucent white or grey, and an extension's stylesheet outranks any Shell
/// theme. These rules go in Recoolor's own extension stylesheet, which ranks like
/// Blur my Shell's, with the class doubled to outrank its rules. Each keeps Blur my
/// Shell's own alpha, so its style and blur settings still show, in the palette's
/// colors: its fills become the window color (`base`) or, for cards, `surface`;
/// its faint overlays become the palette's text. Its transparent style keeps a
/// readable minimum. A second set, with the fills opaque, applies under
/// `recoolor-solid-popups`, which Recoolor's extension sets while Blur my Shell's
/// popup blur is off, when see-through would show the windows behind unblurred.
pub fn shell_popups(palette: &Palette, bms_css: &str) -> String {
    let c = &palette.colors;
    let rgba =
        |color: Color, alpha: f32| format!("rgba({}, {}, {}, {alpha})", color.r, color.g, color.b);
    let (mut see_through, mut solid): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
    let push = |lines: &mut Vec<String>, line: String| {
        if !lines.contains(&line) {
            lines.push(line);
        }
    };
    for (sels, body) in rules(bms_css) {
        let Some((bms_color, alpha)) = background(body) else {
            continue;
        };
        let light = u16::from(bms_color.r) + u16::from(bms_color.g) + u16::from(bms_color.b) > 384;
        for sel in sels.split(',').map(str::trim) {
            for variant in ["light", "dark", "transparent"] {
                let class = format!(".bms-popup-surface-{variant}");
                let Some(inner) = sel.strip_prefix(&format!("{class} ")) else {
                    continue;
                };
                let container = !inner.contains([' ', ':', '>']);
                // Its own background for the style, not a tint over one.
                let fill = match variant {
                    "light" => light,
                    "dark" => !light,
                    _ => !light && (alpha > 0.0 || container),
                };
                if alpha == 0.0 && !(variant == "transparent" && container) {
                    continue;
                }
                let card = inner == ".quick-toggle-menu"
                    || inner.starts_with(".message") && !inner.contains(".message-media-control");
                let always_dark = palette.variant == Variant::Light
                    && matches!(
                        inner,
                        ".osd-window"
                            | ".workspace-switcher"
                            | ".resize-popup"
                            | ".screenshot-ui-panel"
                    );
                let color = match () {
                    _ if !fill => c.text,
                    _ if always_dark => c.text,
                    _ if card => c.surface,
                    _ => c.base,
                };
                let alpha = if variant == "transparent" && fill {
                    alpha.max(0.5)
                } else {
                    alpha
                };
                let rule = |prefix: &str, alpha: f32| {
                    format!(
                        "{prefix}{class}{class} {inner} {{ background-color: {} !important; }}",
                        rgba(color, alpha)
                    )
                };
                push(&mut see_through, rule("", alpha));
                if fill {
                    push(&mut solid, rule(".recoolor-solid-popups", 1.0));
                }
            }
        }
    }
    see_through.extend(solid);
    see_through.join("\n")
}

/// The color and alpha of a rule's `background` or `background-color`, with
/// `transparent` as black at 0.
fn background(body: &str) -> Option<(Color, f32)> {
    let value = body.split(';').find_map(|decl| {
        let (key, v) = decl.split_once(':')?;
        matches!(key.trim(), "background" | "background-color").then(|| v.trim())
    })?;
    if value.starts_with("transparent") {
        return Some((BLACK, 0.0));
    }
    let inner = value.strip_prefix("rgba(")?.split(')').next()?;
    let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
    let [r, g, b, a] = parts[..] else {
        return None;
    };
    Some((
        Color::rgb(r.parse().ok()?, g.parse().ok()?, b.parse().ok()?),
        a.parse().ok()?,
    ))
}

/// Whether SearchLightNG's code (`extension.js`) still names its widgets as
/// [`searchlight`]'s rules expect; an update renaming them would leave them unused.
pub fn searchlight_names_known(js: &str) -> bool {
    ["'searchLight'", "'searchLightBox'"]
        .iter()
        .all(|name| js.contains(name))
}

/// SearchLightNG, a search popup: with its blur synced to Blur my Shell it ignores
/// its own background color and always uses white text. A palette tint behind its
/// results, as see-through as Blur my Shell's popups are for the style, and the
/// palette's text, with two ids to outrank its own one-id rules. The tint sits on
/// the results box itself: on the layers below, the blur's brightness darkens it.
pub fn searchlight(palette: &Palette, bms_css: Option<&str>) -> String {
    let c = &palette.colors;
    let variant = match palette.variant {
        Variant::Light => "light",
        Variant::Dark => "dark",
    };
    let popup = format!(".bms-popup-surface-{variant} .popup-menu-content");
    // Blur my Shell's own popup alpha; its defaults without it.
    let alpha = bms_css
        .and_then(|bms| {
            rules(bms)
                .filter(|(sels, _)| sels.split(',').any(|s| s.trim() == popup))
                .find_map(|(_, body)| background(body))
        })
        .map_or(
            if palette.variant == Variant::Light {
                0.65
            } else {
                0.5
            },
            |(_, alpha)| alpha,
        );
    format!(
        "#searchLight #searchLightBox {{ background-color: rgba({}, {}, {}, {alpha}); }}\n\
         #searchLight #searchLightBox * {{ color: {} !important; }}\n\
         #searchLight #searchLightBox StBoxLayout.search-section-content {{ background-color: transparent !important; }}",
        c.base.r, c.base.g, c.base.b, c.text
    )
}

/// GNOME's high-contrast Shell outlines popups, cards and controls and draws strong
/// focus rings, but Recoolor's theme loads on top of it and replaces them. These
/// rules redraw them in the palette (the text at 40%, the accent for focus) for the
/// same selectors, under `recoolor-high-contrast`, which Recoolor's extension sets
/// while High Contrast is on. `hc_css` is the Shell's `gnome-shell-high-contrast.css`.
pub fn shell_high_contrast(palette: &Palette, hc_css: &str) -> String {
    let c = &palette.colors;
    let text = |alpha: &str| format!("rgba({}, {}, {}, {alpha})", c.text.r, c.text.g, c.text.b);
    let mut groups: Vec<(String, Vec<String>)> = Vec::new();
    for (sels, body) in rules(hc_css) {
        // The last of each wins, as in CSS: focus rules often follow an outline.
        let last = |property: &str| {
            body.split(';')
                .rev()
                .find_map(|d| d.trim().strip_prefix(property).map(str::trim))
        };
        let mut decls = Vec::new();
        // Outlines and focus rings, not the 100px fills it uses for highlights.
        if let Some(shadow) = last("box-shadow:")
            && let Some((width, _)) = shadow
                .strip_prefix("inset 0 0 0 ")
                .and_then(|s| s.split_once("px "))
            && matches!(width, "1" | "2")
        {
            let color = if shadow.contains("accent-color") {
                c.accent.to_string()
            } else {
                text("0.4")
            };
            decls.push(format!(
                "box-shadow: inset 0 0 0 {width}px {color} !important;"
            ));
        }
        // Popups' own borders: white at some strength, which becomes the text.
        for property in ["border:", "border-color:"] {
            if let Some(value) = last(property)
                && let Some((before, rest)) = value.split_once("rgba(255, 255, 255, ")
                && let Some((alpha, after)) = rest.split_once(')')
            {
                decls.push(format!(
                    "{property} {before}{}{after} !important;",
                    text(alpha)
                ));
            }
        }
        for decl in decls {
            let index = match groups.iter().position(|(d, _)| *d == decl) {
                Some(i) => i,
                None => {
                    groups.push((decl, Vec::new()));
                    groups.len() - 1
                }
            };
            for sel in sels.split(',').map(str::trim) {
                let sel = format!(".recoolor-high-contrast {sel}");
                if !groups[index].1.contains(&sel) {
                    groups[index].1.push(sel);
                }
            }
        }
    }
    groups
        .iter()
        .map(|(decl, sels)| format!("{} {{ {decl} }}\n", sels.join(",\n")))
        .collect()
}

/// `(selectors, body)` of each top-level rule; Blur my Shell's sheet has no nesting.
fn rules(css: &str) -> impl Iterator<Item = (&str, &str)> {
    css.split('}').filter_map(|rule| {
        let (sels, body) = rule.split_once('{')?;
        // Drop comments in front of the selectors.
        let sels = sels.rsplit("*/").next().unwrap_or(sels).trim();
        Some((sels, body))
    })
}

/// GDM Settings rebuilds the system stylesheet as the original followed by the
/// chosen theme, so after it has used Recoolor's theme (or one from before the rename to Recoolor) the original
/// ends at the first generated marker. Recoloring past it would recolor colors
/// twice, compounding with every use.
fn pristine(css: &str) -> &str {
    css.find("/* Generated by ").map_or(css, |end| &css[..end])
}

fn recolor(palette: &Palette, css: &str) -> String {
    let accent = palette.colors.accent.to_string();
    let accent_fg = Roles::new(palette).accent_fg.to_string();
    let bytes = css.as_bytes();
    let mut out = String::with_capacity(css.len());
    let (mut i, mut copied) = (0, 0);
    let (mut depth, mut in_value) = (0u32, false);
    // Where the current declaration starts, and whether it is a text `color`.
    let (mut decl_start, mut text) = (0, false);

    while i < bytes.len() {
        if bytes[i..].starts_with(b"/*") {
            i = css[i..].find("*/").map_or(bytes.len(), |end| i + end + 2);
            continue;
        }
        if let quote @ (b'"' | b'\'') = bytes[i] {
            i = css[i + 1..]
                .find(char::from(quote))
                .map_or(bytes.len(), |end| i + 1 + end + 1);
            continue;
        }
        let replacement = match bytes[i] {
            b'{' => {
                depth += 1;
                in_value = false;
                decl_start = i + 1;
                None
            }
            b'}' => {
                depth = depth.saturating_sub(1);
                in_value = false;
                None
            }
            b':' if depth > 0 && !in_value => {
                in_value = true;
                text = css[decl_start..i].trim() == "color";
                None
            }
            b';' => {
                in_value = false;
                decl_start = i + 1;
                None
            }
            _ if in_value => token_at(css, i, palette, &accent, &accent_fg, text),
            _ => None,
        };
        match replacement {
            Some((len, text)) => {
                out.push_str(&css[copied..i]);
                out.push_str(&text);
                i += len;
                copied = i;
            }
            None => i += 1,
        }
    }
    out.push_str(&css[copied..]);
    out
}

/// A color token starting at `i`, as (its length, the replacement).
fn token_at(
    css: &str,
    i: usize,
    palette: &Palette,
    accent: &str,
    accent_fg: &str,
    text: bool,
) -> Option<(usize, String)> {
    let rest = &css[i..];
    let bytes = rest.as_bytes();
    let ident = |b: u8| b.is_ascii_alphanumeric() || b == b'-' || b == b'_';
    let starts_word = i == 0 || !ident(css.as_bytes()[i - 1]);

    for (name, value) in [
        ("-st-accent-fg-color", accent_fg),
        ("-st-accent-color", accent),
    ] {
        if rest.starts_with(name) {
            return Some((name.len(), value.to_owned()));
        }
    }

    if bytes[0] == b'#' {
        let digits = bytes[1..]
            .iter()
            .take_while(|b| b.is_ascii_hexdigit())
            .count();
        let ends = bytes.get(1 + digits).is_none_or(|&b| !ident(b));
        let hex = match digits {
            6 => rest[1..7].to_owned(),
            3 => rest[1..4].chars().flat_map(|c| [c, c]).collect(),
            _ => return None,
        };
        let color = Color::from_hex(&format!("#{hex}")).ok().filter(|_| ends)?;
        return Some((1 + digits, map(color, palette, text).to_string()));
    }

    if starts_word && (rest.starts_with("rgba(") || rest.starts_with("rgb(")) {
        let open = rest.find('(')?;
        let close = rest.find(')')?;
        let parts: Vec<&str> = rest[open + 1..close].split(',').map(str::trim).collect();
        let channel = |k: usize| parts.get(k)?.parse::<u8>().ok();
        let color = map(
            Color::rgb(channel(0)?, channel(1)?, channel(2)?),
            palette,
            text,
        );
        let text = match parts.get(3) {
            Some(alpha) if parts.len() == 4 => {
                format!("rgba({}, {}, {}, {alpha})", color.r, color.g, color.b)
            }
            None => color.to_string(),
            _ => return None,
        };
        return Some((close + 1, text));
    }

    // `whitesmoke` before `white`, which it starts with.
    for (name, color) in [
        ("whitesmoke", Color::rgb(245, 245, 245)),
        ("white", WHITE),
        ("black", BLACK),
    ] {
        let ends = bytes.get(name.len()).is_none_or(|&b| !ident(b));
        if starts_word && ends && rest.starts_with(name) {
            return Some((name.len(), map(color, palette, text).to_string()));
        }
    }
    None
}

/// The palette color standing in for one of Adwaita's.
fn map(color: Color, palette: &Palette, text: bool) -> Color {
    let c = &palette.colors;
    let Some(hue) = color.hue() else {
        let l = (f32::from(color.r) + f32::from(color.g) + f32::from(color.b)) / 3.0 / 255.0;
        // Adwaita's popover grey becomes the palette's base and its card grey the
        // palette's surface, as they are in apps; other greys run between those,
        // out to crust and text.
        return match palette.variant {
            Variant::Dark if l >= ADWAITA_DARK_CARD => c.surface.mix(
                c.text,
                (l - ADWAITA_DARK_CARD) / (ADWAITA_LIGHT - ADWAITA_DARK_CARD),
            ),
            Variant::Dark if l >= ADWAITA_DARK_POPOVER => c.base.mix(
                c.surface,
                (l - ADWAITA_DARK_POPOVER) / (ADWAITA_DARK_CARD - ADWAITA_DARK_POPOVER),
            ),
            Variant::Dark => c
                .base
                .mix(c.crust, (ADWAITA_DARK_POPOVER - l) / ADWAITA_DARK_POPOVER),
            Variant::Light if l <= ADWAITA_LIGHT => c
                .base
                .mix(c.text, (ADWAITA_LIGHT - l) / (ADWAITA_LIGHT - ADWAITA_DARK)),
            // Lighter than the popover: white text on the dark OSDs and panel stays
            // near white; white cards take the palette's surface.
            Variant::Light if text => c
                .base
                .mix(WHITE, (l - ADWAITA_LIGHT) / (1.0 - ADWAITA_LIGHT) * 0.5),
            Variant::Light => c
                .base
                .mix(c.surface, (l - ADWAITA_LIGHT) / (1.0 - ADWAITA_LIGHT)),
        };
    };

    // ponytail: every shade of a hue maps to one palette color, so Adwaita's
    // hover/pressed variants of red or yellow look the same. Keep the lightness
    // offset if that shows.
    palette.hue_color(hue)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::css::applied_palette_id;
    use crate::palette::bundled;

    fn palette(id: &str) -> Palette {
        bundled().into_iter().find(|p| p.id == id).unwrap()
    }

    #[test]
    fn moves_blur_my_shell_rules_to_preserve_mode() {
        let mocha = palette("catppuccin-mocha");
        let bms = "/* popups */\n\
            .bms-popup-surface-dark .quick-settings { background: rgba(30, 30, 33, 0.5); }\n\
            .bms-popup-background-dark .quick-settings,\n\
            .bms-popup-background-dark .quick-toggle { background-color: rgba(255, 255, 255, 0.1); }\n\
            .bms-popup-background-dark .quick-toggle:checked { background-color: -st-accent-color !important; }\n";
        let css = shell_blur(&mocha, bms);
        // Inner element moved and recolored; the container is left to Blur my Shell.
        assert!(css.contains(
            ".bms-popup-surface-dark .quick-toggle { background-color: rgba(205, 214, 244, 0.1); }"
        ));
        assert!(!css.contains("surface-dark .quick-settings"));
        assert!(css.contains("background-color: st-transparentize(#cba6f7, 0.25) !important;"));
        assert_eq!(shell_blur(&mocha, ""), "");
    }

    #[test]
    fn popups_follow_blur_my_shells_alpha_in_the_palette() {
        let mocha = palette("catppuccin-mocha");
        let bms = ".bms-popup-surface-dark .popup-menu-content,\n\
            .bms-popup-surface-dark .quick-settings { background: rgba(30, 30, 33, 0.5) !important; }\n\
            .bms-popup-surface-dark .message-list .message { background: rgba(255, 255, 255, 0.07) !important; }\n\
            .bms-popup-surface-light .message-list .message:hover { background: rgba(255, 255, 255, 0.4) !important; }\n\
            .bms-popup-surface-transparent .popup-menu-content { background: transparent !important; }\n\
            .bms-popup-surface-dark .message-media-control { background-color: transparent; }\n\
            .bms-popup-surface-dark .modal-dialog { border: 1px solid rgba(255, 255, 255, 0.1); }\n\
            .bms-popup-background-dark .quick-toggle { background-color: rgba(255, 255, 255, 0.1); }\n";
        let css = shell_popups(&mocha, bms);
        let has = |line: &str| assert!(css.lines().any(|l| l == line), "missing {line}\n{css}");
        // Fills keep Blur my Shell's alpha in base (#1e1e2e); cards in surface (#313244).
        has(
            ".bms-popup-surface-dark.bms-popup-surface-dark .quick-settings { background-color: rgba(30, 30, 46, 0.5) !important; }",
        );
        has(
            ".bms-popup-surface-light.bms-popup-surface-light .message-list .message:hover { background-color: rgba(49, 50, 68, 0.4) !important; }",
        );
        // Dark cards stay its faint overlay, in the palette's text (#cdd6f4).
        has(
            ".bms-popup-surface-dark.bms-popup-surface-dark .message-list .message { background-color: rgba(205, 214, 244, 0.07) !important; }",
        );
        // Its transparent style keeps a readable minimum.
        has(
            ".bms-popup-surface-transparent.bms-popup-surface-transparent .popup-menu-content { background-color: rgba(30, 30, 46, 0.5) !important; }",
        );
        // Opaque fills while its blur is off; overlays need none.
        has(
            ".recoolor-solid-popups.bms-popup-surface-dark.bms-popup-surface-dark .quick-settings { background-color: rgba(30, 30, 46, 1) !important; }",
        );
        assert!(!css.contains(
            ".recoolor-solid-popups.bms-popup-surface-dark.bms-popup-surface-dark .message-list"
        ));
        // Not where it paints nothing, nor its other mode.
        assert!(!css.contains("media-control"));
        assert!(!css.contains("modal-dialog"));
        assert!(!css.contains("quick-toggle "));
        assert_eq!(shell_popups(&mocha, ""), "");
        // OSDs stay dark in light style: the palette's text (#4c4f69), not its base.
        let latte = palette("catppuccin-latte");
        let osd = ".bms-popup-surface-light .osd-window { background: rgba(255, 255, 255, 0.65) !important; }";
        assert!(
            shell_popups(&latte, osd)
                .contains("osd-window { background-color: rgba(76, 79, 105, 0.65)")
        );
    }

    #[test]
    fn osd_bar_takes_the_accent() {
        let latte = palette("catppuccin-latte").with_accent(Some("teal"));
        let css = shell(&latte, "");
        assert!(css.contains(&format!(
            ".osd-window .level {{ -barlevel-active-background-color: {} !important; \
             -barlevel-background-color: rgba(239, 241, 245, 0.3) !important; }}",
            latte.colors.accent
        )));
    }

    #[test]
    fn alt_tab_switcher_follows_the_style() {
        let latte = palette("catppuccin-latte");
        let css = shell(&latte, "");
        assert!(css.contains(".switcher-list { background-color: #eff1f5 !important; }"));
        assert!(css.contains(".switcher-arrow { color: #4c4f69 !important; }"));
        let switcher = ".bms-popup-surface-light .switcher-list { background: rgba(255, 255, 255, 0.65) !important; }";
        assert!(shell_popups(&latte, switcher).contains("rgba(239, 241, 245, 0.65)"));
        // Dark style keeps GNOME's own.
        assert!(!shell(&palette("catppuccin-mocha"), "").contains(".switcher-list {"));
    }

    #[test]
    fn searchlight_names_are_checked() {
        let js = "this._entry = new St.Entry({ name: 'searchLight' });\n\
                  this._box = new St.BoxLayout({ name: 'searchLightBox' });";
        assert!(searchlight_names_known(js));
        assert!(!searchlight_names_known(
            "new St.BoxLayout({ name: 'searchBox' })"
        ));
    }

    #[test]
    fn high_contrast_outlines_take_the_palette() {
        let latte = palette("catppuccin-latte");
        // Shapes from gnome-shell-high-contrast.css.
        let hc = "\
.button, .icon-button {
  box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.4); }
.button:focus {
  box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.4);
  box-shadow: inset 0 0 0 2px st-transparentize(st-mix(-st-accent-color, #ffffff, 60%), 0.1) !important; }
.popup-menu-content {
  border: 1px solid rgba(255, 255, 255, 0.5);
  box-shadow: 0 2px 4px 0 transparent; }
.overview-tile:selected {
  box-shadow: inset 0 0 0 100px rgba(255, 255, 255, 0.28); }
";
        let css = shell_high_contrast(&latte, hc);
        let outline = "box-shadow: inset 0 0 0 1px rgba(76, 79, 105, 0.4) !important;";
        assert!(css.contains(&format!(
            ".recoolor-high-contrast .button,\n.recoolor-high-contrast .icon-button {{ {outline} }}"
        )));
        assert!(css.contains(
            ".recoolor-high-contrast .button:focus { box-shadow: inset 0 0 0 2px #8839ef !important; }"
        ));
        assert!(css.contains(
            ".recoolor-high-contrast .popup-menu-content { border: 1px solid rgba(76, 79, 105, 0.5) !important; }"
        ));
        assert!(!css.contains("overview-tile"));
    }

    #[test]
    fn switcher_titles_take_the_palettes_text() {
        for id in ["rose-pine-moon", "catppuccin-latte"] {
            let p = palette(id);
            let rule = switcher_labels(&p);
            assert_eq!(
                rule,
                format!(
                    ".switcher-list .item-box StLabel {{ color: {} !important; }}",
                    p.colors.text
                )
            );
            assert!(shell(&p, "").contains(&rule));
        }
    }

    #[test]
    fn searchlight_takes_the_palette() {
        let latte = palette("catppuccin-latte");
        let css = searchlight(&latte, None);
        assert!(css.contains(
            "#searchLight #searchLightBox { background-color: rgba(239, 241, 245, 0.65); }"
        ));
        assert!(css.contains("#searchLight #searchLightBox * { color: #4c4f69 !important; }"));
        // As see-through as Blur my Shell's popups in that style.
        let bms = ".bms-popup-surface-dark .popup-menu-content { background: rgba(30, 30, 33, 0.4) !important; }";
        assert!(
            searchlight(&palette("catppuccin-mocha"), Some(bms)).contains("rgba(30, 30, 46, 0.4)")
        );
    }

    #[test]
    fn ignores_themes_appended_by_gdm_settings() {
        let mocha = palette("catppuccin-mocha");
        let original = "#panel { color: #fafafb; }\n\n";
        let appended = format!(
            "{original}/* Generated by Recoolor: palette=nord */\n#panel {{ color: #9ccfd8; }}\n"
        );
        assert_eq!(shell(&mocha, &appended), shell(&mocha, original));
    }

    #[test]
    fn maps_ramp_ends_accent_and_alpha() {
        let mocha = palette("catppuccin-mocha");
        let css = shell(
            &mocha,
            "#panel { background-color: #000000; color: #fafafb; }\n\
             .a { color: -st-accent-color; border-color: rgba(250, 250, 251, 0.4); }\n\
             .b { color: -st-accent-fg-color; }\n",
        );
        assert_eq!(applied_palette_id(&css), Some("catppuccin-mocha"));
        assert!(css.contains("#panel { background-color: #11111b; color: #cdd6f4; }"));
        assert!(css.contains(".a { color: #cba6f7; border-color: rgba(205, 214, 244, 0.4); }"));
        assert!(css.contains(".b { color: #11111b; }"));
    }

    #[test]
    fn leaves_selectors_comments_and_urls_alone() {
        let mocha = palette("catppuccin-mocha");
        let source = "#add:hover, .white { color: #c01c28; } /* #ffffff */\n\
                      .c { background-image: url(\"resource:///x/#bad.svg\"); }\n";
        let out = recolor(&mocha, source);
        assert!(out.starts_with("#add:hover, .white { color: #f38ba8; } /* #ffffff */"));
        assert!(out.contains("url(\"resource:///x/#bad.svg\")"));
    }

    #[test]
    fn maps_colors_inside_st_functions_and_named_colors() {
        let latte = palette("catppuccin-latte");
        let out = recolor(
            &latte,
            ".d { color: st-mix(#fafafb, #222226, 9%); background: white; }\n\
             .e { color: white; background-color: whitesmoke; }",
        );
        // White cards take the surface (#ccd0da); white text stays near white.
        assert_eq!(
            out,
            ".d { color: st-mix(#eff1f5, #4c4f69, 9%); background: #ccd0da; }\n\
             .e { color: #f7f8fa; background-color: #ebedf2; }"
        );
    }

    #[test]
    fn popovers_and_cards_take_the_palettes_own_colors() {
        let hex = |h: &str| Color::from_hex(h).unwrap();
        for id in ["catppuccin-mocha", "everforest-dark"] {
            let p = palette(id);
            assert_eq!(map(hex("#36363a"), &p, false), p.colors.base);
            assert_eq!(map(hex("#47474c"), &p, false), p.colors.surface);
        }
        let latte = palette("catppuccin-latte");
        assert_eq!(map(hex("#fafafb"), &latte, false), latte.colors.base);
        assert_eq!(map(WHITE, &latte, false), latte.colors.surface);
    }

    #[test]
    fn adwaita_accents_land_on_matching_palette_colors() {
        let mocha = palette("catppuccin-mocha");
        let c = &mocha.colors;
        let hex = |s: &str| Color::from_hex(s).unwrap();
        assert_eq!(map(hex("#cd9309"), &mocha, false), c.yellow);
        assert_eq!(map(hex("#ff7800"), &mocha, false), c.orange);
        assert_eq!(map(hex("#c01c28"), &mocha, false), c.red);
        assert_eq!(map(hex("#3584e4"), &mocha, false), c.blue);
        assert_eq!(map(hex("#2ec27e"), &mocha, false), c.green);
    }
}
