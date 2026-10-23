//! Slack keeps its theme in the user's Slack account, so Recoolor can only offer the
//! colors to enter by hand, in the four fields of Slack's custom theme.

use crate::color::Color;
use crate::palette::Palette;

/// Slack's custom theme fields, in Slack's order.
pub fn colors(palette: &Palette) -> [(&'static str, Color); 4] {
    let c = &palette.colors;
    [
        ("System navigation", c.mantle),
        ("Selected items", c.accent),
        ("Presence indication", c.green),
        ("Notifications", c.red),
    ]
}

/// The four colors comma-separated, as Slack shares a custom theme.
pub fn theme_string(palette: &Palette) -> String {
    colors(palette)
        .map(|(_, color)| color.to_string())
        .join(",")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::bundled;

    #[test]
    fn four_colors_in_slacks_order() {
        let mocha = bundled()
            .into_iter()
            .find(|p| p.id == "catppuccin-mocha")
            .unwrap();
        assert_eq!(theme_string(&mocha), "#181825,#cba6f7,#a6e3a1,#f38ba8");
    }
}
