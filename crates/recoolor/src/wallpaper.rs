//! Decoding and saving wallpapers through gdk-pixbuf, which has whatever image
//! loaders the system has (JPEG XL, AVIF, WebP, SVG). The tint itself is in core.

use std::path::{Path, PathBuf};

use gtk::gdk_pixbuf::{Colorspace, Pixbuf};
use recoolor_core::Palette;
use recoolor_core::wallpaper::{self, Lut, Tint};

/// The image to use for `source` under `palette`: the source itself without a
/// tint, else a tinted copy in Recoolor's folder, made once and reused.
pub fn render(
    data_home: &Path,
    source: &Path,
    palette: &Palette,
    tint: Tint,
    strength: u8,
) -> Result<PathBuf, String> {
    if tint == Tint::None || strength == 0 {
        return Ok(source.to_owned());
    }
    let stem = wallpaper::output_stem(data_home, source, palette, tint, strength)
        .ok_or_else(|| format!("cannot read {}", source.display()))?;
    for ext in ["jpg", "png"] {
        let done = stem.with_extension(ext);
        if done.is_file() {
            return Ok(done);
        }
    }

    let is = |want: &str| {
        source
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case(want))
    };
    // At most 4K: GNOME scales bigger images down anyway, and every extra pixel costs
    // time to tint and save (a 6K HEIC has 2.4 times the pixels). SVGs have no pixel
    // size of their own, so they render at 4K too.
    // ponytail: fixed at 4K; a 5K or 8K screen would want its own size.
    let large = Pixbuf::file_info(source).is_none_or(|(_, w, h)| w > 3840 || h > 2160);
    let image = if is("svg") || large {
        Pixbuf::from_file_at_scale(source, 3840, 2160, true)
    } else {
        Pixbuf::from_file(source)
    }
    .map_err(|e| format!("cannot open {}: {e}", source.display()))?;

    let (width, height, rowstride) = (image.width(), image.height(), image.rowstride());
    let mut pixels = image.read_pixel_bytes().to_vec();
    Lut::new(palette, tint, strength).apply(
        &mut pixels,
        usize::try_from(width).unwrap_or(0),
        usize::try_from(rowstride).unwrap_or(0),
        usize::try_from(image.n_channels()).unwrap_or(3),
    );
    let tinted = Pixbuf::from_mut_slice(
        pixels,
        Colorspace::Rgb,
        image.has_alpha(),
        8,
        width,
        height,
        rowstride,
    );

    // Flat art bands in JPEG, and JPEG has no transparency.
    let png = is("png") || is("svg") || image.has_alpha();
    let out = stem.with_extension(if png { "png" } else { "jpg" });
    std::fs::create_dir_all(wallpaper::dir(data_home)).map_err(|e| e.to_string())?;
    let saved = if png {
        tinted.savev(&out, "png", &[])
    } else {
        tinted.savev(&out, "jpeg", &[("quality", "92")])
    };
    saved.map_err(|e| format!("cannot save {}: {e}", out.display()))?;
    Ok(out)
}
