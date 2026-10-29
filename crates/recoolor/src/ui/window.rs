//! The main window: palette grid, preview pane, Apply and Reset.
//!
//! Selecting a card previews it on Recoolor's own window, by loading the same GTK4 CSS
//! that Apply would write into a display-wide provider.

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::rc::Rc;

use adw::prelude::*;
use gtk::gdk;
use recoolor_core::apps::slack;
use recoolor_core::palette::{self, Palette, Variant};
use recoolor_core::{ApplyError, CssState, GtkConfig, RestoreOutcome, Status, Toolkit, css, paths};

use super::accent::AccentRow;
use super::extras::ExtrasRows;
use super::palette_card::PaletteCard;
use super::preview;
use crate::desktop::{self, Dirs};
use crate::targets::{self, Choice};
use crate::{log, login_screen, restart};

// The window lives as long as the app, so handlers hold it strongly.
struct Window {
    window: adw::ApplicationWindow,
    toasts: adw::ToastOverlay,
    title: adw::WindowTitle,
    grid: gtk::FlowBox,
    show_all: gtk::ToggleButton,
    /// Shows only the starred palettes, in both styles.
    favorites_only: gtk::ToggleButton,
    favorites: RefCell<BTreeSet<String>>,
    search: gtk::SearchEntry,
    cards: Vec<PaletteCard>,
    palettes: Vec<Palette>,
    /// Indices into `palettes` behind the light and dark combo rows.
    light: Vec<usize>,
    dark: Vec<usize>,
    pair: adw::SwitchRow,
    light_row: adw::ComboRow,
    dark_row: adw::ComboRow,
    /// Recoolor (applies), with Reset in its dropdown, in the header bar.
    /// that only the narrow layout shows.
    apply: adw::SplitButton,
    apply_action: gtk::gio::SimpleAction,
    reset_action: gtk::gio::SimpleAction,
    /// Set while Apply or Reset runs on a worker thread.
    busy: Cell<bool>,
    style: adw::StyleManager,
    /// GNOME's interface settings, for the accent picked in Settings.
    interface: Option<gtk::gio::Settings>,
    accent: Option<AccentRow>,
    preview: gtk::CssProvider,
    previewing: Cell<bool>,
    /// Counts palette changes, so only the last one ends the color fade.
    fading: Rc<Cell<u32>>,
    extras: ExtrasRows,
    dirs: Option<Dirs>,
}

