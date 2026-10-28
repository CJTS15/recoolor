//! The "Also Apply To" rows (GNOME Shell, wallpaper, icons and cursors) and the app
//! rows, in groups by kind (terminal, browsers, GNOME apps, Flatpak, custom themes,
//! others). An app's row shows only while the app is installed, or while it is on.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::path::Path;
use std::rc::Rc;

use adw::prelude::*;
use recoolor_core::{flatpak, themes};

use crate::desktop::{self, Dirs};
use crate::targets::{DEFAULT_STRENGTH, Extras};
use recoolor_core::wallpaper::Tint;

const TINTS: [(Tint, &str); 3] = [
    (Tint::Match, "Match Palette"),
    (Tint::Mono, "Mono"),
    (Tint::None, "Original"),
];

const UNSET: &str = "Not Set by Recoolor";

const GDM_SETTINGS: &str = "io.github.realmazharhussain.GdmSettings.desktop";

/// Tells from the installed launcher ids whether an app is installed.
type Installed = Box<dyn Fn(&HashSet<String>) -> bool>;

/// A row in an app group, and how to tell whether its app is installed.
struct AppRow {
    row: gtk::Widget,
    /// On rows stay visible, so what Recoolor turned on can be turned off.
    switch: Option<adw::SwitchRow>,
    installed: Installed,
}

/// Launcher ids installed now, from the system and Flatpak alike.
fn installed_ids() -> HashSet<String> {
    gtk::gio::AppInfo::all()
        .into_iter()
        .filter_map(|a| a.id().map(|id| id.to_string()))
        .collect()
}

/// An app is installed when one of its launchers is.
fn launcher(ids: &'static [&'static str]) -> Installed {
    Box::new(move |installed| ids.iter().any(|id| installed.contains(*id)))
}

pub struct ExtrasRows {
    pub group: adw::PreferencesGroup,
    /// The app groups, in order; each hides while none of its apps is installed.
    apps: Vec<(adw::PreferencesGroup, Vec<AppRow>)>,
    /// Opens the Slack colors; the window knows the palette.
    pub slack: gtk::Button,
    ptyxis: adw::SwitchRow,
    fish: adw::SwitchRow,
    btop: adw::SwitchRow,
    vscode: adw::SwitchRow,
    firefox: adw::SwitchRow,
    thunderbird: adw::SwitchRow,
    text_editor: adw::SwitchRow,
    kde: adw::SwitchRow,
    spotifast: adw::SwitchRow,
    fade: adw::SwitchRow,
    flatpak: adw::SwitchRow,
    shell: adw::SwitchRow,
    adw_gtk3: adw::SwitchRow,
    icons: adw::ComboRow,
    cursors: adw::ComboRow,
    wallpaper: Rc<RefCell<Option<String>>>,
    wallpaper_row: WallpaperRow,
    wallpaper_dark: Rc<RefCell<Option<String>>>,
    wallpaper_dark_row: WallpaperRow,
    tint: adw::ComboRow,
    strength: gtk::Scale,
    lock: Rc<RefCell<Option<String>>>,
    lock_row: WallpaperRow,
    lock_blur: adw::SwitchRow,
    // Grow when a theme Recoolor set lies outside the scanned folders.
    icon_names: RefCell<Vec<String>>,
    cursor_names: RefCell<Vec<String>>,
}

