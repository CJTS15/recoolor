//! GTK3.
//!
//! GTK3's Adwaita compiles most colors into its selectors instead of reading named
//! colors, so redefining the names alone recolors little. This file sets the names
//! and then restyles the common widgets directly. Coverage is good for typical apps
//! but not complete; a full GTK3 theme (the upstream installer) covers the rest.

use super::{Roles, header};
use crate::palette::Palette;

/// GTK3 under the adw-gtk3 theme, which draws from libadwaita's named colors like
/// GTK4 does: just those colors, without the widget rules `gtk3` needs to recolor
/// stock Adwaita (they would fight adw-gtk3's own look).
pub fn gtk3_adw(palette: &Palette) -> String {
    let c = &palette.colors;
    header(&palette.id)
        + &super::gtk4::define_colors(palette)
        + &format!(
            "@define-color panel_bg_color {};\n@define-color panel_fg_color {};\n",
            c.crust, c.text
        )
}

pub fn gtk3(palette: &Palette) -> String {
    let c = &palette.colors;
    let r = Roles::new(palette);
    let shade = r.shade.alpha(r.shade_alpha);
    let selected_bg = c.accent;
    let selected_fg = r.accent_fg;
    let disabled_fg = c.text.mix(c.base, 0.5);
    let (base, mantle, crust, surface, text) = (c.base, c.mantle, c.crust, c.surface, c.text);
    // GTK3 uses these three as text (warning and error labels, entries).
    let dark = palette.variant == crate::palette::Variant::Dark;
    let (warning, error, success) = (
        c.yellow.standalone(dark),
        c.red.standalone(dark),
        c.green.standalone(dark),
    );
    let (accent, subtext, overlay, red, yellow) = (c.accent, c.subtext, c.overlay, c.red, c.yellow);
    let (hover, active, border) = (r.hover, r.active, r.border);
    let destructive_fg = r.destructive_fg;
    let accent_fg = r.accent_fg;

    let mut css = header(&palette.id);
    css.push_str(&format!(
        "\
@define-color theme_bg_color {base};
@define-color theme_fg_color {text};
@define-color theme_base_color {mantle};
@define-color theme_text_color {text};
@define-color theme_selected_bg_color {selected_bg};
@define-color theme_selected_fg_color {selected_fg};
@define-color theme_unfocused_bg_color {base};
@define-color theme_unfocused_fg_color {text};
@define-color theme_unfocused_base_color {mantle};
@define-color theme_unfocused_text_color {text};
@define-color theme_unfocused_selected_bg_color {selected_bg};
@define-color theme_unfocused_selected_fg_color {selected_fg};
@define-color insensitive_bg_color {base};
@define-color insensitive_fg_color {disabled_fg};
@define-color insensitive_base_color {mantle};
@define-color borders {border};
@define-color unfocused_borders {border};
@define-color warning_color {warning};
@define-color error_color {error};
@define-color success_color {success};
@define-color wm_title {text};
@define-color wm_unfocused_title {subtext};
@define-color wm_highlight transparent;
@define-color wm_bg {mantle};
@define-color wm_unfocused_bg {base};
@define-color content_view_bg {mantle};

window, .background, dialog, messagedialog .dialog-action-box {{
  background-color: {base};
  color: {text};
}}

headerbar, .titlebar:not(headerbar), .titlebar headerbar {{
  background: {mantle};
  color: {text};
  border-color: {crust};
  box-shadow: inset 0 -1px {shade};
}}
headerbar:backdrop, .titlebar:backdrop {{
  background: {base};
  color: {subtext};
}}

.view, textview text, iconview, treeview.view, list, row, .content-view {{
  background-color: {mantle};
  color: {text};
}}
treeview.view header button {{
  background: {base};
  color: {subtext};
  border-color: {border};
}}
row:hover, treeview.view:hover {{
  background-color: {hover};
}}
.view:selected, treeview.view:selected, row:selected, *:selected, selection,
iconview:selected, textview text selection {{
  background-color: {selected_bg};
  color: {selected_fg};
}}
.sidebar, placessidebar list, stacksidebar list, .navigation-sidebar {{
  background-color: {mantle};
  border-color: {border};
}}

entry, spinbutton:not(.vertical), searchbar entry {{
  background: {crust};
  color: {text};
  border-color: {border};
  box-shadow: none;
}}
entry:focus, spinbutton:focus {{
  border-color: {accent};
  box-shadow: inset 0 0 0 1px {accent};
}}
entry placeholder, entry .dim-label, .dim-label {{
  color: {overlay};
}}

button, combobox button, menubutton button {{
  background-image: none;
  background-color: {surface};
  color: {text};
  border-color: {border};
  box-shadow: none;
  text-shadow: none;
  -gtk-icon-shadow: none;
}}
button:hover {{
  background-color: {hover};
}}
button:active, button:checked {{
  background-color: {active};
}}
button:disabled {{
  background-color: {base};
  color: {disabled_fg};
}}
button.flat, headerbar button.titlebutton {{
  background-color: transparent;
  border-color: transparent;
}}
button.flat:hover, headerbar button.titlebutton:hover {{
  background-color: {hover};
}}
button.suggested-action {{
  background-color: {accent};
  color: {accent_fg};
  border-color: {accent};
}}
button.destructive-action {{
  background-color: {red};
  color: {destructive_fg};
  border-color: {red};
}}

check:checked, radio:checked, check:indeterminate, radio:indeterminate {{
  background-image: none;
  background-color: {accent};
  color: {accent_fg};
  border-color: {accent};
}}
switch {{
  background-image: none;
  background-color: {surface};
  border-color: {border};
}}
switch:checked {{
  background-color: {accent};
  border-color: {accent};
}}
switch slider {{
  background-image: none;
  background-color: {text};
  border-color: {border};
}}

scale trough, progressbar trough, levelbar trough {{
  background-image: none;
  background-color: {surface};
  border-color: {border};
}}
scale highlight, progressbar progress, levelbar block.filled {{
  background-image: none;
  background-color: {accent};
  border-color: {accent};
}}
scale slider {{
  background-image: none;
  background-color: {text};
  border-color: {border};
}}

notebook > header {{
  background-color: {mantle};
  border-color: {border};
}}
notebook > header tab {{
  color: {subtext};
}}
notebook > header tab:checked {{
  color: {text};
  box-shadow: inset 0 -3px {accent};
}}
notebook > stack:not(:only-child) {{
  background-color: {base};
}}

menu, .menu, .context-menu, popover.background, popover.menu {{
  background-color: {surface};
  color: {text};
  border-color: {border};
}}
menu menuitem:hover, popover modelbutton.flat:hover, menubar > menuitem:hover {{
  background-color: {hover};
  color: {text};
}}
menubar {{
  background-color: {mantle};
  color: {text};
  box-shadow: none;
}}

tooltip, tooltip.background {{
  background-color: {crust};
  color: {text};
  border-color: {border};
}}
tooltip * {{
  color: {text};
}}

infobar.info > revealer > box {{
  background-color: {surface};
  color: {text};
}}
infobar.warning > revealer > box {{
  background-color: {yellow};
  color: {crust};
}}
infobar.error > revealer > box {{
  background-color: {red};
  color: {destructive_fg};
}}

separator {{
  background-color: {border};
}}
scrollbar {{
  background-color: {mantle};
  border-color: {border};
}}
scrollbar slider {{
  background-color: {overlay};
}}
scrollbar slider:hover {{
  background-color: {subtext};
}}

*:link, link, button.link {{
  color: {accent};
}}
"
    ));
    css
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::bundled;

    #[test]
    fn dracula_sets_named_and_widget_colors() {
        let dracula = bundled().into_iter().find(|p| p.id == "dracula").unwrap();
        let css = gtk3(&dracula);
        assert!(css.contains("@define-color theme_bg_color #282a36;"));
        assert!(css.contains("@define-color theme_selected_bg_color #bd93f9;"));
        assert!(css.contains("background-color: #bd93f9;"));
    }
}
