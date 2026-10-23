//! The main window: palette grid, preview pane, Apply and Reset.
//!
//! Selecting a card previews it on Pinta's own window, by loading the same GTK4 CSS
//! that Apply would write into a display-wide provider.

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

use adw::prelude::*;
use gtk::gdk;
use pinta_core::palette::{self, Palette, Variant};
use pinta_core::{ApplyError, CssState, GtkConfig, RestoreOutcome, Status, Toolkit, css, paths};

use super::palette_card::PaletteCard;
use super::preview;
use crate::{desktop, log};

// The window lives as long as the app, so handlers hold it strongly.
struct Window {
    window: adw::ApplicationWindow,
    toasts: adw::ToastOverlay,
    title: adw::WindowTitle,
    grid: gtk::FlowBox,
    show_all: gtk::ToggleButton,
    cards: Vec<PaletteCard>,
    palettes: Vec<Palette>,
    /// Indices into `palettes` behind the light and dark combo rows.
    light: Vec<usize>,
    dark: Vec<usize>,
    pair: adw::SwitchRow,
    light_row: adw::ComboRow,
    dark_row: adw::ComboRow,
    apply: gtk::Button,
    style: adw::StyleManager,
    preview: gtk::CssProvider,
    previewing: Cell<bool>,
    config_home: Option<PathBuf>,
}