impl ExtrasRows {
    pub fn new(dirs: Option<&Dirs>) -> Self {
        let flatpak = adw::SwitchRow::builder()
            .title("Flatpak Apps")
            .subtitle("Let sandboxed apps read Recoolor's styles")
            .build();
        if !desktop::flatpak_installed() {
            flatpak.set_sensitive(false);
            flatpak.set_subtitle("Flatpak is not installed");
        } else if flatpak::covered(&[Path::new(flatpak::SYSTEM_OVERRIDES)]) {
            // Nothing for Recoolor to add or take away.
            flatpak.set_active(true);
            flatpak.set_sensitive(false);
            flatpak.set_subtitle("Already allowed for all users by a system-wide override");
        }

        let shell = adw::SwitchRow::builder()
            .title("GNOME Shell")
            .subtitle("Top bar, menus, notifications and overview")
            .build();
        let installed = dirs.is_some_and(|d| desktop::user_theme_settings(&d.data).is_some());
        if !installed {
            shell.set_sensitive(false);
            shell.set_subtitle("Needs the User Themes extension");
        } else if !desktop::user_theme_enabled() {
            shell.set_sensitive(false);
            shell.set_subtitle("Turn on the User Themes extension in Extensions");
        }

        let search = themes::search_dirs(dirs.map(|d| d.data.as_path()));
        let choices = |found: Vec<String>| -> Vec<String> {
            std::iter::once(UNSET.to_owned()).chain(found).collect()
        };
        let (icon_names, cursor_names) = (
            choices(themes::icon_themes(&search)),
            choices(themes::cursor_themes(&search)),
        );
        let combo = |title: &str, names: &[String]| {
            let names: Vec<&str> = names.iter().map(String::as_str).collect();
            adw::ComboRow::builder()
                .title(title)
                .model(&gtk::StringList::new(&names))
                .enable_search(true)
                .build()
        };
        let icons = combo("Icons", &icon_names);
        let cursors = combo("Cursor", &cursor_names);

        let group = adw::PreferencesGroup::builder()
            .title("Also Apply To")
            .description(
                "The accent you pick in Settings → Appearance becomes the palette's own color of that name. Reset puts each of these back the way it was.",
            )
            .build();
        let fade = adw::SwitchRow::builder()
            .title("Screen Fade")
            .subtitle(
                "Crossfade the screen when you Recoolor, through a small Recoolor extension. After first turning it on, log out and back in once.",
            )
            .sensitive(dirs.is_some())
            .build();
        group.add(&fade);
        group.add(&shell);
        let adw_gtk3 = adw::SwitchRow::builder()
            .title("adw-gtk3 for GTK3 Apps")
            .subtitle("Full coverage for GTK3 apps through the libadwaita-style adw-gtk3 theme")
            .build();
        if !dirs.is_some_and(|d| desktop::adw_gtk3_installed(&d.data)) {
            adw_gtk3.set_sensitive(false);
            adw_gtk3.set_subtitle("Install adw-gtk3 first: sudo dnf install adw-gtk3-theme");
        } else if desktop::flatpak_installed() && !desktop::adw_gtk3_flatpak() {
            adw_gtk3.set_subtitle(
                "For GTK3 Flatpak apps too, install it from Flathub: flatpak install org.gtk.Gtk3theme.adw-gtk3 org.gtk.Gtk3theme.adw-gtk3-dark",
            );
        }
        group.add(&adw_gtk3);
        let wallpaper = Rc::new(RefCell::new(None));
        let wallpaper_row = WallpaperRow::new(
            "Wallpaper",
            "Not Set by Recoolor",
            "picture-uri",
            wallpaper.clone(),
        );
        let wallpaper_dark = Rc::new(RefCell::new(None));
        let wallpaper_dark_row = WallpaperRow::new(
            "Dark Style Wallpaper",
            "Same as Light Style",
            "picture-uri-dark",
            wallpaper_dark.clone(),
        );
        let tint = adw::ComboRow::builder()
            .title("Wallpaper Tint")
            .model(&gtk::StringList::new(&TINTS.map(|(_, label)| label)))
            .build();
        let strength = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 100.0, 5.0);
        strength.set_width_request(160);
        strength.set_valign(gtk::Align::Center);
        strength.set_value(f64::from(DEFAULT_STRENGTH));
        strength.update_property(&[gtk::accessible::Property::Label("Tint Strength")]);
        let strength_row = adw::ActionRow::builder().title("Tint Strength").build();
        strength_row.add_suffix(&strength);
        // Strength means nothing for the original image.
        tint.bind_property("selected", &strength_row, "sensitive")
            .transform_to(|_, i: u32| Some(TINTS[i as usize].0 != Tint::None))
            .sync_create()
            .build();
        group.add(&wallpaper_row.row);
        group.add(&wallpaper_dark_row.row);
        group.add(&tint);
        group.add(&strength_row);
        let lock = Rc::new(RefCell::new(None));
        let lock_row = WallpaperRow::new(
            "Lock Screen",
            "Same as Desktop, Blurred",
            "picture-uri",
            lock.clone(),
        );
        let lock_blur = adw::SwitchRow::builder()
            .title("Blur Lock Screen")
            .subtitle("A lock screen image of its own goes through Recoolor's Shell extension: log out and back in once after first setting one")
            .build();
        // Blur only applies to an image of its own; GNOME always blurs the desktop's.
        lock_row
            .clear
            .bind_property("visible", &lock_blur, "sensitive")
            .sync_create()
            .build();
        group.add(&lock_row.row);
        group.add(&lock_blur);
        group.add(&icons);
        group.add(&cursors);

