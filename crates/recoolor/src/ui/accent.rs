//! The accent picker: GNOME's nine named accents (plus the palette's own), each
//! drawn in the palette color it stands for. It writes GNOME's `accent-color`,
//! the same setting Settings → Appearance shows, so the two stay in sync.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::gio;
use recoolor_core::{Color, Palette};

use crate::desktop;

/// `None` is the palette's own accent (GNOME's setting left at its default).
const CHOICES: [(Option<&str>, &str); 10] = [
    (None, "Palette's Own"),
    (Some("blue"), "Blue"),
    (Some("teal"), "Teal"),
    (Some("green"), "Green"),
    (Some("yellow"), "Yellow"),
    (Some("orange"), "Orange"),
    (Some("red"), "Red"),
    (Some("pink"), "Pink"),
    (Some("purple"), "Purple"),
    (Some("slate"), "Slate"),
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
        let colors = Rc::new(RefCell::new(vec![
            Color::rgb(0x80, 0x80, 0x80);
            CHOICES.len()
        ]));
        let row = gtk::Box::builder()
            .spacing(4)
            .halign(gtk::Align::Center)
            .build();
        let mut buttons: Vec<gtk::ToggleButton> = Vec::new();
        for (i, (name, label)) in CHOICES.into_iter().enumerate() {
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
        let i = CHOICES.iter().position(|(n, _)| *n == choice).unwrap_or(0);
        self.buttons[i].set_active(true);
    }

    /// Draws the swatches in `palette`'s colors.
    pub fn set_palette(&self, palette: &Palette) {
        *self.colors.borrow_mut() = CHOICES
            .iter()
            .map(|(name, _)| palette.with_accent(*name).colors.accent)
            .collect();
        for button in &self.buttons {
            if let Some(swatch) = button.child() {
                swatch.queue_draw();
            }
        }
    }
}