pub fn build(app: &adw::Application) -> adw::ApplicationWindow {
    let (palettes, errors) = palette::catalog(paths::user_palettes_dir().as_deref());
    for e in &errors {
        log::error(&format!("skipped palette {e}"));
    }
    let config_home = desktop::config_home().ok();

    let title = adw::WindowTitle::new("Pinta", "");
    let apply = gtk::Button::builder()
        .label("Apply")
        .css_classes(["suggested-action"])
        .sensitive(false)
        .build();
    let reset = gtk::Button::builder()
        .label("Reset")
        .tooltip_text("Remove Pinta's styles and bring back what you had before")
        .sensitive(config_home.is_some())
        .build();
    let show_all = gtk::ToggleButton::builder()
        .label("Show All")
        .tooltip_text("Show light and dark palettes")
        .build();
    let header = adw::HeaderBar::builder().title_widget(&title).build();
    header.pack_start(&reset);
    header.pack_end(&apply);
    header.pack_end(&show_all);

    let no_config = adw::Banner::builder()
        .title(
            "Cannot find your config folder. Set HOME or XDG_CONFIG_HOME, then open Pinta again.",
        )
        .revealed(config_home.is_none())
        .build();
    let broken = adw::Banner::builder()
        .title(format!(
            "{} palette file(s) could not be loaded",
            errors.len()
        ))
        .button_label("Details")
        .revealed(!errors.is_empty())
        .build();

    let cards: Vec<PaletteCard> = palettes.iter().map(PaletteCard::new).collect();
    let grid = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::Single)
        .homogeneous(true)
        .min_children_per_line(1)
        .max_children_per_line(4)
        .row_spacing(12)
        .column_spacing(12)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .valign(gtk::Align::Start)
        .build();
    for card in &cards {
        grid.append(&card.child);
    }

    let indices = |variant| -> Vec<usize> {
        (0..palettes.len())
            .filter(|&i| palettes[i].variant == variant)
            .collect()
    };
    let (light, dark) = (indices(Variant::Light), indices(Variant::Dark));
    let combo = |title: &str, indices: &[usize], default: &str| {
        let names: Vec<&str> = indices.iter().map(|&i| palettes[i].name.as_str()).collect();
        let row = adw::ComboRow::builder()
            .title(title)
            .model(&gtk::StringList::new(&names))
            .build();
        if let Some(pos) = indices.iter().position(|&i| palettes[i].id == default) {
            row.set_selected(pos as u32);
        }
        row
    };
    let light_row = combo("Light Style", &light, "catppuccin-latte");
    let dark_row = combo("Dark Style", &dark, "catppuccin-mocha");
    let pair = adw::SwitchRow::builder()
        .title("Follow System Style")
        .subtitle("Use one palette in light style and another in dark style")
        .sensitive(!light.is_empty() && !dark.is_empty())
        .build();
    for row in [&light_row, &dark_row] {
        pair.bind_property("active", row, "sensitive")
            .sync_create()
            .build();
    }
    let pair_group = adw::PreferencesGroup::new();
    pair_group.add(&pair);
    pair_group.add(&light_row);
    pair_group.add(&dark_row);

    let side = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(24)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();
    side.append(&pair_group);
    side.append(&preview::build());

    let content = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    content.append(
        &gtk::ScrolledWindow::builder()
            .child(&grid)
            .hexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build(),
    );
    content.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    content.append(
        &gtk::ScrolledWindow::builder()
            .child(&side)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .width_request(380)
            .hexpand(false)
            .build(),
    );
    let toasts = adw::ToastOverlay::new();
    toasts.set_child(Some(&content));

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.add_top_bar(&no_config);
    toolbar.add_top_bar(&broken);
    toolbar.set_content(Some(&toasts));

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Pinta")
        .default_width(1060)
        .default_height(700)
        .content(&toolbar)
        .build();
    // A single-selection FlowBox selects whatever has focus, which would start a
    // preview before the user picks anything.
    GtkWindowExt::set_focus(&window, Some(&show_all));

    let preview = gtk::CssProvider::new();
    // A new provider evaluates `prefers-color-scheme` as light (GTK 4.20+); follow
    // the desktop instead, so a pair previews its dark half in dark style.
    if preview.find_property("prefers-color-scheme").is_some() {
        if let Some(settings) = gtk::Settings::default() {
            settings
                .bind_property(
                    "gtk-interface-color-scheme",
                    &preview,
                    "prefers-color-scheme",
                )
                .sync_create()
                .build();
        }
    }

    let win = Rc::new(Window {
        window: window.clone(),
        toasts,
        title,
        grid: grid.clone(),
        show_all: show_all.clone(),
        cards,
        palettes,
        light,
        dark,
        pair: pair.clone(),
        light_row: light_row.clone(),
        dark_row: dark_row.clone(),
        apply: apply.clone(),
        style: adw::StyleManager::default(),
        preview,
        previewing: Cell::new(false),
        config_home,
    });

    // Start from what is on disk, before any handler can turn it into a preview.
    let statuses = win.refresh();
    if let Some(CssState::PintaPair { light, dark }) = gtk4_state(&statuses) {
        win.select_in(&win.light_row, &win.light, light);
        win.select_in(&win.dark_row, &win.dark, dark);
        win.pair.set_active(true);
        win.update_apply();
    }

    let w = win.clone();
    grid.set_filter_func(move |child| w.matches_filter(child));
    let w = win.clone();
    show_all.connect_toggled(move |_| w.grid.invalidate_filter());
    let w = win.clone();
    win.style
        .connect_dark_notify(move |_| w.grid.invalidate_filter());

    let w = win.clone();
    grid.connect_selected_children_changed(move |_| w.on_card_selected());
    let w = win.clone();
    pair.connect_active_notify(move |_| w.update_preview());
    for row in [&light_row, &dark_row] {
        let w = win.clone();
        row.connect_selected_notify(move |_| w.update_preview());
    }

    let w = win.clone();
    apply.connect_clicked(move |_| w.on_apply());
    let w = win.clone();
    reset.connect_clicked(move |_| w.on_reset());

    let details = errors
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n\n");
    let w = win.clone();
    broken.connect_button_clicked(move |_| w.alert("Palettes that were skipped", &details));

    window
}

impl Window {
    fn gtk_config(&self) -> Option<GtkConfig> {
        self.config_home.clone().map(GtkConfig::new)
    }

    fn selected(&self) -> Option<&Palette> {
        let child = self.grid.selected_children().into_iter().next()?;
        self.palettes.get(usize::try_from(child.index()).ok()?)
    }

    fn pair_choice(&self) -> Option<(&Palette, &Palette)> {
        let pick = |row: &adw::ComboRow, indices: &[usize]| {
            indices
                .get(row.selected() as usize)
                .map(|&i| &self.palettes[i])
        };
        Some((
            pick(&self.light_row, &self.light)?,
            pick(&self.dark_row, &self.dark)?,
        ))
    }