        let app_switch = |title: &str, subtitle: &str, available: bool, missing: &str| {
            adw::SwitchRow::builder()
                .title(title)
                .subtitle(if available { subtitle } else { missing })
                .sensitive(available)
                .build()
        };
        let ptyxis = app_switch(
            "Terminal",
            "Ptyxis palette for the default profile; changes open terminals",
            desktop::ptyxis_profile().is_some(),
            "Ptyxis is not installed",
        );
        let ohmyposh = dirs.is_some_and(|d| {
            std::fs::read_to_string(recoolor_core::apps::ohmyposh::config_fish(&d.config))
                .is_ok_and(|text| recoolor_core::apps::ohmyposh::config_arg(&text).is_some())
        });
        let fish = app_switch(
            "fish",
            if ohmyposh {
                "A Recoolor theme and your oh-my-posh prompt in the palette, for new shells"
            } else {
                "A Recoolor theme, light and dark, for new shells"
            },
            desktop::fish_installed(),
            "fish is not installed",
        );
        let btop = app_switch(
            "btop",
            "Selects a generated “recoolor” theme; changes open windows",
            dirs.is_some_and(|d| desktop::btop_installed(&d.config)),
            "Open btop once first",
        );
        let vscode = app_switch(
            "VS Code",
            "Selects a generated “Recoolor” theme; Reset brings yours back",
            dirs.is_some_and(|d| desktop::vscode_installed(&d.config)),
            "Open VS Code once first",
        );

        let home = desktop::home().ok();
        let firefox = app_switch(
            "Firefox",
            "Colors firefox-gnome-theme and Catppuccin-style userChrome; restart Firefox to see it",
            home.as_ref()
                .is_some_and(|h| !recoolor_core::apps::firefox::profiles(h).is_empty()),
            "Open Firefox once first",
        );
        let thunderbird = app_switch(
            "Thunderbird",
            "Colors Thunderbird's window through userChrome; restart Thunderbird to see it",
            home.as_ref()
                .is_some_and(|h| !recoolor_core::apps::firefox::thunderbird_profiles(h).is_empty()),
            "Open Thunderbird once first",
        );
        let text_editor = app_switch(
            "Text Editor",
            "A Recoolor style scheme for light and dark, selected in GNOME Text Editor",
            desktop::text_editor_settings().is_some(),
            "GNOME Text Editor is not installed",
        );
        let kde = adw::SwitchRow::builder()
            .title("Qt and KDE Apps")
            .subtitle("A KDE color scheme for KDE apps and Qt Flatpaks such as Kdenlive; other Qt apps follow GTK3. Restart them to see it")
            .build();
        let spotifast = app_switch(
            "Spotifast",
            "Adds a “Recoolor” theme; pick it once in Spotifast → Settings → Appearance",
            home.as_ref().zip(dirs).is_some_and(|(h, d)| {
                recoolor_core::apps::spotifast::config_dir(h, &d.config).is_some()
            }),
            "Open Spotifast once first",
        );
        let chrome_row = adw::ActionRow::builder()
            .title("Chrome")
            .subtitle(
                "In Chrome → Settings → Appearance, reset the theme and choose GTK. Chrome then takes Recoolor's GTK colors; restart it after applying.",
            )
            .build();

