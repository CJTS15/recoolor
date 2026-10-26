//! Wallpapers recolored to a palette. The pixel work is here (raw RGB(A) bytes in,
//! the same out); decoding and encoding images is the app's job.
//!
//! Images cannot carry a text marker, so Recoolor's folder is the marker:
//! everything in `$XDG_DATA_HOME/recoolor/wallpapers/` is Recoolor's.

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::color::{Color, oklab_from_rgb, rgb_from_oklab};
use crate::palette::Palette;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tint {
    /// Greys follow the palette's tones, colors move to its nearest hue.
    #[default]
    Match,
    /// A gradient map through the palette's tones (and accent).
    Mono,
    /// The image as it is.
    None,
}

pub fn dir(data_home: &Path) -> PathBuf {
    data_home.join("recoolor/wallpapers")
}

pub fn is_ours(path: &Path, data_home: &Path) -> bool {
    path.starts_with(dir(data_home))
}

/// Where a tinted copy of `source` goes, without the extension (the app picks JPEG
/// for photos, PNG for flat art or transparency). The name changes with anything
/// that changes the result (the source file itself, palette, tint, strength),
/// because the Shell caches backgrounds by file name.
pub fn output_stem(
    data_home: &Path,
    source: &Path,
    palette: &Palette,
    tint: Tint,
    strength: u8,
) -> Option<PathBuf> {
    let meta = fs::metadata(source).ok()?;
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    meta.len().hash(&mut hasher);
    meta.modified().ok().hash(&mut hasher);
    let mut colors = palette.colors.clone();
    if tint != Tint::Mono {
        // Only Mono uses the accent, so an accent change needs no new file.
        colors.accent = colors.base;
    }
    format!("{colors:?}").hash(&mut hasher);
    (tint, strength).hash(&mut hasher);
    Some(dir(data_home).join(format!("{:016x}-{}", hasher.finish(), palette.id)))
}

/// Deletes Recoolor's wallpapers except `keep` (the ones in use).
pub fn prune(data_home: &Path, keep: &[&Path]) {
    let Ok(entries) = fs::read_dir(dir(data_home)) else {
        return;
    };
    for path in entries.filter_map(|e| e.ok().map(|e| e.path())) {
        if !keep.contains(&path.as_path()) {
            let _ = fs::remove_file(path);
        }
    }
}

const GRID: usize = 33;

/// A 3D lookup table of the tint, built once per palette, tint and strength.
pub struct Lut {
    data: Vec<[f32; 3]>,
}

impl Lut {
    pub fn new(palette: &Palette, tint: Tint, strength: u8) -> Self {
        let c = &palette.colors;
        let lab = |c: Color| c.to_oklab();
        let ramp = |colors: &[Color]| {
            let mut stops: Vec<[f32; 3]> = colors.iter().map(|&c| lab(c)).collect();
            stops.sort_by(|a, b| a[0].total_cmp(&b[0]));
            stops
        };
        let tones = ramp(&[
            c.crust, c.mantle, c.base, c.surface, c.overlay, c.subtext, c.text,
        ]);
        let mono = ramp(&[
            c.crust, c.base, c.surface, c.overlay, c.accent, c.subtext, c.text,
        ]);
        let strength = f32::from(strength.min(100)) / 100.0;

        let step = 1.0 / (GRID - 1) as f32;
        let mut data = Vec::with_capacity(GRID * GRID * GRID);
        for r in 0..GRID {
            for g in 0..GRID {
                for b in 0..GRID {
                    let rgb = [r as f32 * step, g as f32 * step, b as f32 * step];
                    let pixel = oklab_from_rgb(rgb);
                    let tinted = match tint {
                        Tint::None => pixel,
                        Tint::Match => matched(pixel, rgb, &tones, palette),
                        Tint::Mono => along(&mono, pixel[0]),
                    };
                    data.push(rgb_from_oklab(lerp(pixel, tinted, strength)));
                }
            }
        }
        Self { data }
    }

    fn at(&self, r: usize, g: usize, b: usize) -> [f32; 3] {
        self.data[(r * GRID + g) * GRID + b]
    }

    /// Looks up one sRGB color (0 to 1 per channel), trilinearly.
    fn lookup(&self, rgb: [f32; 3]) -> [f32; 3] {
        let scaled = rgb.map(|v| v.clamp(0.0, 1.0) * (GRID - 1) as f32);
        let lo = scaled.map(|v| (v as usize).min(GRID - 2));
        let t = [0, 1, 2].map(|i| scaled[i] - lo[i] as f32);
        let mut out = [0.0; 3];
        for (dr, wr) in [(0, 1.0 - t[0]), (1, t[0])] {
            for (dg, wg) in [(0, 1.0 - t[1]), (1, t[1])] {
                for (db, wb) in [(0, 1.0 - t[2]), (1, t[2])] {
                    let v = self.at(lo[0] + dr, lo[1] + dg, lo[2] + db);
                    let w = wr * wg * wb;
                    for i in 0..3 {
                        out[i] += v[i] * w;
                    }
                }
            }
        }
        out
    }

    /// Tints pixel rows in place. `channels` is 3 (RGB) or 4 (RGBA; alpha kept);
    /// `rowstride` may include padding, which is left alone.
    pub fn apply(&self, pixels: &mut [u8], width: usize, rowstride: usize, channels: usize) {
        for row in pixels.chunks_mut(rowstride) {
            let len = (width * channels).min(row.len());
            for px in row[..len].chunks_exact_mut(channels) {
                let rgb = [px[0], px[1], px[2]].map(|v| f32::from(v) / 255.0);
                let out = self.lookup(rgb);
                for i in 0..3 {
                    px[i] = (out[i] * 255.0).round() as u8;
                }
            }
        }
    }
}