pub fn build(app: &adw::Application) -> adw::ApplicationWindow {
    let (palettes, errors) = palette::catalog(paths::user_palettes_dir().as_deref());
    for e in &errors {
        log::error(&format!("skipped palette {e}"));
    }
    let dirs = Dirs::find().ok();

    let title = adw::WindowTitle::new("Recoolor", "");
    // Actions, so Reset in the dropdown stays usable while Apply is off.
    let apply_action = gtk::gio::SimpleAction::new("apply", None);
    apply_action.set_enabled(false);
    let reset_action = gtk::gio::SimpleAction::new("reset", None);
    reset_action.set_enabled(dirs.is_some());
    let apply_menu = gtk::gio::Menu::new();
    apply_menu.append(Some("Reset…"), Some("win.reset"));
    let apply = adw::SplitButton::builder()
        .label("Recoolor")
        .action_name("win.apply")
        .menu_model(&apply_menu)
        .dropdown_tooltip("Reset: remove Recoolor's styles and bring back what you had before")
        .css_classes(["suggested-action"])
        .build();
    let show_all = gtk::ToggleButton::builder()
        .label("Show All")
        .tooltip_text("Show light and dark palettes")
        .margin_end(12)
        .build();
    let menu = gtk::gio::Menu::new();
    menu.append(Some("About Recoolor"), Some("app.about"));
    let main_menu = gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .tooltip_text("Main Menu")
        .primary(true)
        .menu_model(&menu)
        .build();
    let header = adw::HeaderBar::builder().title_widget(&title).build();
    header.pack_end(&main_menu);
    header.pack_end(&apply);

    let no_config = adw::Banner::builder()
        .title(
            "Cannot find your home folders. Set HOME or the XDG_*_HOME variables, then open Recoolor again.",
        )
        .revealed(dirs.is_none())
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
        .min_children_per_line(2)
        .max_children_per_line(2)
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
        // Searchable, by the palette's name: there are dozens of each style.
        let row = adw::ComboRow::builder()
            .title(title)
            .model(&gtk::StringList::new(&names))
            .expression(gtk::PropertyExpression::new(
                gtk::StringObject::static_type(),
                gtk::Expression::NONE,
                "string",
            ))
            .enable_search(true)
            .search_match_mode(gtk::StringFilterMatchMode::Substring)
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
    let pair_group = adw::PreferencesGroup::builder()
        .title("System Style")
        .build();
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
    let extras = ExtrasRows::new(dirs.as_ref());
    if let Some(dirs) = &dirs {
        extras.set(&targets::current(dirs));
    }
    let interface = desktop::interface_settings();
    let accent = interface.as_ref().and_then(AccentRow::new);
    side.append(&preview::build());
    side.append(&pair_group);
    if let Some(accent) = &accent {
        side.append(&accent.group);
    }
    side.append(&extras.group);
    for group in extras.app_groups() {
        side.append(group);
    }

    let palette_column = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .hexpand(true)
        .build();
    let search = gtk::SearchEntry::builder()
        .placeholder_text("Search palettes")
        .hexpand(true)
        .build();
    let favorites_only = gtk::ToggleButton::builder()
        .icon_name("starred-symbolic")
        .tooltip_text("Show Favorites Only")
        .margin_start(6)
        .build();
    favorites_only.update_property(&[gtk::accessible::Property::Label("Show Favorites Only")]);
    // The search sits beside the toggles; Show All keeps its own margins.
    show_all.set_margin_start(6);
    let tools = gtk::Box::builder().margin_start(12).margin_top(12).build();
    tools.append(&search);
    tools.append(&favorites_only);
    tools.append(&show_all);
    palette_column.append(&tools);
    let palette_scroll = gtk::ScrolledWindow::builder()
        .child(&grid)
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .build();
    palette_column.append(&palette_scroll);
    let separator = gtk::Separator::new(gtk::Orientation::Vertical);
    let side_scroll = gtk::ScrolledWindow::builder()
        .child(&side)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .width_request(380)
        .hexpand(false)
        .build();
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    content.append(&palette_column);
    content.append(&separator);
    content.append(&side_scroll);
    // Wide, each column scrolls on its own; narrow, they stack and this scrolls both.
    let page = gtk::ScrolledWindow::builder()
        .child(&content)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Never)
        .build();
    let toasts = adw::ToastOverlay::new();
    toasts.set_child(Some(&page));

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.add_top_bar(&no_config);
    toolbar.add_top_bar(&broken);
    toolbar.set_content(Some(&toasts));

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Recoolor")
        .default_width(1060)
        .default_height(700)
        .width_request(360)
        .height_request(294)
        .content(&toolbar)
        .build();

    // Narrow windows (phones, tiled halves): one scrolling column with one card per
    // line, and the side pane under the palettes.
    let narrow = adw::Breakpoint::new(
        adw::BreakpointCondition::parse("max-width: 720sp").expect("valid breakpoint"),
    );
    let never = gtk::PolicyType::Never;
    narrow.add_setter(
        &content,
        "orientation",
        Some(&gtk::Orientation::Vertical.to_value()),
    );
    narrow.add_setter(
        &separator,
        "orientation",
        Some(&gtk::Orientation::Horizontal.to_value()),
    );
    narrow.add_setter(&side_scroll, "width-request", Some(&(-1).to_value()));
    narrow.add_setter(
        &palette_scroll,
        "vscrollbar-policy",
        Some(&never.to_value()),
    );
    narrow.add_setter(&side_scroll, "vscrollbar-policy", Some(&never.to_value()));
    narrow.add_setter(
        &page,
        "vscrollbar-policy",
        Some(&gtk::PolicyType::Automatic.to_value()),
    );
    narrow.add_setter(&grid, "min-children-per-line", Some(&1u32.to_value()));
    narrow.add_setter(&grid, "max-children-per-line", Some(&1u32.to_value()));
    window.add_breakpoint(narrow);

    // Wide windows (maximized or fullscreen): four cards per line.
    let wide = adw::Breakpoint::new(
        adw::BreakpointCondition::parse("min-width: 1400sp").expect("valid breakpoint"),
    );
    wide.add_setter(&grid, "min-children-per-line", Some(&4u32.to_value()));
    wide.add_setter(&grid, "max-children-per-line", Some(&4u32.to_value()));
    window.add_breakpoint(wide);
    // A single-selection FlowBox selects whatever has focus, which would start a
    // preview before the user picks anything.
    GtkWindowExt::set_focus(&window, Some(&show_all));

    // Colors fade while the `fading` class is on the window. Only then, so the
    // transition does not replace libadwaita's own ones the rest of the time.
    // The selected card and accent get an accent-colored ring that follows their
    // own rounded corners (GTK draws outlines along the border radius) instead of
    // the flow box's square selection background.
    let fade = gtk::CssProvider::new();
    fade.load_from_string(
        "window.fading, window.fading * { transition: background-color 400ms ease-out, \
         color 400ms ease-out, border-color 400ms ease-out, box-shadow 400ms ease-out, \
         outline-color 400ms ease-out; }\n\
         flowboxchild:selected { background: none; }\n\
         flowboxchild > .card { outline: 2px solid transparent; outline-offset: 2px; \
         transition: outline-color 200ms ease-out; }\n\
         flowboxchild:selected > .card { outline-color: var(--accent-color); }\n\
         flowboxchild:hover:not(:selected) > .card { outline-color: alpha(currentColor, 0.15); }\n\
         button.accent-swatch { padding: 4px; outline: 2px solid transparent; \
         outline-offset: -2px; transition: outline-color 200ms ease-out; }\n\
         button.accent-swatch:checked { background: none; outline-color: var(--accent-color); }",
    );
    if let Some(display) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &fade,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    let preview = gtk::CssProvider::new();
    // A new provider evaluates `prefers-color-scheme` as light (GTK 4.20+); follow
    // the desktop instead, so a pair previews its dark half in dark style.
    if preview.find_property("prefers-color-scheme").is_some()
        && let Some(settings) = gtk::Settings::default()
    {
        settings
            .bind_property(
                "gtk-interface-color-scheme",
                &preview,
                "prefers-color-scheme",
            )
            .sync_create()
            .build();
    }

    let win = Rc::new(Window {
        window: window.clone(),
        toasts,
        title,
        grid: grid.clone(),
        show_all: show_all.clone(),
        favorites_only: favorites_only.clone(),
        favorites: RefCell::new(
            recoolor_core::favorites::path()
                .map(|p| recoolor_core::favorites::load(&p))
                .unwrap_or_default(),
        ),
        search: search.clone(),
        cards,
        palettes,
        light,
        dark,
        pair: pair.clone(),
        light_row: light_row.clone(),
        dark_row: dark_row.clone(),
        apply,
        apply_action: apply_action.clone(),
        reset_action: reset_action.clone(),
        busy: Cell::new(false),
        style: adw::StyleManager::default(),
        interface,
        accent,
        preview,
        previewing: Cell::new(false),
        fading: Rc::new(Cell::new(0)),
        extras,
        dirs,
    });

    // Start from what is on disk, before any handler can turn it into a preview.
    let statuses = win.refresh();
    if let Some(CssState::RecoolorPair { light, dark }) = gtk4_state(&statuses) {
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
    favorites_only.connect_toggled(move |_| w.grid.invalidate_filter());
    for (card, palette) in win.cards.iter().zip(&win.palettes) {
        // Set before connecting, so loading them does not save them again.
        card.star
            .set_active(win.favorites.borrow().contains(&palette.id));
        let (w, id) = (win.clone(), palette.id.clone());
        card.star
            .connect_toggled(move |star| w.set_favorite(&id, star.is_active()));
    }
    let w = win.clone();
    search.connect_search_changed(move |_| w.grid.invalidate_filter());
    let w = win.clone();
    win.style
        .connect_dark_notify(move |_| w.grid.invalidate_filter());
    if let Some(interface) = &win.interface {
        let w = win.clone();
        interface.connect_changed(Some("accent-color"), move |s, _| {
            if let Some(accent) = &w.accent {
                accent.select(desktop::accent_choice(s).as_deref());
            }
            w.update_preview();
        });
    }

    let w = win.clone();
    grid.connect_selected_children_changed(move |_| w.on_card_selected());
    let w = win.clone();
    pair.connect_active_notify(move |_| w.update_preview());
    for row in [&light_row, &dark_row] {
        let w = win.clone();
        row.connect_selected_notify(move |_| w.update_preview());
    }

    let w = win.clone();
    win.extras
        .slack
        .connect_clicked(move |_| w.show_slack_colors());
    // An app installed or removed while Recoolor was in the background.
    let w = win.clone();
    window.connect_is_active_notify(move |window| {
        if window.is_active() {
            w.extras.refresh_installed();
            w.refresh_login();
        }
    });
    let update_login = gtk::gio::SimpleAction::new("update-login", None);
    let w = win.clone();
    update_login.connect_activate(move |_, _| w.update_login_screen());
    window.add_action(&update_login);
    win.refresh_login();
    let w = win.clone();
    apply_action.connect_activate(move |_, _| w.on_apply());
    let w = win.clone();
    reset_action.connect_activate(move |_, _| w.confirm_reset());
    window.add_action(&apply_action);
    window.add_action(&reset_action);

    let details = errors
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n\n");
    let w = win.clone();
    broken.connect_button_clicked(move |_| w.alert("Palettes that were skipped", &details));

    // Select the applied palette, so Apply works for changing only the extras.
    if let Some(CssState::Recoolor(id)) = gtk4_state(&statuses)
        && let Some(card) = win.palettes.iter().position(|p| &p.id == id)
    {
        grid.select_child(&win.cards[card].child);
    }

    // In the applied palette's colors from the start, or GNOME's without one.
    if let Some(accent) = &win.accent {
        accent.set_palette(win.palette_in_view());
    }

    window
}

