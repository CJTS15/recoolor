//! The accent picker: GNOME's nine named accents (plus the palette's own), each
//! drawn in the palette color it stands for. It writes GNOME's `accent-color`,
//! the same setting Settings → Appearance shows, so the two stay in sync.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::gio;
use recoolor_core::{Color, Palette};

use crate::desktop;

/// `None` is the palette's own accent (GNOME's setting left at its default). The
/// colors are GNOME's own, shown until a palette is in view; GNOME's default is blue.
const CHOICES: [(Option<&str>, &str, Color); 10] = [
    (None, "Palette's Own", Color::rgb(0x35, 0x84, 0xe4)),
    (Some("blue"), "Blue", Color::rgb(0x35, 0x84, 0xe4)),
    (Some("teal"), "Teal", Color::rgb(0x21, 0x90, 0xa4)),
    (Some("green"), "Green", Color::rgb(0x3a, 0x94, 0x4a)),
    (Some("yellow"), "Yellow", Color::rgb(0xc8, 0x88, 0x00)),
    (Some("orange"), "Orange", Color::rgb(0xed, 0x5b, 0x00)),
    (Some("red"), "Red", Color::rgb(0xe6, 0x2d, 0x42)),
    (Some("pink"), "Pink", Color::rgb(0xd5, 0x61, 0x99)),
    (Some("purple"), "Purple", Color::rgb(0x91, 0x41, 0xac)),
    (Some("slate"), "Slate", Color::rgb(0x6f, 0x83, 0x96)),
];

pub struct AccentRow {
    pub group: adw::PreferencesGroup,
    buttons: Vec<gtk::ToggleButton>,
    colors: Rc<RefCell<Vec<Color>>>,
}

impl AccentRow {
    /// `None` on GNOME before 47, which has no accent setting.
    pub fn new(interface: &gio::Settings) -> Option<Self> {
        interface
            .settings_schema()?
            .has_key("accent-color")
            .then_some(())?;
        let colors = Rc::new(RefCell::new(
            CHOICES.iter().map(|c| c.2).collect::<Vec<_>>(),
        ));
        let row = gtk::Box::builder()
            .spacing(4)
            .halign(gtk::Align::Center)
            .build();
        let mut buttons: Vec<gtk::ToggleButton> = Vec::new();
        for (i, (name, label, _)) in CHOICES.into_iter().enumerate() {
            let swatch = gtk::DrawingArea::builder()
                .content_width(20)
                .content_height(20)
                .build();
            let shared = colors.clone();
            swatch.set_draw_func(move |_, cr, w, h| {
                let c = shared.borrow()[i];
                let channel = |v: u8| f64::from(v) / 255.0;
                cr.set_source_rgb(channel(c.r), channel(c.g), channel(c.b));
                let r = f64::from(w.min(h)) / 2.0;
                cr.arc(
                    f64::from(w) / 2.0,
                    f64::from(h) / 2.0,
                    r,
                    0.0,
                    std::f64::consts::TAU,
                );
                let _ = cr.fill();
            });
            let button = gtk::ToggleButton::builder()
                .child(&swatch)
                .tooltip_text(label)
                .css_classes(["circular", "flat", "accent-swatch"])
                .build();
            button.update_property(&[gtk::accessible::Property::Label(label)]);
            if let Some(first) = buttons.first() {
                button.set_group(Some(first));
            }
            let interface = interface.clone();
            button.connect_toggled(move |b| {
                // Only for a real pick: re-selecting after an outside change is a no-op.
                if !b.is_active() || desktop::accent_choice(&interface).as_deref() == name {
                    return;
                }
                let result = match name {
                    Some(name) => desktop::set_string(&interface, "accent-color", name),
                    None => {
                        interface.reset("accent-color");
                        gio::Settings::sync();
                        Ok(())
                    }
                };
                if let Err(e) = result {
                    crate::log::error(&format!("accent: {e}"));
                }
            });
            row.append(&button);
            buttons.push(button);
        }

        let group = adw::PreferencesGroup::builder()
            .title("Accent Color")
            .description("The palette's color of each name. Also sets the accent in Settings.")
            .build();
        group.add(&row);
        let this = Self {
            group,
            buttons,
            colors,
        };
        this.select(desktop::accent_choice(interface).as_deref());
        Some(this)
    }

    /// Marks the current choice, e.g. after it changed in Settings.
    pub fn select(&self, choice: Option<&str>) {
        let i = CHOICES.iter().position(|(n, ..)| *n == choice).unwrap_or(0);
        self.buttons[i].set_active(true);
    }

    /// Draws the swatches in `palette`'s colors, or GNOME's without one.
    pub fn set_palette(&self, palette: Option<&Palette>) {
        *self.colors.borrow_mut() = CHOICES
            .iter()
            .map(|(name, _, gnome)| palette.map_or(*gnome, |p| p.with_accent(*name).colors.accent))
            .collect();
        for button in &self.buttons {
            if let Some(swatch) = button.child() {
                swatch.queue_draw();
            }
        }
    }
}