        let slack = gtk::Button::builder()
            .label("Colors")
            .valign(gtk::Align::Center)
            .build();
        let slack_row = adw::ActionRow::builder()
            .title("Slack")
            .subtitle(
                "Slack keeps its theme in your account, so paste these in Preferences → Themes",
            )
            .build();
        slack_row.add_suffix(&slack);

        let gdm_row = adw::ActionRow::builder()
            .title("Login Screen")
            .subtitle("Use Recoolor's Shell theme through GDM Settings")
            .build();
        let setup = gtk::Button::builder()
            .label("Set Up…")
            .valign(gtk::Align::Center)
            .build();
        let theme = dirs.map(|d| d.data.join("themes").join(recoolor_core::shell::THEME_NAME));
        setup.connect_clicked(move |button| show_gdm_setup(button, theme.as_deref()));
        gdm_row.add_suffix(&setup);

        let app = |row: &adw::SwitchRow, installed: Installed| AppRow {
            row: row.clone().upcast(),
            switch: Some(row.clone()),
            installed,
        };
        let hint = |row: &adw::ActionRow, installed: Installed| AppRow {
            row: row.clone().upcast(),
            switch: None,
            installed,
        };
        let groups = [
            (
                "Terminal",
                None,
                vec![
                    app(&ptyxis, launcher(&["org.gnome.Ptyxis.desktop"])),
                    app(&fish, Box::new(|_| desktop::fish_installed())),
                    app(&btop, launcher(&["btop.desktop"])),
                ],
            ),
            (
                "Browsers and Email",
                None,
                vec![
                    app(&firefox, launcher(crate::restart::FIREFOX.desktop)),
                    app(&thunderbird, launcher(crate::restart::THUNDERBIRD.desktop)),
                    hint(&chrome_row, launcher(crate::restart::CHROME.desktop)),
                ],
            ),
            (
                "GNOME Apps",
                Some("Settings, Files and other GNOME apps follow the GTK styles"),
                vec![
                    app(&text_editor, launcher(&["org.gnome.TextEditor.desktop"])),
                    hint(&gdm_row, launcher(&[GDM_SETTINGS])),
                ],
            ),
            (
                "Flatpak Apps",
                None,
                vec![app(&flatpak, Box::new(|_| desktop::flatpak_installed()))],
            ),
            (
                "Custom Themes",
                Some("Apps whose own theme files Recoolor writes"),
                vec![
                    app(
                        &vscode,
                        launcher(&[
                            "com.microsoft.VSCode.desktop",
                            "code.desktop",
                            "com.visualstudio.code.desktop",
                        ]),
                    ),
                    app(&spotifast, launcher(&["rocks.spotifast.Spotifast.desktop"])),
                    // Qt apps come from many places, often Flatpaks: always offered.
                    app(&kde, Box::new(|_| true)),
                ],
            ),
            (
                "Other Apps",
                None,
                vec![hint(
                    &slack_row,
                    launcher(&["slack.desktop", "com.slack.Slack.desktop"]),
                )],
            ),
        ];
        let apps = groups
            .into_iter()
            .map(|(title, description, rows)| {
                let group = adw::PreferencesGroup::builder()
                    .title(title)
                    .description(description.unwrap_or_default())
                    .build();
                for row in &rows {
                    group.add(&row.row);
                }
                (group, rows)
            })
            .collect();