impl Window {
    fn gtk_config(&self) -> Option<GtkConfig> {
        self.dirs.as_ref().map(|d| GtkConfig::new(&d.config))
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

    /// The selected card always; while searching, palettes of either style whose
    /// name or family matches; otherwise this style's, or all with Show All.
    fn matches_filter(&self, child: &gtk::FlowBoxChild) -> bool {
        let Some(p) = usize::try_from(child.index())
            .ok()
            .and_then(|i| self.palettes.get(i))
        else {
            return false;
        };
        let query = self.search.text().trim().to_lowercase();
        if child.is_selected() {
            return true;
        }
        let favorite = self.favorites.borrow().contains(&p.id);
        if self.favorites_only.is_active() && !favorite {
            return false;
        }
        if !query.is_empty() {
            return p.name.to_lowercase().contains(&query)
                || p.family.to_lowercase().contains(&query);
        }
        let current = if self.style.is_dark() {
            Variant::Dark
        } else {
            Variant::Light
        };
        // Starred on purpose, so favorites show in both styles.
        self.favorites_only.is_active() || self.show_all.is_active() || p.variant == current
    }

    fn set_favorite(&self, id: &str, on: bool) {
        let mut favorites = self.favorites.borrow_mut();
        let changed = if on {
            favorites.insert(id.to_owned())
        } else {
            favorites.remove(id)
        };
        if !changed {
            return;
        }
        if let Some(path) = recoolor_core::favorites::path()
            && let Err(e) = recoolor_core::favorites::save(&path, &favorites)
        {
            log::error(&format!("cannot save favorites: {e}"));
        }
        drop(favorites);
        if self.favorites_only.is_active() {
            self.grid.invalidate_filter();
        }
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
        // Preview with the palette color matching the accent picked in Settings.
        let accent = self.interface.as_ref().and_then(desktop::accent_choice);
        let tint = |p: &Palette| p.with_accent(accent.as_deref());
        let css = if self.pair.is_active() {
            self.pair_choice()
                .map(|(l, d)| css::gtk4_pair(&tint(l), &tint(d)))
        } else {
            self.selected().map(|p| Toolkit::Gtk4.render(&tint(p)))
        };
        self.set_preview(css.as_deref());
        if let Some(accent) = &self.accent {
            accent.set_palette(self.palette_in_view());
        }
        self.update_apply();
    }

    fn update_apply(&self) {
        let ready = if self.pair.is_active() {
            self.pair_choice().is_some()
        } else {
            self.selected().is_some()
        };
        self.apply_action
            .set_enabled(ready && self.dirs.is_some() && !self.busy.get());
    }

    fn set_preview(&self, css: Option<&str>) {
        let Some(display) = gdk::Display::default() else {
            return;
        };
        self.fade_colors();
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

    /// Turns the color transitions on for a moment around a palette change.
    fn fade_colors(&self) {
        let generation = self.fading.get() + 1;
        self.fading.set(generation);
        self.window.add_css_class("fading");
        let (window, fading) = (self.window.clone(), self.fading.clone());
        gtk::glib::timeout_add_local_once(std::time::Duration::from_millis(600), move || {
            if fading.get() == generation {
                window.remove_css_class("fading");
            }
        });
    }

    /// The palette in view, tinted with the accent picked in Settings: the selected
    /// card, else the pair's current half, else what is applied.
    fn shown_palette(&self) -> Option<Palette> {
        let accent = self.interface.as_ref().and_then(desktop::accent_choice);
        Some(self.palette_in_view()?.with_accent(accent.as_deref()))
    }

    /// The selected card, else the pair's current half, else what is applied.
    fn palette_in_view(&self) -> Option<&Palette> {
        let dark = self.style.is_dark();
        let in_view = if self.pair.is_active() {
            self.pair_choice().map(|(l, d)| if dark { d } else { l })
        } else {
            self.selected()
        };
        let palette = in_view.or_else(|| {
            let statuses = self.gtk_config()?.status().ok()?;
            let id = match gtk4_state(&statuses)? {
                CssState::Recoolor(id) => id.clone(),
                CssState::RecoolorPair { light, dark: d } => {
                    if dark {
                        d.clone()
                    } else {
                        light.clone()
                    }
                }
                CssState::Missing | CssState::Foreign => return None,
            };
            self.palettes.iter().find(|p| p.id == id)
        })?;
        Some(palette)
    }

    fn show_slack_colors(&self) {
        let Some(palette) = self.shown_palette() else {
            self.toast("Pick a palette first");
            return;
        };
        let lines = slack::colors(&palette)
            .iter()
            .map(|(name, color)| format!("{name}: {color}"))
            .collect::<Vec<_>>()
            .join("\n");
        let dialog = adw::AlertDialog::new(
            Some(&format!("Slack Colors for {}", palette.name)),
            Some(&format!(
                "Enter these in Slack → Preferences → Themes → Custom theme:\n\n{lines}\n\nCopy puts all four on the clipboard, comma-separated, the way Slack shares a theme."
            )),
        );
        dialog.add_responses(&[("close", "Close"), ("copy", "Copy")]);
        dialog.set_response_appearance("copy", adw::ResponseAppearance::Suggested);
        let theme = slack::theme_string(&palette);
        let window = self.window.clone();
        dialog.connect_response(Some("copy"), move |_, _| {
            window.clipboard().set_text(&theme)
        });
        dialog.present(Some(&self.window));
    }

    fn on_apply(self: &Rc<Self>) {
        let Some(dirs) = self.dirs.clone() else {
            return;
        };
        let choice = if self.pair.is_active() {
            let Some((light, dark)) = self.pair_choice() else {
                return;
            };
            Choice::Pair { light, dark }
        } else {
            let Some(p) = self.selected() else {
                return;
            };
            Choice::Single(p)
        };
        let (name, palettes) = (choice.name(), choice.to_vec());
        let (extras, dark_now) = (self.extras.get(), self.style.is_dark());
        // Apps that only read the new colors at startup. Chrome's GTK mode reads
        // Recoolor's GTK colors whether or not anything else is on.
        let restartable: Vec<&'static restart::App> = [
            (extras.firefox, &restart::FIREFOX),
            (extras.thunderbird, &restart::THUNDERBIRD),
            (true, &restart::CHROME),
        ]
        .into_iter()
        .filter_map(|(on, app)| on.then_some(app))
        .collect();

        self.run_busy(
            move || {
                let choice = Choice::from_slice(&palettes);
                let result = targets::apply(&dirs, choice, &extras, dark_now);
                let hint = targets::login_screen_hint(&dirs, choice, &extras);
                (result, hint)
            },
            move |w, (result, hint)| match result {
                Ok(problems) => {
                    w.show_extras_on_disk();
                    w.offer_restart(&name, restartable);
                    if let Some(hint) = hint {
                        log::info(&hint);
                        let toast = adw::Toast::builder()
                            .title(hint)
                            .button_label("Update…")
                            .action_name("win.update-login")
                            .timeout(10)
                            .build();
                        w.toasts.add_toast(toast);
                    }
                    w.refresh_login();
                    if !problems.is_empty() {
                        w.alert("Some parts were not applied", &problems.join("\n\n"));
                    }
                }
                Err(e @ ApplyError::BackupExists { .. }) => {
                    w.alert("Your gtk.css is in the way", &e.to_string());
                }
                Err(e) => w.alert("Could not apply", &e.to_string()),
            },
        );
    }

    fn confirm_reset(self: &Rc<Self>) {
        let dialog = adw::AlertDialog::new(
            Some("Reset Recoolor?"),
            Some(
                "This removes Recoolor's styles from GTK, the Shell and the apps it themed, and brings back what you had before.",
            ),
        );
        dialog.add_responses(&[("cancel", "_Cancel"), ("reset", "_Reset")]);
        dialog.set_response_appearance("reset", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let w = self.clone();
        dialog.connect_response(Some("reset"), move |_, _| w.on_reset());
        dialog.present(Some(&self.window));
    }

    fn on_reset(self: &Rc<Self>) {
        let Some(dirs) = self.dirs.clone() else {
            return;
        };
        self.run_busy(
            move || targets::reset(&dirs),
            |w, (outcomes, problems)| {
                w.show_extras_on_disk();
                if !outcomes.is_empty() {
                    w.toast(&reset_summary(&outcomes));
                }
                if !problems.is_empty() {
                    w.alert("Some parts were not reset", &problems.join("\n\n"));
                }
                w.grid.unselect_all();
                w.pair.set_active(false);
                w.set_preview(None);
                // Only GDM Settings can undo what it wrote.
                if let login_screen::Shows::Palette(_) = login_screen::shows() {
                    let toast = adw::Toast::builder()
                        .title("The login screen keeps Recoolor's colors until you reset it in GDM Settings.")
                        .button_label("Open GDM Settings")
                        .timeout(10)
                        .build();
                    toast.connect_button_clicked(|_| {
                        if let Err(e) = login_screen::open() {
                            log::error(&e);
                        }
                    });
                    w.toasts.add_toast(toast);
                }
                w.refresh_login();
            },
        );
    }

    /// The switches as Recoolor and Reset left them on disk. Only after those: a
    /// restart must not undo switches changed meanwhile.
    fn show_extras_on_disk(&self) {
        if let Some(dirs) = &self.dirs {
            self.extras.set(&targets::current(dirs));
        }
    }

    /// The Login Screen row's status: what the login screen shows now.
    fn refresh_login(&self) {
        let linked = login_screen::linked();
        let status = match login_screen::shows() {
            _ if !linked => "Set up once to use Recoolor's theme through GDM Settings".to_owned(),
            login_screen::Shows::Palette(id) => {
                format!("Shows {}", login_screen::palette_name(&id))
            }
            login_screen::Shows::Gnome => "Shows GNOME's own look".to_owned(),
        };
        self.extras.set_login_status(linked, &status);
    }

    /// Update…: hands GDM Settings Recoolor's theme, accent, icons and cursors, and
    /// opens it, so only its Apply is left. Set Up… first when the theme is not linked.
    fn update_login_screen(self: &Rc<Self>) {
        let Some(dirs) = self.dirs.clone() else {
            return;
        };
        if !login_screen::linked() {
            let theme = dirs
                .data
                .join("themes")
                .join(recoolor_core::shell::THEME_NAME);
            super::extras::show_gdm_setup(&self.window, Some(&theme));
            return;
        }
        let shown = match recoolor_core::shell::status(&dirs.data) {
            Ok(CssState::Recoolor(id)) => login_screen::palette_name(&id),
            _ => {
                self.alert(
                    "Login Screen",
                    "Turn on GNOME Shell and click Recoolor first: the login screen takes Recoolor's Shell theme.",
                );
                return;
            }
        };
        let prepared = if login_screen::running() {
            // It would overwrite its settings file when it closes.
            Err("GDM Settings is already open: pick “Recoolor” as its shell theme there".to_owned())
        } else {
            let extras = self.extras.get();
            let accent = self.interface.as_ref().and_then(desktop::accent_choice);
            // The image the lock screen shows, copied where GDM Settings can read it.
            let lock = recoolor_core::apps::shell_fade::read_config(&dirs.data).lock_image;
            let background = match lock {
                Some(image) => {
                    recoolor_core::shell::login_background(&dirs.data, std::path::Path::new(&image))
                        .map(Some)
                        .map_err(|e| e.to_string())
                }
                None => Ok(None),
            };
            background.and_then(|background| {
                desktop::home().and_then(|home| {
                    login_screen::prefill(
                        &home,
                        accent.as_deref(),
                        extras.icons.as_deref(),
                        extras.cursors.as_deref(),
                        background.as_deref(),
                    )
                })
            })
        };
        if let Err(e) = targets::remember_login_screen(&dirs, true) {
            log::error(&e);
        }
        match login_screen::open() {
            Ok(()) => self.toast(&match prepared {
                Ok(()) => format!(
                    "In GDM Settings, press Apply. The login screen gets the palette on screen now: {shown}."
                ),
                Err(e) => format!("{e}, then press Apply."),
            }),
            Err(e) => self.alert("Login Screen", &e),
        }
    }

    /// Runs `work` on a worker thread, since it waits on systemctl, D-Bus and
    /// other apps, then `done` back on the main thread, and refreshes. Apply and
    /// Reset are off meanwhile, and the app stays up even if the window closes,
    /// so the work is never cut off halfway.
    fn run_busy<T: Send + 'static>(
        self: &Rc<Self>,
        work: impl FnOnce() -> T + Send + 'static,
        done: impl FnOnce(&Rc<Self>, T) + 'static,
    ) {
        if self.busy.replace(true) {
            return;
        }
        self.apply_action.set_enabled(false);
        self.reset_action.set_enabled(false);
        let busy = gtk::Box::builder()
            .spacing(6)
            .halign(gtk::Align::Center)
            .build();
        busy.append(&adw::Spinner::new());
        busy.append(&gtk::Label::new(Some("Recoolor")));
        self.apply.set_child(Some(&busy));
        let hold = self.window.application().map(|app| app.hold());
        let w = self.clone();
        gtk::glib::spawn_future_local(async move {
            let result = gtk::gio::spawn_blocking(work).await;
            drop(hold);
            w.busy.set(false);
            w.apply.set_label("Recoolor");
            w.reset_action.set_enabled(w.dirs.is_some());
            match result {
                Ok(value) => done(&w, value),
                // `alert` logs it; the panic message itself went to stderr.
                Err(_) => w.alert(
                    "Something went wrong",
                    "Recoolor stopped partway, so some changes may be missing. Try again, or Reset.",
                ),
            }
            w.refresh();
            w.update_apply();
        });
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
            Some(CssState::Recoolor(id)) => format!("Applied: {}", name(id)),
            Some(CssState::RecoolorPair { light, dark }) => {
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
                CssState::Recoolor(id) => vec![id.as_str()],
                CssState::RecoolorPair { light, dark } => vec![light.as_str(), dark.as_str()],
                CssState::Missing | CssState::Foreign => vec![],
            })
            .collect();
        for (card, palette) in self.cards.iter().zip(&self.palettes) {
            card.set_applied(applied.contains(&palette.id.as_str()));
        }
        statuses
    }

