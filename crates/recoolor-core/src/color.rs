//! sRGB colors parsed from `#rrggbb` hex strings.

use std::fmt;

use serde::Deserialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[derive(Debug, thiserror::Error)]
#[error("invalid color {0:?}: expected #rrggbb")]
pub struct ParseColorError(String);

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub fn from_hex(s: &str) -> Result<Self, ParseColorError> {
        let err = || ParseColorError(s.to_owned());
        let hex = s.strip_prefix('#').ok_or_else(err)?;
        if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(err());
        }
        let channel = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| err());
        Ok(Self::rgb(channel(0)?, channel(2)?, channel(4)?))
    }

    /// Linear interpolation towards `other`: `t = 0.0` is `self`, `t = 1.0` is `other`.
    pub fn mix(self, other: Color, t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);
        let lerp = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * t).round() as u8;
        Color::rgb(
            lerp(self.r, other.r),
            lerp(self.g, other.g),
            lerp(self.b, other.b),
        )
    }

    /// CSS `rgba()` with the given alpha.
    pub fn alpha(self, a: f32) -> String {
        format!(
            "rgba({}, {}, {}, {})",
            self.r,
            self.g,
            self.b,
            a.clamp(0.0, 1.0)
        )
    }

    /// WCAG relative luminance.
    pub fn luminance(self) -> f32 {
        let lin = |c: u8| {
            let c = f32::from(c) / 255.0;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * lin(self.r) + 0.7152 * lin(self.g) + 0.0722 * lin(self.b)
    }

    /// WCAG contrast ratio, from 1.0 to 21.0.
    pub fn contrast(self, other: Color) -> f32 {
        let (a, b) = (self.luminance(), other.luminance());
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    /// OKLab `[L, a, b]`, the perceptual space the wallpaper tint works in.
    pub fn to_oklab(self) -> [f32; 3] {
        oklab_from_rgb([self.r, self.g, self.b].map(|c| f32::from(c) / 255.0))
    }

    /// libadwaita's "standalone" form of a fill color, for using it as text on the
    /// window: OKLab lightness at most 0.5 in light style, at least 0.85 in dark
    /// (its `--standalone-color-oklab`).
    pub fn standalone(self, dark: bool) -> Color {
        let [l, a, b] = self.to_oklab();
        let l = if dark { l.max(0.85) } else { l.min(0.5) };
        let [r, g, b] = rgb_from_oklab([l, a, b]).map(|v| (v * 255.0).round() as u8);
        Color::rgb(r, g, b)
    }

    /// Hue in degrees, or `None` for a grey (channels within 24 of each other).
    pub fn hue(self) -> Option<f32> {
        let (r, g, b) = (i32::from(self.r), i32::from(self.g), i32::from(self.b));
        let (max, min) = (r.max(g).max(b), r.min(g).min(b));
        if max - min <= 24 {
            return None;
        }
        let d = (max - min) as f32;
        let h = if max == r {
            ((g - b) as f32 / d).rem_euclid(6.0)
        } else if max == g {
            (b - r) as f32 / d + 2.0
        } else {
            (r - g) as f32 / d + 4.0
        };
        Some(h * 60.0)
    }

    /// The candidate that reads best on top of `self`.
    pub fn best_foreground(self, candidates: &[Color]) -> Color {
        candidates
            .iter()
            .copied()
            .max_by(|a, b| self.contrast(*a).total_cmp(&self.contrast(*b)))
            .unwrap_or(Color::rgb(255, 255, 255))
    }
}

impl TryFrom<String> for Color {
    type Error = ParseColorError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::from_hex(s.trim())
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_formats_hex() {
        let c = Color::from_hex("#1E1e2e").unwrap();
        assert_eq!(c, Color::rgb(0x1e, 0x1e, 0x2e));
        assert_eq!(c.to_string(), "#1e1e2e");
    }

    #[test]
    fn rejects_bad_hex() {
        for s in ["1e1e2e", "#1e1e2", "#1e1e2eff", "#gggggg", "#１２３"] {
            assert!(Color::from_hex(s).is_err(), "{s} should be rejected");
        }
    }

    #[test]
    fn mixes_endpoints() {
        let (a, b) = (Color::rgb(0, 0, 0), Color::rgb(255, 255, 255));
        assert_eq!(a.mix(b, 0.0), a);
        assert_eq!(a.mix(b, 1.0), b);
        assert_eq!(a.mix(b, 0.5), Color::rgb(128, 128, 128));
    }

    #[test]
    fn contrast_of_black_on_white_is_21() {
        let ratio = Color::rgb(0, 0, 0).contrast(Color::rgb(255, 255, 255));
        assert!((ratio - 21.0).abs() < 0.01);
    }
}

fn to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn from_linear(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// sRGB (0 to 1 per channel) to OKLab.
pub fn oklab_from_rgb(rgb: [f32; 3]) -> [f32; 3] {
    let [r, g, b] = rgb.map(to_linear);
    let l = (0.412_221_47 * r + 0.536_332_55 * g + 0.051_445_99 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    [
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    ]
}

/// OKLab to sRGB (0 to 1 per channel), clamped into gamut.
pub fn rgb_from_oklab(lab: [f32; 3]) -> [f32; 3] {
    let [l, a, b] = lab;
    let l_ = (l + 0.396_337_78 * a + 0.215_803_76 * b).powi(3);
    let m_ = (l - 0.105_561_346 * a - 0.063_854_17 * b).powi(3);
    let s_ = (l - 0.089_484_18 * a - 1.291_485_5 * b).powi(3);
    [
        4.076_741_7 * l_ - 3.307_711_6 * m_ + 0.230_969_94 * s_,
        -1.268_438 * l_ + 2.609_757_4 * m_ - 0.341_319_38 * s_,
        -0.004_196_086_3 * l_ - 0.703_418_6 * m_ + 1.707_614_7 * s_,
    ]
    .map(|c| from_linear(c.clamp(0.0, 1.0)))
}

#[cfg(test)]
mod oklab_tests {
    use super::*;

    #[test]
    fn round_trips_and_has_known_whites() {
        let white = Color::rgb(255, 255, 255).to_oklab();
        assert!((white[0] - 1.0).abs() < 1e-3 && white[1].abs() < 1e-3);
        for c in [Color::rgb(0x1e, 0x1e, 0x2e), Color::rgb(0xcb, 0xa6, 0xf7)] {
            let back = rgb_from_oklab(c.to_oklab()).map(|v| (v * 255.0).round() as u8);
            assert_eq!(back, [c.r, c.g, c.b]);
        }
    }
}
