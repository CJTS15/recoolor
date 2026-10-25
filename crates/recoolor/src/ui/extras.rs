//! The "Also Apply To" rows (Flatpak apps, GNOME Shell, icons and cursors) and the
//! "Apps" rows (Ptyxis, fish, VS Code, and hints for Slack and the login screen).

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use adw::prelude::*;
use recoolor_core::{flatpak, themes};

use crate::desktop::{self, Dirs};
use crate::targets::Extras;

const UNSET: &str = "Not Set by Recoolor";

pub struct ExtrasRows {
    pub group: adw::PreferencesGroup,
    pub apps_group: adw::PreferencesGroup,
    /// Opens the Slack colors; the window knows the palette.
    pub slack: gtk::Button,
    ptyxis: adw::SwitchRow,
    fish: adw::SwitchRow,
    vscode: adw::SwitchRow,
    firefox: adw::SwitchRow,
    spotifast: adw::SwitchRow,
    fade: adw::SwitchRow,
    flatpak: adw::SwitchRow,
    shell: adw::SwitchRow,
    icons: adw::ComboRow,
    cursors: adw::ComboRow,
    wallpaper: Rc<RefCell<Option<String>>>,
    wallpaper_row: WallpaperRow,
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
                "Crossfade the screen on Apply, through a small Recoolor extension. After first turning it on, log out and back in once.",
            )
            .sensitive(dirs.is_some())
            .build();
        group.add(&fade);
        group.add(&flatpak);
        group.add(&shell);
        let wallpaper = Rc::new(RefCell::new(None));
        let wallpaper_row = WallpaperRow::new(wallpaper.clone());
        group.add(&wallpaper_row.row);
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
        let fish = app_switch(
            "fish",
            "Syntax colors for new shells",
            desktop::fish_installed(),
            "fish is not installed",
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

        let apps_group = adw::PreferencesGroup::builder().title("Apps").build();
        for row in [&ptyxis, &fish, &vscode, &firefox, &spotifast] {
            apps_group.add(row);
        }
        apps_group.add(&chrome_row);
        apps_group.add(&slack_row);
        apps_group.add(&gdm_row);

        Self {
            group,
            apps_group,
            slack,
            ptyxis,
            fish,
            vscode,
            firefox,
            spotifast,
            fade,
            flatpak,
            shell,
            icons,
            cursors,
            wallpaper,
            wallpaper_row,
            icon_names: RefCell::new(icon_names),
            cursor_names: RefCell::new(cursor_names),
        }
    }

    pub fn set(&self, extras: &Extras) {
        self.flatpak.set_active(extras.flatpak);
        for (row, on) in [
            (&self.shell, extras.shell),
            (&self.ptyxis, extras.ptyxis),
            (&self.fish, extras.fish),
            (&self.vscode, extras.vscode),
            (&self.firefox, extras.firefox),
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
        select(&self.icons, &self.icon_names, &extras.icons);
        select(&self.cursors, &self.cursor_names, &extras.cursors);
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
            icons: chosen(&self.icons, &self.icon_names),
            wallpaper: self.wallpaper.borrow().clone(),
            cursors: chosen(&self.cursors, &self.cursor_names),
            ptyxis: self.ptyxis.is_active(),
            fish: self.fish.is_active(),
            vscode: self.vscode.is_active(),
            firefox: self.firefox.is_active(),
            spotifast: self.spotifast.is_active(),
            fade: self.fade.is_active(),
        }
    }
}

/// GDM Settings lists only system themes, so the steps start with a one-time link.
fn show_gdm_setup(button: &gtk::Button, theme: Option<&std::path::Path>) {
    let gdm_settings = gtk::gio::AppInfo::all()
        .into_iter()
        .find(|a| a.id().as_deref() == Some("io.github.realmazharhussain.GdmSettings.desktop"));
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
            "1. Turn on GNOME Shell above and Apply.\n\
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

/// The wallpaper row: a thumbnail, Choose and Use Current, and a button to stop
/// setting it. The choice takes effect on Apply, like the other rows.
#[derive(Clone)]
struct WallpaperRow {
    row: adw::ActionRow,
    picture: gtk::Picture,
    clear: gtk::Button,
}

impl WallpaperRow {
    fn new(chosen: Rc<RefCell<Option<String>>>) -> Self {
        let picture = gtk::Picture::builder()
            .content_fit(gtk::ContentFit::Cover)
            .width_request(64)
            .height_request(40)
            .valign(gtk::Align::Center)
            .css_classes(["card"])
            .overflow(gtk::Overflow::Hidden)
            .build();
        let clear = gtk::Button::builder()
            .icon_name("edit-clear-symbolic")
            .tooltip_text("Stop Setting the Wallpaper")
            .valign(gtk::Align::Center)
            .css_classes(["flat"])
            .build();
        let current = gtk::Button::builder()
            .icon_name("view-refresh-symbolic")
            .tooltip_text("Use the Current Wallpaper")
            .valign(gtk::Align::Center)
            .css_classes(["flat"])
            .build();
        let choose = gtk::Button::builder()
            .label("Choose…")
            .valign(gtk::Align::Center)
            .build();
        let row = adw::ActionRow::builder().title("Wallpaper").build();
        row.add_suffix(&picture);
        row.add_suffix(&clear);
        row.add_suffix(&current);
        row.add_suffix(&choose);
        let this = Self {
            row,
            picture,
            clear,
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
                .and_then(|s| gtk::gio::File::for_uri(&s.string("picture-uri")).path())
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
        self.row
            .set_subtitle(name.as_deref().unwrap_or("Not Set by Recoolor"));
        self.picture.set_filename(path);
        self.picture.set_visible(path.is_some());
        self.clear.set_visible(path.is_some());
    }
}