    /// After Recoolor: apps read the colors when they start, so one dialog lists the
    /// open ones to restart (the browsers, and every app the Shell extension sees),
    /// or a toast when none is open.
    fn offer_restart(self: &Rc<Self>, name: &str, browsers: Vec<&'static restart::App>) {
        let browsers: Vec<&'static restart::App> = browsers
            .into_iter()
            .filter(|a| restart::is_running(a))
            .collect();
        // Restored as they were, so checked; anything else could lose its state.
        const CHECKED: [&str; 2] = ["org.gnome.Nautilus.desktop", "org.gnome.Settings.desktop"];
        let own = [restart::FIREFOX, restart::THUNDERBIRD, restart::CHROME];
        let others: Vec<(String, String)> = desktop::running_apps()
            .into_iter()
            .filter(|(id, _)| id != &format!("{}.desktop", super::APP_ID))
            .filter(|(id, _)| !own.iter().any(|b| b.desktop.contains(&id.as_str())))
            .collect();
        if browsers.is_empty() && others.is_empty() {
            self.toast(&format!("Recoolored with {name}."));
            return;
        }

        let list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .css_classes(["boxed-list"])
            .build();
        let row = |title: &str, checked: bool| {
            let check = gtk::CheckButton::builder().active(checked).build();
            let row = adw::ActionRow::builder().title(title).build();
            row.add_prefix(&check);
            row.set_activatable_widget(Some(&check));
            list.append(&row);
            check
        };
        let browser_checks: Vec<_> = browsers
            .iter()
            .map(|app| (*app, row(app.name, true)))
            .collect();
        let other_checks: Vec<_> = others
            .into_iter()
            .map(|(id, title)| {
                let check = row(&title, CHECKED.contains(&id.as_str()));
                (id, check)
            })
            .collect();

        let dialog = adw::AlertDialog::builder()
            .heading("Restart Apps")
            .body(format!(
                "Recoolored with {name}. Open apps show the new colors after a restart. \
                 Apps that save their session come back as they were; others open fresh."
            ))
            .extra_child(&list)
            .build();
        dialog.add_responses(&[("later", "_Later"), ("restart", "_Restart")]);
        dialog.set_response_appearance("restart", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("restart"));
        dialog.set_close_response("later");
        let w = self.clone();
        dialog.connect_response(Some("restart"), move |_, _| {
            let apps: Vec<&'static restart::App> = browser_checks
                .iter()
                .filter(|(_, c)| c.is_active())
                .map(|(a, _)| *a)
                .collect();
            let ids: Vec<String> = other_checks
                .iter()
                .filter(|(_, c)| c.is_active())
                .map(|(id, _)| id.clone())
                .collect();
            w.run_busy(
                move || {
                    let mut problems: Vec<String> = apps
                        .iter()
                        .filter_map(|app| restart::restart(app).err())
                        .collect();
                    if !ids.is_empty()
                        && let Err(e) = desktop::restart_apps(&ids)
                    {
                        problems.push(format!("cannot restart the other apps: {e}"));
                    }
                    problems
                },
                |w, problems| {
                    if problems.is_empty() {
                        w.toast("Restarted");
                    } else {
                        w.alert("Some apps were not restarted", &problems.join("\n\n"));
                    }
                },
            );
        });
        dialog.present(Some(&self.window));
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
                RestoreOutcome::LeftForeign => "not Recoolor's, left alone",
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