        let this = Self {
            group,
            apps,
            slack,
            ptyxis,
            fish,
            btop,
            vscode,
            firefox,
            thunderbird,
            text_editor,
            kde,
            spotifast,
            fade,
            flatpak,
            shell,
            adw_gtk3,
            icons,
            cursors,
            wallpaper,
            wallpaper_row,
            wallpaper_dark,
            wallpaper_dark_row,
            tint,
            strength,
            lock,
            lock_row,
            lock_blur,
            icon_names: RefCell::new(icon_names),
            cursor_names: RefCell::new(cursor_names),
        };
        this.refresh_installed();
        this
    }

    pub fn app_groups(&self) -> impl Iterator<Item = &adw::PreferencesGroup> {
        self.apps.iter().map(|(group, _)| group)
    }

    /// Shows the rows of installed apps (and of those that are on), and the groups
    /// with any row showing. Cheap; called again whenever the window comes back.
    pub fn refresh_installed(&self) {
        let ids = installed_ids();
        for (group, rows) in &self.apps {
            let mut any = false;
            for row in rows {
                let on = row.switch.as_ref().is_some_and(adw::SwitchRow::is_active);
                let show = on || (row.installed)(&ids);
                row.row.set_visible(show);
                any |= show;
            }
            group.set_visible(any);
        }
    }

    pub fn set(&self, extras: &Extras) {
        self.flatpak.set_active(extras.flatpak);
        for (row, on) in [
            (&self.shell, extras.shell),
            (&self.adw_gtk3, extras.adw_gtk3),
            (&self.ptyxis, extras.ptyxis),
            (&self.fish, extras.fish),
            (&self.btop, extras.btop),
            (&self.vscode, extras.vscode),
            (&self.firefox, extras.firefox),
            (&self.thunderbird, extras.thunderbird),
            (&self.text_editor, extras.text_editor),
            (&self.kde, extras.kde),
            (&self.spotifast, extras.spotifast),
            (&self.fade, extras.fade),
        ] {
            row.set_active(on);
            // Something Recoolor turned on must stay switchable off, even if its app or
            // extension has since gone.
            if on {
                row.set_sensitive(true);
            }
        }
        let select = |row: &adw::ComboRow, names: &RefCell<Vec<String>>, name: &Option<String>| {
            let mut names = names.borrow_mut();
            let pos = match name {
                None => 0,
                Some(name) => names.iter().position(|x| x == name).unwrap_or_else(|| {
                    // Listed here so the next Apply keeps it instead of undoing it.
                    if let Some(list) = row.model().and_downcast::<gtk::StringList>() {
                        list.append(name);
                    }
                    names.push(name.clone());
                    names.len() - 1
                }),
            };
            row.set_selected(pos as u32);
        };
        *self.wallpaper.borrow_mut() = extras.wallpaper.clone();
        self.wallpaper_row.show(extras.wallpaper.as_deref());
        *self.wallpaper_dark.borrow_mut() = extras.wallpaper_dark.clone();
        self.wallpaper_dark_row
            .show(extras.wallpaper_dark.as_deref());
        let pos = TINTS.iter().position(|(t, _)| *t == extras.wallpaper_tint);
        self.tint.set_selected(pos.unwrap_or(0) as u32);
        self.strength
            .set_value(f64::from(extras.wallpaper_strength));
        *self.lock.borrow_mut() = extras.lock_image.clone();
        self.lock_row.show(extras.lock_image.as_deref());
        self.lock_blur.set_active(extras.lock_blur);
        select(&self.icons, &self.icon_names, &extras.icons);
        select(&self.cursors, &self.cursor_names, &extras.cursors);
        self.refresh_installed();
    }

    pub fn get(&self) -> Extras {
        // Position 0 is "Not Set by Recoolor".
        let chosen = |row: &adw::ComboRow, names: &RefCell<Vec<String>>| {
            let pos = row.selected() as usize;
            (pos > 0)
                .then(|| names.borrow().get(pos).cloned())
                .flatten()
        };
        Extras {
            flatpak: self.flatpak.is_active(),
            shell: self.shell.is_active(),
            adw_gtk3: self.adw_gtk3.is_active(),
            icons: chosen(&self.icons, &self.icon_names),
            wallpaper: self.wallpaper.borrow().clone(),
            wallpaper_dark: self.wallpaper_dark.borrow().clone(),
            wallpaper_tint: TINTS
                .get(self.tint.selected() as usize)
                .map_or(Tint::default(), |(t, _)| *t),
            wallpaper_strength: self.strength.value().round().clamp(0.0, 100.0) as u8,
            lock_image: self.lock.borrow().clone(),
            lock_blur: self.lock_blur.is_active(),
            cursors: chosen(&self.cursors, &self.cursor_names),
            ptyxis: self.ptyxis.is_active(),
            fish: self.fish.is_active(),
            btop: self.btop.is_active(),
            vscode: self.vscode.is_active(),
            firefox: self.firefox.is_active(),
            thunderbird: self.thunderbird.is_active(),
            text_editor: self.text_editor.is_active(),
            kde: self.kde.is_active(),
            spotifast: self.spotifast.is_active(),
            fade: self.fade.is_active(),
        }
    }
}