    fn select_in(&self, row: &adw::ComboRow, indices: &[usize], id: &str) {
        if let Some(pos) = indices.iter().position(|&i| self.palettes[i].id == id) {
            row.set_selected(pos as u32);
        }
    }

    fn matches_filter(&self, child: &gtk::FlowBoxChild) -> bool {
        let current = if self.style.is_dark() {
            Variant::Dark
        } else {
            Variant::Light
        };
        self.show_all.is_active()
            || usize::try_from(child.index())
                .ok()
                .and_then(|i| self.palettes.get(i))
                .is_some_and(|p| p.variant == current)
    }

    fn on_card_selected(&self) {
        // In pair mode a card fills in the light or dark half.
        if let Some(p) = self.selected().filter(|_| self.pair.is_active()) {
            let id = p.id.clone();
            match p.variant {
                Variant::Light => self.select_in(&self.light_row, &self.light, &id),
                Variant::Dark => self.select_in(&self.dark_row, &self.dark, &id),
            }
        }
        self.update_preview();
    }

    /// Previews whatever Apply would write, or nothing.
    fn update_preview(&self) {
        let css = if self.pair.is_active() {
            self.pair_choice().map(|(l, d)| css::gtk4_pair(l, d))
        } else {
            self.selected().map(|p| Toolkit::Gtk4.render(p))
        };
        self.set_preview(css.as_deref());
        self.update_apply();
    }

    fn update_apply(&self) {
        let ready = if self.pair.is_active() {
            self.pair_choice().is_some()
        } else {
            self.selected().is_some()
        };
        self.apply
            .set_sensitive(ready && self.config_home.is_some());
    }

    fn set_preview(&self, css: Option<&str>) {
        let Some(display) = gdk::Display::default() else {
            return;
        };
        match css {
            Some(css) => {
                self.preview.load_from_string(css);
                if !self.previewing.replace(true) {
                    // Above USER, so the preview beats the gtk.css GTK itself loaded.
                    gtk::style_context_add_provider_for_display(
                        &display,
                        &self.preview,
                        gtk::STYLE_PROVIDER_PRIORITY_USER + 1,
                    );
                }
            }
            None => {
                if self.previewing.replace(false) {
                    gtk::style_context_remove_provider_for_display(&display, &self.preview);
                }
            }
        }
    }

    fn on_apply(&self) {
        let (Some(cfg), Some(home)) = (self.gtk_config(), self.config_home.as_deref()) else {
            return;
        };
        let (result, applied, follow) = if self.pair.is_active() {
            let Some((light, dark)) = self.pair_choice() else {
                return;
            };
            (
                cfg.apply_pair(light, dark, self.style.is_dark()),
                format!("{} and {}", light.name, dark.name),
                true,
            )
        } else {
            let Some(p) = self.selected() else {
                return;
            };
            (cfg.apply(p), p.name.clone(), false)
        };

        match result {
            Ok(()) => {
                self.toast(&format!("Applied {applied}. Restart apps to see it."));
                if let Err(e) = desktop::set_watch(home, follow) {
                    let heading = if follow {
                        "GTK3 apps will not follow the style switch"
                    } else {
                        "Could not stop pinta watch"
                    };
                    self.alert(heading, &e);
                }
            }
            Err(e @ ApplyError::BackupExists { .. }) => {
                self.alert("Your gtk.css is in the way", &e.to_string());
            }
            Err(e) => self.alert("Could not apply", &e.to_string()),
        }
        self.refresh();
    }

    fn on_reset(&self) {
        let (Some(cfg), Some(home)) = (self.gtk_config(), self.config_home.as_deref()) else {
            return;
        };
        match cfg.restore() {
            Ok(outcomes) => self.toast(&reset_summary(&outcomes)),
            Err(e) => self.alert("Could not reset", &e.to_string()),
        }
        if let Err(e) = desktop::set_watch(home, false) {
            self.alert("Could not stop pinta watch", &e);
        }
        self.grid.unselect_all();
        self.pair.set_active(false);
        self.set_preview(None);
        self.refresh();
    }

