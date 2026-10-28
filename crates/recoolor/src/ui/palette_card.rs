//! One palette in the grid: a strip of swatches, the name, a check mark when applied,
//! and a star to keep it among the favorites.

use gtk::pango;
use gtk::prelude::*;
use recoolor_core::Palette;

pub struct PaletteCard {
    pub child: gtk::FlowBoxChild,
    /// The window keeps the favorites; this only shows and toggles one.
    pub star: gtk::ToggleButton,
    check: gtk::Image,
}

impl PaletteCard {
    pub fn new(palette: &Palette) -> Self {
        let c = &palette.colors;
        let swatches = [
            c.base, c.surface, c.text, c.accent, c.red, c.yellow, c.green, c.blue,
        ];
        let strip = gtk::DrawingArea::builder().content_height(48).build();
        strip.set_draw_func(move |_, cr, width, height| {
            let step = f64::from(width) / swatches.len() as f64;
            for (i, color) in swatches.iter().enumerate() {
                let channel = |v: u8| f64::from(v) / 255.0;
                cr.set_source_rgb(channel(color.r), channel(color.g), channel(color.b));
                // The extra half pixel hides seams between neighbours.
                cr.rectangle(i as f64 * step, 0.0, step + 0.5, f64::from(height));
                let _ = cr.fill();
            }
        });

        let name = gtk::Label::builder()
            .label(&palette.name)
            .xalign(0.0)
            .ellipsize(pango::EllipsizeMode::End)
            .css_classes(["heading"])
            .build();
        // Converted Gogh themes say so; a lone one's family is its own name.
        let caption = match (
            palette.id.starts_with("gogh-"),
            palette.family == palette.name,
        ) {
            (false, _) => palette.family.clone(),
            (true, true) => "Gogh".to_owned(),
            (true, false) => format!("{} · Gogh", palette.family),
        };
        let family = gtk::Label::builder()
            .label(&caption)
            .xalign(0.0)
            .css_classes(["caption", "dim-label"])
            .build();
        let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
        text.set_hexpand(true);
        text.append(&name);
        text.append(&family);

        let check = gtk::Image::builder()
            .icon_name("object-select-symbolic")
            .tooltip_text("Applied")
            .visible(false)
            .build();
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        row.set_margin_top(10);
        row.set_margin_bottom(10);
        row.set_margin_start(12);
        row.set_margin_end(12);
        let star = gtk::ToggleButton::builder()
            .icon_name("non-starred-symbolic")
            .valign(gtk::Align::Center)
            .css_classes(["flat", "circular"])
            .build();
        let label = |on: bool| {
            if on {
                "Remove from Favorites"
            } else {
                "Add to Favorites"
            }
        };
        star.set_tooltip_text(Some(label(false)));
        star.update_property(&[gtk::accessible::Property::Label(label(false))]);
        star.connect_toggled(move |star| {
            let on = star.is_active();
            star.set_icon_name(if on {
                "starred-symbolic"
            } else {
                "non-starred-symbolic"
            });
            star.set_tooltip_text(Some(label(on)));
            star.update_property(&[gtk::accessible::Property::Label(label(on))]);
        });
        row.append(&text);
        row.append(&check);
        row.append(&star);

        let card = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .css_classes(["card"])
            .overflow(gtk::Overflow::Hidden)
            .build();
        card.append(&strip);
        card.append(&row);

        let child = gtk::FlowBoxChild::builder().child(&card).build();
        Self { child, star, check }
    }

    pub fn set_applied(&self, applied: bool) {
        self.check.set_visible(applied);
    }
}