fn lerp(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * t)
}

/// The color at lightness `l` (0 to 1) along stops sorted dark to light.
fn along(stops: &[[f32; 3]], l: f32) -> [f32; 3] {
    let pos = l.clamp(0.0, 1.0) * (stops.len() - 1) as f32;
    let i = (pos as usize).min(stops.len() - 2);
    lerp(stops[i], stops[i + 1], pos - i as f32)
}

/// Match: the palette tone at the pixel's lightness, pulled toward the palette
/// color for its hue (the Shell's table) as much as the pixel itself is colored.
fn matched(pixel: [f32; 3], rgb: [f32; 3], tones: &[[f32; 3]], palette: &Palette) -> [f32; 3] {
    let tone = along(tones, pixel[0]);
    let chroma = pixel[1].hypot(pixel[2]);
    let weight = (chroma / 0.12).clamp(0.0, 1.0);
    if weight == 0.0 {
        return tone;
    }
    let [r, g, b] = rgb.map(|v| (v * 255.0).round() as u8);
    let Some(hue) = Color::rgb(r, g, b).hue() else {
        return tone;
    };
    let target = palette.hue_color(hue).to_oklab();
    // The target's hue, at no more than the pixel's own chroma.
    let target_chroma = target[1].hypot(target[2]).max(1e-6);
    let scale = (chroma / target_chroma).min(1.0);
    let colored = [tone[0], target[1] * scale, target[2] * scale];
    lerp(tone, colored, weight)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::bundled;

    fn palette(id: &str) -> Palette {
        bundled().into_iter().find(|p| p.id == id).unwrap()
    }

    fn tint_one(lut: &Lut, rgb: [u8; 3]) -> [u8; 3] {
        let mut px = rgb.to_vec();
        lut.apply(&mut px, 1, 3, 3);
        [px[0], px[1], px[2]]
    }

    #[test]
    fn match_maps_greys_to_tones_and_colors_to_palette_hues() {
        let mocha = palette("catppuccin-mocha");
        let lut = Lut::new(&mocha, Tint::Match, 100);
        // Black and white land on the palette's darkest and lightest tones.
        let dark = tint_one(&lut, [0, 0, 0]);
        let light = tint_one(&lut, [255, 255, 255]);
        let near = |a: [u8; 3], c: Color| {
            a.iter()
                .zip([c.r, c.g, c.b])
                .all(|(x, y)| x.abs_diff(y) <= 3)
        };
        assert!(near(dark, mocha.colors.crust), "{dark:?}");
        assert!(near(light, mocha.colors.text), "{light:?}");
        // A pure red takes Mocha's red hue (pinkish), not its own.
        let red = tint_one(&lut, [255, 0, 0]);
        let red_hue = Color::rgb(red[0], red[1], red[2]).hue().unwrap();
        let mocha_hue = mocha.colors.red.hue().unwrap();
        assert!(
            (red_hue - mocha_hue).abs() < 15.0,
            "{red_hue} vs {mocha_hue}"
        );
    }

    #[test]
    fn strength_zero_and_none_keep_the_image() {
        let mocha = palette("catppuccin-mocha");
        for lut in [
            Lut::new(&mocha, Tint::Match, 0),
            Lut::new(&mocha, Tint::None, 100),
        ] {
            for rgb in [[12, 200, 90], [255, 128, 0], [40, 40, 40]] {
                let out = tint_one(&lut, rgb);
                assert!(
                    out.iter().zip(rgb).all(|(a, b)| a.abs_diff(b) <= 2),
                    "{rgb:?} -> {out:?}"
                );
            }
        }
    }

    #[test]
    fn apply_keeps_alpha_and_row_padding() {
        let lut = Lut::new(&palette("dracula"), Tint::Mono, 100);
        // Two RGBA pixels per row, two padding bytes, two rows.
        let mut px = vec![
            10, 20, 30, 77, 200, 100, 50, 88, 9, 9, 1, 2, 3, 99, 4, 5, 6, 66, 9, 9,
        ];
        lut.apply(&mut px, 2, 10, 4);
        assert_eq!((px[3], px[7], px[13], px[17]), (77, 88, 99, 66));
        assert_eq!((px[8], px[9], px[18], px[19]), (9, 9, 9, 9));
    }

    #[test]
    fn output_names_change_with_the_palette_but_not_the_accent_for_match() {
        let home = tempfile::tempdir().unwrap();
        let source = home.path().join("tree.png");
        fs::write(&source, b"not really a png").unwrap();
        let mocha = palette("catppuccin-mocha");
        let path = |p: &Palette, t| output_stem(home.path(), &source, p, t, 80).unwrap();
        let blue = mocha.with_accent(Some("blue"));
        assert_eq!(path(&mocha, Tint::Match), path(&blue, Tint::Match));
        assert_ne!(path(&mocha, Tint::Mono), path(&blue, Tint::Mono));
        assert_ne!(
            path(&mocha, Tint::Match),
            path(&palette("dracula"), Tint::Match)
        );
        assert!(is_ours(&path(&mocha, Tint::Match), home.path()));
    }

    /// `cargo test --release -- --ignored lut_speed` to time a 4K image.
    #[test]
    #[ignore]
    fn lut_speed() {
        let mocha = palette("catppuccin-mocha");
        let start = std::time::Instant::now();
        let lut = Lut::new(&mocha, Tint::Match, 80);
        let built = start.elapsed();
        let mut px: Vec<u8> = (0..3840 * 2160 * 3).map(|i| (i * 7 % 256) as u8).collect();
        let start = std::time::Instant::now();
        lut.apply(&mut px, 3840, 3840 * 3, 3);
        println!("lut {built:?}, 4K apply {:?}", start.elapsed());
    }
}
