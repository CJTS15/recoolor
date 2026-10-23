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