/// GDM Settings lists only system themes, so the steps start with a one-time link.
fn show_gdm_setup(button: &gtk::Button, theme: Option<&std::path::Path>) {
    let gdm_settings = gtk::gio::AppInfo::all()
        .into_iter()
        .find(|a| a.id().as_deref() == Some(GDM_SETTINGS));
    let command = theme.map(|theme| {
        format!(
            "sudo mkdir -p /usr/local/share/themes && sudo ln -sfn '{}' /usr/local/share/themes/{}",
            theme.display().to_string().replace('\'', "'\\''"),
            recoolor_core::shell::THEME_NAME
        )
    });
    let dialog = adw::AlertDialog::new(
        Some("Login Screen"),
        Some(
            "1. Turn on GNOME Shell above and click Recoolor.\n\
             2. GDM Settings lists only system themes, so run the copied command once in a terminal. It links Recoolor's theme into /usr/local/share/themes.\n\
             3. In GDM Settings, pick “Recoolor” as the shell theme.\n\n\
             GDM Settings copies the theme, so pick it again after changing palettes.",
        ),
    );
    dialog.add_response("close", "Close");
    if command.is_some() {
        dialog.add_response("copy", "Copy Command");
    }
    if gdm_settings.is_some() {
        dialog.add_response("open", "Open GDM Settings");
        dialog.set_response_appearance("open", adw::ResponseAppearance::Suggested);
    }
    let clipboard = button.clipboard();
    dialog.connect_response(None, move |_, response| match response {
        "copy" => {
            if let Some(command) = &command {
                clipboard.set_text(command);
            }
        }
        "open" => {
            if let Some(app) = &gdm_settings
                && let Err(e) = app.launch(&[], gtk::gio::AppLaunchContext::NONE)
            {
                crate::log::error(&format!("cannot open GDM Settings: {e}"));
            }
        }
        _ => {}
    });
    dialog.present(Some(button));
}

/// A wallpaper row on two lines: the title and file name, then a preview with
/// Remove, Use Current and Choose…, so it fits a narrow window. The choice takes
/// effect on Recoolor, like the other rows.
#[derive(Clone)]
struct WallpaperRow {
    row: adw::PreferencesRow,
    subtitle: gtk::Label,
    picture: gtk::Picture,
    clear: gtk::Button,
    unset: &'static str,
    /// Counts thumbnail loads, so a slow one never replaces a newer choice.
    loads: Rc<Cell<u32>>,
}

impl WallpaperRow {
    /// `key` is the background setting Use Current reads.
    fn new(
        title: &str,
        unset: &'static str,
        key: &'static str,
        chosen: Rc<RefCell<Option<String>>>,
    ) -> Self {
        let picture = gtk::Picture::builder()
            .content_fit(gtk::ContentFit::Cover)
            .hexpand(true)
            .height_request(90)
            .css_classes(["card"])
            .overflow(gtk::Overflow::Hidden)
            .build();
        let button = |icon: &str, tooltip: &str| {
            let button = gtk::Button::builder()
                .icon_name(icon)
                .tooltip_text(tooltip)
                .valign(gtk::Align::Center)
                .css_classes(["flat"])
                .build();
            button.update_property(&[gtk::accessible::Property::Label(tooltip)]);
            button
        };
        let clear = button("user-trash-symbolic", "Remove");
        let current = button("view-refresh-symbolic", "Use the Current Wallpaper");
        let choose = gtk::Button::builder()
            .label("Choose…")
            .valign(gtk::Align::Center)
            .build();

        let heading = gtk::Label::builder()
            .label(title)
            .xalign(0.0)
            .css_classes(["title"])
            .build();
        let subtitle = gtk::Label::builder()
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::Middle)
            .css_classes(["subtitle"])
            .build();
        let controls = gtk::Box::builder().spacing(6).build();
        controls.append(&picture);
        controls.append(&clear);
        controls.append(&current);
        controls.append(&choose);
        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .margin_top(10)
            .margin_bottom(10)
            .margin_start(12)
            .margin_end(12)
            .build();
        content.append(&heading);
        content.append(&subtitle);
        content.append(&controls);
        let row = adw::PreferencesRow::builder()
            .title(title)
            .activatable(false)
            .child(&content)
            .build();
        let this = Self {
            row,
            subtitle,
            picture,
            clear,
            unset,
            loads: Rc::new(Cell::new(0)),
        };