    /// Updates the status line and check marks from disk, and returns what it read.
    fn refresh(&self) -> Vec<Status> {
        let Some(cfg) = self.gtk_config() else {
            self.title.set_subtitle("Not available");
            return Vec::new();
        };
        let statuses = match cfg.status() {
            Ok(statuses) => statuses,
            Err(e) => {
                log::error(&format!("cannot read GTK config: {e}"));
                self.title
                    .set_subtitle(&format!("Cannot read your GTK config: {e}"));
                return Vec::new();
            }
        };

        let name = |id: &str| {
            self.palettes
                .iter()
                .find(|p| p.id == id)
                .map_or(id.to_owned(), |p| p.name.clone())
        };
        let mut text = match gtk4_state(&statuses) {
            Some(CssState::Pinta(id)) => format!("Applied: {}", name(id)),
            Some(CssState::PintaPair { light, dark }) => {
                format!("Applied: {} / {}", name(light), name(dark))
            }
            _ => "Not applied".to_owned(),
        };
        if statuses.iter().any(|s| s.css == CssState::Foreign) {
            text.push_str(" · a gtk.css from elsewhere is in use");
        }
        if statuses.iter().any(|s| s.has_backup) {
            text.push_str(" · your own gtk.css is backed up");
        }
        self.title.set_subtitle(&text);

        let applied: Vec<&str> = statuses
            .iter()
            .flat_map(|s| match &s.css {
                CssState::Pinta(id) => vec![id.as_str()],
                CssState::PintaPair { light, dark } => vec![light.as_str(), dark.as_str()],
                CssState::Missing | CssState::Foreign => vec![],
            })
            .collect();
        for (card, palette) in self.cards.iter().zip(&self.palettes) {
            card.set_applied(applied.contains(&palette.id.as_str()));
        }
        statuses
    }

    fn toast(&self, text: &str) {
        log::info(text);
        self.toasts.add_toast(adw::Toast::new(text));
    }

    fn alert(&self, heading: &str, body: &str) {
        log::error(&format!("{heading}: {body}"));
        let dialog = adw::AlertDialog::new(Some(heading), Some(body));
        dialog.add_response("ok", "OK");
        dialog.present(Some(&self.window));
    }
}

fn gtk4_state(statuses: &[Status]) -> Option<&CssState> {
    statuses
        .iter()
        .find(|s| s.toolkit == Toolkit::Gtk4)
        .map(|s| &s.css)
}

fn reset_summary(outcomes: &[(Toolkit, RestoreOutcome)]) -> String {
    if outcomes
        .iter()
        .all(|(_, o)| *o == RestoreOutcome::NothingToDo)
    {
        return "Nothing to reset".to_owned();
    }
    outcomes
        .iter()
        .map(|(toolkit, outcome)| {
            let name = match toolkit {
                Toolkit::Gtk3 => "GTK3",
                Toolkit::Gtk4 => "GTK4",
            };
            let what = match outcome {
                RestoreOutcome::Restored => "put back your gtk.css",
                RestoreOutcome::Removed => "removed",
                RestoreOutcome::LeftForeign => "not Pinta's, left alone",
                RestoreOutcome::NothingToDo => "nothing to do",
            };
            format!("{name}: {what}")
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    /// GTK's own parser is the real judge of the CSS core generates.
    #[test]
    fn gtk_parses_generated_css_without_errors() {
        // ponytail: needs a display; skips without one. CI runs tests under xvfb-run.
        if gtk::init().is_err() {
            eprintln!("skipped: no display");
            return;
        }
        let errors = Rc::new(RefCell::new(Vec::new()));
        let provider = gtk::CssProvider::new();
        let sink = errors.clone();
        provider.connect_parsing_error(move |_, section, error| {
            sink.borrow_mut().push(format!("{section}: {error}"));
        });
        let palettes = palette::bundled();
        let (latte, mocha) = (&palettes[0], &palettes[3]);
        let mut sheets: Vec<String> = palettes.iter().map(css::gtk4).collect();
        sheets.push(css::gtk4_pair(latte, mocha));
        for sheet in &sheets {
            provider.load_from_string(sheet);
        }
        assert_eq!(*errors.borrow(), Vec::<String>::new());
    }
}