        let pick = {
            let (this, chosen) = (this.clone(), chosen.clone());
            move |path: Option<String>| {
                this.show(path.as_deref());
                *chosen.borrow_mut() = path;
            }
        };
        let p = pick.clone();
        this.clear.connect_clicked(move |_| p(None));
        let p = pick.clone();
        current.connect_clicked(move |_| {
            let path = desktop::background_settings()
                .and_then(|s| gtk::gio::File::for_uri(&s.string(key)).path())
                .map(|p| p.display().to_string());
            if path.is_some() {
                p(path);
            }
        });
        choose.connect_clicked(move |button| {
            let filter = gtk::FileFilter::new();
            filter.set_name(Some("Images"));
            filter.add_mime_type("image/*");
            let filters = gtk::gio::ListStore::new::<gtk::FileFilter>();
            filters.append(&filter);
            let dialog = gtk::FileDialog::builder()
                .title("Choose a Wallpaper")
                .filters(&filters)
                .default_filter(&filter)
                .build();
            let window = button.root().and_downcast::<gtk::Window>();
            let p = pick.clone();
            dialog.open(
                window.as_ref(),
                gtk::gio::Cancellable::NONE,
                move |result| {
                    if let Some(path) = result.ok().and_then(|f| f.path()) {
                        p(Some(path.display().to_string()));
                    }
                },
            );
        });
        this.show(None);
        this
    }

    fn show(&self, path: Option<&str>) {
        let name = path
            .and_then(|p| Path::new(p).file_name())
            .map(|n| n.to_string_lossy());
        self.subtitle
            .set_label(name.as_deref().unwrap_or(self.unset));
        self.picture.set_paintable(None::<&gtk::gdk::Paintable>);
        self.picture.set_visible(path.is_some());
        self.clear.set_visible(path.is_some());
        let Some(path) = path.map(str::to_owned) else {
            return;
        };
        // gdk-pixbuf reads every format the system can (HEIC and AVIF through glycin),
        // where GtkPicture's own loader shows HEIC as a missing image. Scaled, off the
        // main thread: wallpapers are often tens of megapixels.
        let load = self.loads.get() + 1;
        self.loads.set(load);
        let this = self.clone();
        gtk::glib::spawn_future_local(async move {
            // A Pixbuf cannot cross threads; its pixels can.
            let thumbnail = gtk::gio::spawn_blocking(move || {
                let p = gtk::gdk_pixbuf::Pixbuf::from_file_at_scale(&path, 480, 270, true).ok()?;
                let format = if p.has_alpha() {
                    gtk::gdk::MemoryFormat::R8g8b8a8
                } else {
                    gtk::gdk::MemoryFormat::R8g8b8
                };
                Some((
                    p.read_pixel_bytes(),
                    p.width(),
                    p.height(),
                    p.rowstride(),
                    format,
                ))
            })
            .await
            .ok()
            .flatten();
            if this.loads.get() != load {
                return;
            }
            match thumbnail {
                Some((bytes, width, height, stride, format)) => {
                    let texture = gtk::gdk::MemoryTexture::new(
                        width,
                        height,
                        format,
                        &bytes,
                        stride as usize,
                    );
                    this.picture.set_paintable(Some(&texture));
                }
                None => this.subtitle.set_label(&format!(
                    "{} — cannot read this image",
                    this.subtitle.label()
                )),
            }
        });
    }
}
