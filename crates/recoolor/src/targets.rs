//! Everything Recoolor changes, applied and undone together. The GTK stylesheets are
//! always written; Flatpak access, the Shell, icon and cursor themes, and the app
//! configs (Ptyxis, fish, btop, VS Code) are extras the user turns on. Shared by the
//! window and the command line.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk::gio;
use gtk::gio::prelude::*;
use gtk::glib;
use recoolor_core::apps::{
    self, btop, firefox, fish, gtksourceview, kdeglobals, ptyxis, shell_fade, spotifast, vscode,
};
use recoolor_core::palette::{self, Palette};
use recoolor_core::state::{self, State};
use recoolor_core::wallpaper::{self, Tint};

/// How strongly wallpapers are tinted until the user picks otherwise.
pub const DEFAULT_STRENGTH: u8 = 85;
use recoolor_core::{
    ApplyError, CssState, GtkConfig, RestoreOutcome, Toolkit, Variant, flatpak, paths, shell,
};

use crate::desktop::{self, Dirs};
use crate::log;

#[derive(Clone, Copy)]
pub enum Choice<'a> {
    Single(&'a Palette),
    Pair {
        light: &'a Palette,
        dark: &'a Palette,
    },
}

impl<'a> Choice<'a> {
    /// One palette, or a light and a dark one, as a [`Choice`].
    pub fn from_slice(palettes: &'a [Palette]) -> Self {
        match palettes {
            [light, dark] => Choice::Pair { light, dark },
            _ => Choice::Single(&palettes[0]),
        }
    }

    /// The palettes chosen, light first for a pair, to hand to another thread.
    pub fn to_vec(self) -> Vec<Palette> {
        match self {
            Choice::Single(p) => vec![p.clone()],
            Choice::Pair { light, dark } => vec![light.clone(), dark.clone()],
        }
    }

    pub fn name(self) -> String {
        match self {
            Choice::Single(p) => p.name.clone(),
            Choice::Pair { light, dark } => format!("{} and {}", light.name, dark.name),
        }
    }
}

/// What is on screen: the palette in effect now, and both halves of a pair.
#[derive(Clone, Copy)]
struct Look<'a> {
    now: &'a Palette,
    pair: Option<(&'a Palette, &'a Palette)>,
}

/// The optional targets. `None` for icons or cursors means Recoolor leaves them alone.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Extras {
    pub flatpak: bool,
    pub shell: bool,
    pub icons: Option<String>,
    /// The desktop wallpaper in light style, and in dark style (`None`: the same).
    pub wallpaper: Option<String>,
    pub wallpaper_dark: Option<String>,
    pub wallpaper_tint: Tint,
    pub wallpaper_strength: u8,
    /// A lock screen image of its own (`None`: the desktop's, as GNOME does), and
    /// whether to blur it.
    pub lock_image: Option<String>,
    pub lock_blur: bool,
    /// GTK3 apps use the adw-gtk3 theme, recolored through its named colors.
    pub adw_gtk3: bool,
    pub text_editor: bool,
    pub thunderbird: bool,
    /// KDE's color scheme (`kdeglobals`), for KDE apps and Qt Flatpaks.
    pub kde: bool,
    pub cursors: Option<String>,
    pub ptyxis: bool,
    pub fish: bool,
    pub btop: bool,
    pub vscode: bool,
    pub firefox: bool,
    pub spotifast: bool,
    pub fade: bool,
}

/// The extras as they are now, read from disk, the state file and GNOME settings.
pub fn current(dirs: &Dirs) -> Extras {
    let state = State::load(&state::path(&dirs.state)).unwrap_or_default();
    let shell = shell_fade::read_config(&dirs.data);
    let interface = |previous: &Option<String>, key: &str| {
        previous.as_ref()?;
        Some(desktop::interface_settings()?.string(key).to_string())
    };
    Extras {
        // A grant Recoolor added counts as on even if the user denies the other folder.
        flatpak: !state.flatpak_added.is_empty()
            || flatpak::covered(&[
                &flatpak::overrides_path(&dirs.data),
                Path::new(flatpak::SYSTEM_OVERRIDES),
            ]),
        shell: matches!(
            shell::status(&dirs.data),
            Ok(CssState::Recoolor(_) | CssState::RecoolorPair { .. })
        ),
        icons: interface(&state.icons_previous, "icon-theme"),
        wallpaper: state.wallpaper_source.clone(),
        wallpaper_dark: state.wallpaper_source_dark.clone(),
        wallpaper_tint: state.wallpaper_tint.unwrap_or_default(),
        wallpaper_strength: state.wallpaper_strength.unwrap_or(DEFAULT_STRENGTH),
        lock_image: state.lock_source.clone(),
        lock_blur: shell.lock_blur,
        adw_gtk3: state.gtk_theme_previous.is_some(),
        text_editor: state.text_editor_previous.is_some(),
        kde: kdeglobals::is_applied(&dirs.config),
        thunderbird: desktop::home().is_ok_and(|h| {
            firefox::thunderbird_profiles(&h)
                .iter()
                .any(|p| firefox::is_applied(p))
        }),
        cursors: interface(&state.cursors_previous, "cursor-theme"),
        // A built-in palette leaves no file, so the record alone says it is on.
        ptyxis: state.ptyxis_previous.is_some(),
        fish: apps::is_ours(&fish::path(&dirs.config)),
        btop: state.btop_previous.is_some(),
        vscode: state.vscode_previous.is_some()
            && desktop::home().is_ok_and(|h| vscode::extension_installed(&h)),
        firefox: desktop::home()
            .is_ok_and(|h| firefox::profiles(&h).iter().any(|p| firefox::is_applied(p))),
        // Installed before the settings file existed: that was Screen Fade alone.
        fade: shell.fade
            || (shell_fade::is_installed(&dirs.data)
                && !shell_fade::config_path(&dirs.data).exists()),
        spotifast: desktop::home().is_ok_and(|h| {
            spotifast::config_dir(&h, &dirs.config).is_some_and(|d| spotifast::path(&d).exists())
        }),
    }
}

/// Writes the GTK stylesheets, then brings each extra in line with `extras`:
/// on applies it, off undoes whatever Recoolor did to it.
///
/// A GTK failure is returned before anything changes. Trouble with an extra does
/// not stop the others; it comes back as one message each.
pub fn apply(
    dirs: &Dirs,
    choice: Choice,
    extras: &Extras,
    dark_now: bool,
) -> Result<Vec<String>, ApplyError> {
    let accent = desktop::interface_settings().and_then(|s| desktop::accent_choice(&s));
    let tint = |p: &Palette| p.with_accent(accent.as_deref());
    let tinted: Vec<Palette> = match choice {
        Choice::Single(p) => vec![tint(p)],
        Choice::Pair { light, dark } => vec![tint(light), tint(dark)],
    };
    let look = look(&tinted, dark_now);

    // Hide the changes behind a still of the screen, and fade it away once they are
    // all in, failed or not.
    if extras.fade {
        desktop::screen_fade("Freeze");
    }
    let cfg = GtkConfig::new(&dirs.config).with_adw_gtk3(extras.adw_gtk3);
    let applied = write_gtk(&cfg, look, dark_now).map(|()| {
        log::info(&format!(
            "applied {} (accent {}) with {extras:?}",
            choice.name(),
            accent.as_deref().unwrap_or("palette's own")
        ));
        let mut problems = Vec::new();
        if let Err(e) = desktop::restart_background_apps() {
            problems.push(e);
        }
        problems.extend(set_extras(dirs, extras, Some(look)));
        problems
    });
    if extras.fade {
        desktop::screen_fade("Fade");
    }
    let mut problems = applied?;

    // Last: the watcher syncs from the saved state as soon as it starts, so starting
    // it before the extras are recorded would undo them.
    if let Err(e) = desktop::set_watch(&dirs.config, true) {
        problems.push(format!(
            "Recoolor will not follow accent or light/dark changes: {e}"
        ));
    }
    Ok(problems)
}

/// One palette, or a light and a dark one, as a [`Look`].
fn look(tinted: &[Palette], dark_now: bool) -> Look<'_> {
    match tinted {
        [light, dark] => Look {
            now: if dark_now { dark } else { light },
            pair: Some((light, dark)),
        },
        _ => Look {
            now: &tinted[0],
            pair: None,
        },
    }
}

fn write_gtk(cfg: &GtkConfig, look: Look, dark_now: bool) -> Result<(), ApplyError> {
    match look.pair {
        Some((light, dark)) => cfg.apply_pair(light, dark, dark_now),
        None => cfg.apply(look.now),
    }
}

/// Undoes everything. Returns the GTK outcomes (empty if GTK could not be
/// restored) and every problem met on the way.
pub fn reset(dirs: &Dirs) -> (Vec<(Toolkit, RestoreOutcome)>, Vec<String>) {
    let mut problems = Vec::new();
    let outcomes = GtkConfig::new(&dirs.config).restore().unwrap_or_else(|e| {
        problems.push(e.to_string());
        Vec::new()
    });
    if let Err(e) = desktop::set_watch(&dirs.config, false) {
        problems.push(e);
    }
    if let Err(e) = desktop::restart_background_apps() {
        problems.push(e);
    }
    problems.extend(set_extras(dirs, &Extras::default(), None));
    log::info("reset");
    (outcomes, problems)
}

/// `look` is what to show, or `None` to undo everything.
fn set_extras(dirs: &Dirs, extras: &Extras, look: Option<Look>) -> Vec<String> {
    let when = |on: bool| look.filter(|_| on);
    // The window, the command line and the watcher may all get here at once.
    let _lock = match state::lock(&dirs.state) {
        Ok(lock) => lock,
        Err(e) => return vec![format!("left the extras alone: {e}")],
    };
    let path = state::path(&dirs.state);
    let mut state = match State::load(&path) {
        Ok(state) => state,
        // Without the record of previous values, changing more could lose them.
        Err(e) => return vec![format!("left the extras alone: {e}")],
    };
    let results = [
        (
            "Flatpak apps",
            set_flatpak(dirs, &mut state, extras.flatpak),
        ),
        // Before the Shell theme, whose reload picks up the extension's stylesheet.
        (
            "Lock screen and Screen Fade",
            set_shell_extension(dirs, &mut state, extras, look),
        ),
        (
            "GNOME Shell",
            set_shell(dirs, &mut state, when(extras.shell).map(|l| l.now)),
        ),
        (
            "Light style",
            set_light_shell(&mut state, extras.shell && look.is_some()),
        ),
        ("Wallpaper", set_wallpaper(dirs, &mut state, extras, look)),
        (
            "adw-gtk3",
            set_adw_gtk3(dirs, &mut state, when(extras.adw_gtk3).map(|l| l.now)),
        ),
        (
            "Icons",
            set_interface(
                &mut state.icons_previous,
                "icon-theme",
                extras.icons.as_deref(),
            ),
        ),
        (
            "Cursors",
            set_interface(
                &mut state.cursors_previous,
                "cursor-theme",
                extras.cursors.as_deref(),
            ),
        ),
        (
            "Terminal",
            set_ptyxis(dirs, &mut state, when(extras.ptyxis)),
        ),
        ("fish", set_fish(dirs, when(extras.fish).map(|l| l.now))),
        (
            "btop",
            set_btop(dirs, &mut state, when(extras.btop).map(|l| l.now)),
        ),
        (
            "VS Code",
            set_vscode(dirs, &mut state, when(extras.vscode).map(|l| l.now)),
        ),
        (
            "Firefox",
            set_mozilla(
                "Firefox",
                firefox::profiles,
                when(extras.firefox).map(|l| l.now),
            ),
        ),
        (
            "Thunderbird",
            set_mozilla(
                "Thunderbird",
                firefox::thunderbird_profiles,
                when(extras.thunderbird).map(|l| l.now),
            ),
        ),
        (
            "Text Editor",
            set_text_editor(dirs, &mut state, when(extras.text_editor)),
        ),
        (
            "Qt and KDE apps",
            set_kde(dirs, when(extras.kde).map(|l| l.now)),
        ),
        (
            "Spotifast",
            set_spotifast(dirs, when(extras.spotifast).map(|l| l.now)),
        ),
    ];
    let mut problems: Vec<String> = results
        .into_iter()
        .filter_map(|(what, result)| result.err().map(|e| format!("{what}: {e}")))
        .collect();
    prune_wallpapers(dirs);
    if let Err(e) = state.save(&path) {
        problems.push(format!("cannot record what Recoolor changed: {e}"));
    }
    problems
}

fn set_flatpak(dirs: &Dirs, state: &mut State, on: bool) -> Result<(), String> {
    let path = flatpak::overrides_path(&dirs.data);
    if on {
        if !desktop::flatpak_installed() {
            return Err("Flatpak is not installed".into());
        }
        if flatpak::covered(&[&path, Path::new(flatpak::SYSTEM_OVERRIDES)]) {
            return Ok(());
        }
        let added = flatpak::grant(&path).map_err(|e| e.to_string())?;
        state.flatpak_added.extend(added);
    } else {
        flatpak::revoke(&path, &state.flatpak_added).map_err(|e| e.to_string())?;
        state.flatpak_added.clear();
    }
    Ok(())
}

fn set_shell(dirs: &Dirs, state: &mut State, palette: Option<&Palette>) -> Result<(), String> {
    let Some(palette) = palette else {
        set_popups(dirs, "")?;
        shell::remove(&dirs.data).map_err(|e| e.to_string())?;
        // Forget the previous value only once it is back, so a failed restore can retry.
        if let Some(previous) = state.shell_previous.clone() {
            let settings = desktop::user_theme_settings(&dirs.data)
                .ok_or("the User Themes extension is gone, so its theme was not put back")?;
            desktop::set_string(&settings, "name", &previous)?;
            state.shell_previous = None;
        }
        return Ok(());
    };

    let settings = desktop::user_theme_settings(&dirs.data)
        .ok_or("install the User Themes extension to theme the Shell")?;
    if !desktop::user_theme_enabled() {
        return Err("turn on the User Themes extension (in Extensions) first".into());
    }
    let css = desktop::shell_css(palette, &dirs.data)?;
    let mut popups = desktop::blur_my_shell_css(&dirs.data)
        .map(|bms| recoolor_core::css::shell_popups(palette, &bms))
        .unwrap_or_default();
    if desktop::searchlight_installed(&dirs.data) {
        popups.push('\n');
        popups.push_str(&recoolor_core::css::searchlight(palette));
    }
    let popups_changed = set_popups(dirs, &popups)?;
    let changed = popups_changed
        || std::fs::read_to_string(shell::css_path(&dirs.data)).ok() != Some(css.clone());
    shell::write(&dirs.data, &css).map_err(|e| e.to_string())?;
    let name = settings.string("name").to_string();
    state.shell_previous.get_or_insert_with(|| name.clone());
    // Selecting the theme loads it; otherwise reload only when there is something new.
    if name != shell::THEME_NAME {
        desktop::set_string(&settings, "name", shell::THEME_NAME)
    } else if changed {
        desktop::reload_shell_theme(&settings)
    } else {
        Ok(())
    }
}

/// GNOME keeps the Shell dark unless `color-scheme` is `prefer-light`; `default`
/// means light apps but a dark Shell, which Blur my Shell then paints dark behind
/// a light palette. While Recoolor themes the Shell, light mode is `prefer-light`.
fn set_light_shell(state: &mut State, on: bool) -> Result<(), String> {
    let settings =
        desktop::interface_settings().ok_or("GNOME's interface settings are not installed")?;
    let current = settings.string("color-scheme");
    if on {
        if current == "default" {
            state
                .color_scheme_previous
                .get_or_insert_with(|| current.to_string());
            desktop::set_string(&settings, "color-scheme", "prefer-light")?;
        }
    } else if let Some(previous) = state.color_scheme_previous.clone() {
        // Back to the user's own value, unless they have since gone dark.
        if current == "prefer-light" {
            desktop::set_string(&settings, "color-scheme", &previous)?;
        }
        state.color_scheme_previous = None;
    }
    Ok(())
}

/// Sets the desktop wallpaper for light and dark style (the Shell switches between
/// them itself), or puts the user's own background back when neither is set.
fn set_wallpaper(
    dirs: &Dirs,
    state: &mut State,
    extras: &Extras,
    look: Option<Look>,
) -> Result<(), String> {
    let settings =
        || desktop::background_settings().ok_or("GNOME's background settings are not installed");
    let (light, dark) = match look {
        Some(_) => (
            extras.wallpaper.as_deref(),
            extras.wallpaper_dark.as_deref(),
        ),
        None => (None, None),
    };
    if light.is_none() && dark.is_none() {
        if let Some(previous) = state.background_previous.clone() {
            let settings = settings()?;
            for (key, value) in [
                ("picture-uri", &previous.picture_uri),
                ("picture-uri-dark", &previous.picture_uri_dark),
                ("picture-options", &previous.picture_options),
            ] {
                desktop::set_string(&settings, key, value)?;
            }
            state.background_previous = None;
        }
        state.wallpaper_source = None;
        state.wallpaper_source_dark = None;
        state.wallpaper_tint = None;
        state.wallpaper_strength = None;
        return Ok(());
    }
    let look = look.expect("checked above");
    for source in light.into_iter().chain(dark) {
        if !Path::new(source).is_file() {
            return Err(format!("{source} is not there any more"));
        }
    }
    let settings = settings()?;
    // A wallpaper of Recoolor's own is never the user's (the state was lost);
    // fall back to GNOME's default for that key.
    let user_value = |key: &str| {
        let value = settings.string(key).to_string();
        let ours = gio::File::for_uri(&value)
            .path()
            .is_some_and(|p| wallpaper::is_ours(&p, &dirs.data));
        if ours {
            settings
                .default_value(key)
                .and_then(|v| v.str().map(str::to_owned))
                .unwrap_or_default()
        } else {
            value
        }
    };
    let previous = state
        .background_previous
        .get_or_insert_with(|| state::Background {
            picture_uri: user_value("picture-uri"),
            picture_uri_dark: user_value("picture-uri-dark"),
            picture_options: settings.string("picture-options").into(),
        })
        .clone();
    state.wallpaper_source = light.map(str::to_owned);
    state.wallpaper_source_dark = dark.map(str::to_owned);
    state.wallpaper_tint = Some(extras.wallpaper_tint);
    state.wallpaper_strength = Some(extras.wallpaper_strength);

    // Each style's image is tinted with that style's palette.
    let (light_palette, dark_palette) = look.pair.unwrap_or((look.now, look.now));
    let render = |source: &str, palette: &Palette| {
        crate::wallpaper::render(
            &dirs.data,
            Path::new(source),
            palette,
            extras.wallpaper_tint,
            extras.wallpaper_strength,
        )
    };
    let light_file = light.map(|s| render(s, light_palette)).transpose()?;
    let dark_file = dark
        .or(light)
        .map(|s| render(s, dark_palette))
        .transpose()?;
    let uri = |path: &PathBuf| gio::File::for_path(path).uri().to_string();
    // Light alone covers both styles; dark alone leaves light style as it was.
    let light_uri = light_file.as_ref().map_or(previous.picture_uri, uri);
    let dark_uri = dark_file.as_ref().map_or(previous.picture_uri_dark, uri);
    // Only on a change: the Shell redraws the background on every write.
    for (key, value) in [("picture-uri", light_uri), ("picture-uri-dark", dark_uri)] {
        if settings.string(key) != value {
            desktop::set_string(&settings, key, &value)?;
        }
    }
    Ok(())
}

/// Switches GTK3 to adw-gtk3, in the variant matching the palette in effect, or
/// puts the user's GTK3 theme back.
fn set_adw_gtk3(dirs: &Dirs, state: &mut State, palette: Option<&Palette>) -> Result<(), String> {
    let theme = palette.map(|p| match p.variant {
        Variant::Dark => "adw-gtk3-dark",
        Variant::Light => "adw-gtk3",
    });
    if theme.is_some() && !desktop::adw_gtk3_installed(&dirs.data) {
        return Err("install adw-gtk3 first (sudo dnf install adw-gtk3-theme)".into());
    }
    // Only on a change: GTK3 apps reload their theme on every write.
    let current = desktop::interface_settings().map(|s| s.string("gtk-theme").to_string());
    if theme.is_some() && current.as_deref() == theme {
        return Ok(());
    }
    set_interface(&mut state.gtk_theme_previous, "gtk-theme", theme)
}

fn set_interface(
    previous: &mut Option<String>,
    key: &str,
    value: Option<&str>,
) -> Result<(), String> {
    let settings =
        || desktop::interface_settings().ok_or("GNOME's interface settings are not installed");
    match value {
        Some(value) => {
            let settings = settings()?;
            if previous.is_none() {
                *previous = Some(settings.string(key).to_string());
            }
            desktop::set_string(&settings, key, value)
        }
        None => match previous.clone() {
            Some(value) => {
                desktop::set_string(&settings()?, key, &value)?;
                *previous = None;
                Ok(())
            }
            None => Ok(()),
        },
    }
}

fn set_ptyxis(dirs: &Dirs, state: &mut State, look: Option<Look>) -> Result<(), String> {
    let Some(look) = look else {
        ptyxis::remove(&dirs.data).map_err(|e| e.to_string())?;
        if let Some(previous) = state.ptyxis_previous.clone() {
            let profile = desktop::ptyxis_profile()
                .ok_or("Ptyxis is gone, so its palette was not put back")?;
            desktop::set_string(&profile, "palette", &previous)?;
            state.ptyxis_previous = None;
        }
        return Ok(());
    };

    let profile = desktop::ptyxis_profile().ok_or("Ptyxis is not installed")?;
    let current = profile.string("palette").to_string();
    // Recoolor's own palette is never the user's; fall back to Ptyxis's default.
    let recorded = if current == ptyxis::PALETTE_ID {
        "gnome".to_owned()
    } else {
        current.clone()
    };
    let previous = state.ptyxis_previous.get_or_insert(recorded).clone();
    if let Some(name) = ptyxis::builtin(look.now, look.pair) {
        ptyxis::remove(&dirs.data).map_err(|e| e.to_string())?;
        if current == name {
            return Ok(());
        }
        return desktop::set_string(&profile, "palette", name);
    }
    let text = ptyxis::render(look.now, look.pair);
    let changed = std::fs::read_to_string(ptyxis::path(&dirs.data)).ok() != Some(text.clone());
    ptyxis::write(&dirs.data, &text).map_err(|e| e.to_string())?;
    if current == ptyxis::PALETTE_ID {
        // Ptyxis reads a palette when a profile switches to it, so switch away and
        // back, but only when the file changed.
        if !changed || previous == ptyxis::PALETTE_ID {
            return Ok(());
        }
        desktop::set_string(&profile, "palette", &previous)?;
    }
    desktop::set_string(&profile, "palette", ptyxis::PALETTE_ID)
}

fn set_fish(dirs: &Dirs, palette: Option<&Palette>) -> Result<(), String> {
    match palette {
        Some(p) => fish::write(&dirs.config, &fish::render(p, &desktop::fish_themes())),
        None => fish::remove(&dirs.config).map(|_| ()),
    }
    .map_err(|e| e.to_string())
}

fn set_btop(dirs: &Dirs, state: &mut State, palette: Option<&Palette>) -> Result<(), String> {
    let err = |e: ApplyError| e.to_string();
    let conf = btop::conf_path(&dirs.config);
    let text = btop::read_conf(&conf).map_err(err)?;
    let wanted = match palette {
        None => {
            btop::remove_theme(&dirs.config).map_err(err)?;
            let Some(previous) = state.btop_previous.take() else {
                return Ok(());
            };
            previous
        }
        Some(p) => {
            if !conf.exists() {
                return Err("open btop once first, so it has a config file".into());
            }
            btop::write_theme(&dirs.config, &btop::render(p)).map_err(err)?;
            state.btop_previous.get_or_insert_with(|| {
                btop::current_theme(&text).unwrap_or_else(|| btop::DEFAULT_THEME.into())
            });
            btop::theme_path(&dirs.config).display().to_string()
        }
    };
    let selected = btop::set_theme(&text, &wanted);
    if selected != text {
        btop::write_conf(&conf, &selected).map_err(err)?;
    }
    desktop::reload_btop();
    Ok(())
}

/// Blur my Shell's popups and SearchLightNG in the palette, in Recoolor's extension
/// stylesheet (see `css::shell_popups`, `css::searchlight`). Returns whether it changed.
fn set_popups(dirs: &Dirs, css: &str) -> Result<bool, String> {
    if !shell_fade::is_installed(&dirs.data) {
        return Ok(false);
    }
    let path = shell_fade::stylesheet_path(&dirs.data);
    let before = std::fs::read_to_string(&path).unwrap_or_default();
    shell_fade::write_stylesheet(&dirs.data, css).map_err(|e| e.to_string())?;
    Ok(std::fs::read_to_string(&path).unwrap_or_default() != before)
}

/// The Shell extension's jobs: Screen Fade, the lock screen image (tinted like the
/// desktop's, with the palette in effect now), and Blur my Shell's popups while
/// the Shell is themed. Installed while any is on.
fn set_shell_extension(
    dirs: &Dirs,
    state: &mut State,
    extras: &Extras,
    look: Option<Look>,
) -> Result<(), String> {
    let source = extras.lock_image.as_deref().filter(|_| look.is_some());
    let image = match (source, look) {
        (Some(source), Some(look)) => {
            if !Path::new(source).is_file() {
                return Err(format!("{source} is not there any more"));
            }
            let file = crate::wallpaper::render(
                &dirs.data,
                Path::new(source),
                look.now,
                extras.wallpaper_tint,
                extras.wallpaper_strength,
            )?;
            Some(file.display().to_string())
        }
        _ => None,
    };
    state.lock_source = source.map(str::to_owned);
    let config = shell_fade::Config {
        fade: extras.fade && look.is_some(),
        lock_image: image,
        lock_blur: extras.lock_blur,
        popups: extras.shell
            && look.is_some()
            && (desktop::blur_my_shell_css(&dirs.data).is_some()
                || desktop::searchlight_installed(&dirs.data)),
    };
    set_fade(dirs, config.wanted())?;
    shell_fade::write_config(&dirs.data, &config).map_err(|e| e.to_string())
}

/// Deletes Recoolor's tinted wallpapers that neither the desktop nor the lock
/// screen uses any more.
fn prune_wallpapers(dirs: &Dirs) {
    let mut keep: Vec<PathBuf> = desktop::background_settings()
        .map(|s| {
            ["picture-uri", "picture-uri-dark"]
                .iter()
                .filter_map(|key| gio::File::for_uri(&s.string(key)).path())
                .collect()
        })
        .unwrap_or_default();
    keep.extend(
        shell_fade::read_config(&dirs.data)
            .lock_image
            .map(PathBuf::from),
    );
    let keep: Vec<&Path> = keep.iter().map(PathBuf::as_path).collect();
    wallpaper::prune(&dirs.data, &keep);
}

fn set_fade(dirs: &Dirs, on: bool) -> Result<(), String> {
    if on {
        shell_fade::install(&dirs.data).map_err(|e| e.to_string())?;
        desktop::set_extension_enabled(shell_fade::UUID, true)
    } else {
        // Only take it out of the enabled list if Recoolor put it there.
        if shell_fade::is_installed(&dirs.data) {
            desktop::set_extension_enabled(shell_fade::UUID, false)?;
        }
        shell_fade::remove(&dirs.data)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

/// Firefox or Thunderbird: `profiles` finds the app's profile folders.
fn set_mozilla(
    app: &str,
    profiles: fn(&Path) -> Vec<PathBuf>,
    palette: Option<&Palette>,
) -> Result<(), String> {
    let profiles = profiles(&desktop::home()?);
    if palette.is_some() && profiles.is_empty() {
        return Err(format!("no {app} profile found; open {app} once first"));
    }
    for profile in &profiles {
        match palette {
            Some(p) => firefox::apply(profile, p),
            None => firefox::remove(profile),
        }
        .map_err(|e| e.to_string())?;
    }
    // Recoolor cannot set this one with an exact undo (Firefox copies user.js
    // into prefs.js), so it says so instead.
    if palette.is_some() && !profiles.iter().any(|p| firefox::stylesheets_enabled(p)) {
        return Err(format!(
            "turn on toolkit.legacyUserProfileCustomizations.stylesheets in {app}'s config editor, then restart {app}"
        ));
    }
    Ok(())
}

/// GNOME Text Editor: Recoolor's GtkSourceView schemes (light and dark, which
/// Text Editor switches between itself), selected in its settings.
fn set_text_editor(dirs: &Dirs, state: &mut State, look: Option<Look>) -> Result<(), String> {
    let Some(look) = look else {
        gtksourceview::remove(&dirs.data).map_err(|e| e.to_string())?;
        if let Some(previous) = state.text_editor_previous.clone() {
            let settings = desktop::text_editor_settings()
                .ok_or("Text Editor is gone, so its scheme was not put back")?;
            desktop::set_string(&settings, "style-scheme", &previous)?;
            state.text_editor_previous = None;
        }
        return Ok(());
    };
    let settings = desktop::text_editor_settings().ok_or("GNOME Text Editor is not installed")?;
    let (light, dark) = look.pair.unwrap_or((look.now, look.now));
    let path = gtksourceview::path(&dirs.data, gtksourceview::SCHEME);
    let before = std::fs::read_to_string(&path).ok();
    gtksourceview::write(&dirs.data, light, dark).map_err(|e| e.to_string())?;
    let current = settings.string("style-scheme").to_string();
    let previous = state
        .text_editor_previous
        .get_or_insert_with(|| {
            // Recoolor's own scheme is never the user's; fall back to the default.
            if current.starts_with(gtksourceview::SCHEME) {
                "Adwaita".to_owned()
            } else {
                current.clone()
            }
        })
        .clone();
    if current.starts_with(gtksourceview::SCHEME) {
        // Text Editor reloads a scheme when it switches to it, so switch away and
        // back, but only when the files changed.
        if before == std::fs::read_to_string(&path).ok() {
            return Ok(());
        }
        desktop::set_string(&settings, "style-scheme", &previous)?;
    }
    desktop::set_string(&settings, "style-scheme", gtksourceview::SCHEME)
}

fn set_kde(dirs: &Dirs, palette: Option<&Palette>) -> Result<(), String> {
    match palette {
        Some(p) => kdeglobals::write(&dirs.config, p).map_err(|e| match e {
            ApplyError::Foreign { .. } => {
                "you have your own ~/.config/kdeglobals, so Recoolor leaves it alone".to_owned()
            }
            e => e.to_string(),
        }),
        None => kdeglobals::remove(&dirs.config)
            .map(drop)
            .map_err(|e| e.to_string()),
    }
}

fn set_spotifast(dirs: &Dirs, palette: Option<&Palette>) -> Result<(), String> {
    let Some(dir) = spotifast::config_dir(&desktop::home()?, &dirs.config) else {
        return match palette {
            Some(_) => Err("open Spotifast once first, so it has a config folder".into()),
            None => Ok(()),
        };
    };
    match palette {
        Some(p) => spotifast::write(&dir, p),
        None => spotifast::remove(&dir),
    }
    .map_err(|e| e.to_string())
}

fn set_vscode(dirs: &Dirs, state: &mut State, palette: Option<&Palette>) -> Result<(), String> {
    let home = desktop::home()?;
    let path = vscode::settings_path(&dirs.config);
    let err = |e: ApplyError| e.to_string();
    let Some(palette) = palette else {
        vscode::remove_extension(&home).map_err(err)?;
        if let Some(previous) = state.vscode_previous.clone() {
            let text = vscode::read_settings(&path).map_err(err)?;
            let restored = if previous.is_empty() {
                vscode::remove_theme(&text)
            } else {
                vscode::set_theme(&text, &previous)
            };
            if restored != text {
                vscode::write_settings(&path, &restored).map_err(err)?;
            }
            state.vscode_previous = None;
        }
        return Ok(());
    };

    if !desktop::vscode_installed(&dirs.config) {
        return Err("open VS Code once first, so it has a settings file".into());
    }
    vscode::write_extension(&home, palette).map_err(err)?;
    let text = vscode::read_settings(&path).map_err(err)?;
    state
        .vscode_previous
        .get_or_insert_with(|| vscode::current_theme(&text).unwrap_or_default());
    if vscode::auto_detects(&text) {
        return Err("VS Code follows the system style (window.autoDetectColorScheme), so it ignores the selected theme; turn that off, or pick Recoolor Dark and Light as its preferred themes".into());
    }
    let selected = vscode::set_theme(&text, vscode::label(palette));
    if selected != text {
        vscode::write_settings(&path, &selected).map_err(err)?;
        // VS Code reads each theme file once per session and watches only the current
        // one, so the theme it switches to may be stale. Rewrite it unchanged once VS
        // Code has switched, which reloads it.
        // ponytail: the CLI exits before this runs; only the window and the watcher get it.
        let theme = vscode::theme_path(&home, palette.variant);
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(2));
            if let Ok(contents) = std::fs::read(&theme) {
                let _ = std::fs::write(&theme, contents);
            }
        });
    }
    Ok(())
}

/// `recoolor watch`: keeps what Recoolor applied in step with GNOME. A light/dark switch
/// rewrites everything for a pair; an accent change (Settings → Appearance)
/// rewrites everything with the palette's matching color. Runs until killed.
pub fn watch(dirs: Dirs) -> Result<(), String> {
    let settings =
        desktop::interface_settings().ok_or("GNOME's interface settings are not installed")?;
    // GSettings also reports a key as changed when others in the same folder change
    // (such as the cursor Recoolor sets itself), so only act on a real change.
    let last = std::cell::RefCell::new(None);
    let sync = move |settings: &gio::Settings| {
        // An update replaced recoolor on disk: exit so systemd (Restart=on-failure)
        // starts the new one, which syncs as it starts. Otherwise this old one keeps
        // running until the next login.
        if std::fs::read_link("/proc/self/exe")
            .is_ok_and(|p| p.to_string_lossy().ends_with(" (deleted)"))
        {
            log::info("recoolor watch: recoolor was updated, restarting");
            std::process::exit(1);
        }
        let now = (
            desktop::prefers_dark(settings),
            desktop::accent_choice(settings),
        );
        if last.borrow().as_ref() != Some(&now) {
            *last.borrow_mut() = Some(now);
            if let Err(e) = sync(&dirs, settings) {
                log::error(&format!("recoolor watch: {e}"));
                eprintln!("recoolor watch: {e}");
            }
        }
        // Recoolor's Shell extension froze the screen on the switch; fade to the new
        // look, also when there was nothing to do.
        desktop::screen_fade("Fade");
    };
    // Reading the keys here also subscribes to their changes.
    sync(&settings);
    let sync = Rc::new(sync);
    for key in ["color-scheme", "accent-color"] {
        let sync = sync.clone();
        settings.connect_changed(Some(key), move |s, _| sync(s));
    }
    glib::MainLoop::new(None, false).run();
    Ok(())
}

/// Rewrites Recoolor's files for the current style and accent, for the GTK palette
/// and every extra that is on.
fn sync(dirs: &Dirs, settings: &gio::Settings) -> Result<(), String> {
    let dark_now = desktop::prefers_dark(settings);
    let accent = desktop::accent_choice(settings);
    let extras = current(dirs);
    let cfg = GtkConfig::new(&dirs.config).with_adw_gtk3(extras.adw_gtk3);
    let statuses = cfg.status().map_err(|e| e.to_string())?;
    let gtk4 = statuses
        .into_iter()
        .find(|s| s.toolkit == Toolkit::Gtk4)
        .map(|s| s.css);
    let (palettes, _) = palette::catalog(paths::user_palettes_dir().as_deref());
    let find = |id: &str| -> Result<Palette, String> {
        palettes
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.with_accent(accent.as_deref()))
            .ok_or_else(|| format!("palette {id:?} is gone"))
    };
    let tinted = match gtk4 {
        Some(CssState::Recoolor(id)) => vec![find(&id)?],
        Some(CssState::RecoolorPair { light, dark }) => vec![find(&light)?, find(&dark)?],
        // After a restore there is nothing to keep up to date.
        _ => return Ok(()),
    };
    let look = look(&tinted, dark_now);
    write_gtk(&cfg, look, dark_now).map_err(|e| e.to_string())?;

    let problems = set_extras(dirs, &extras, Some(look));
    for p in &problems {
        log::error(&format!("recoolor watch: {p}"));
    }
    log::info(&format!(
        "recoolor watch: now using {} (accent {})",
        look.now.name,
        accent.as_deref().unwrap_or("palette's own")
    ));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs `reset_restores_inner` in a child process on GLib's in-memory settings
    /// backend, so the test never touches the real desktop's settings.
    #[test]
    fn reset_restores_the_users_settings() {
        let out = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "targets::tests::reset_restores_inner",
            ])
            .env("GSETTINGS_BACKEND", "memory")
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            out.status.success() && stdout.contains("1 passed"),
            "{stdout}"
        );
    }

    #[test]
    #[ignore = "run by reset_restores_the_users_settings with the memory backend"]
    fn reset_restores_inner() {
        assert_eq!(
            std::env::var("GSETTINGS_BACKEND").as_deref(),
            Ok("memory"),
            "refusing to change the real settings"
        );
        let (Some(interface), Some(background)) = (
            desktop::interface_settings(),
            desktop::background_settings(),
        ) else {
            eprintln!("skipped: GNOME's settings schemas are not installed");
            return;
        };
        let read = |s: &gio::Settings, key: &str| s.string(key).to_string();
        let before = [
            read(&background, "picture-uri"),
            read(&background, "picture-uri-dark"),
            read(&interface, "icon-theme"),
            read(&interface, "color-scheme"),
        ];

        let root = std::env::temp_dir().join(format!("recoolor-test-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let image = root.join("tree.png");
        std::fs::write(&image, b"not decoded with Original").unwrap();
        let dirs = Dirs {
            config: root.join("config"),
            data: root.join("data"),
            state: root.join("state"),
        };
        let mocha = palette::bundled()
            .into_iter()
            .find(|p| p.id == "catppuccin-mocha")
            .unwrap();
        let look = Look {
            now: &mocha,
            pair: None,
        };
        let extras = Extras {
            wallpaper: Some(image.display().to_string()),
            wallpaper_tint: Tint::None,
            ..Extras::default()
        };
        let mut state = State::default();

        set_wallpaper(&dirs, &mut state, &extras, Some(look)).unwrap();
        set_interface(&mut state.icons_previous, "icon-theme", Some("Papirus")).unwrap();
        set_light_shell(&mut state, true).unwrap();
        assert_eq!(
            read(&background, "picture-uri"),
            gio::File::for_path(&image).uri()
        );
        assert_eq!(read(&interface, "icon-theme"), "Papirus");
        // GNOME's default `default` becomes `prefer-light` while the Shell is themed.
        if before[3] == "default" {
            assert_eq!(read(&interface, "color-scheme"), "prefer-light");
        }

        set_wallpaper(&dirs, &mut state, &Extras::default(), None).unwrap();
        set_interface(&mut state.icons_previous, "icon-theme", None).unwrap();
        set_light_shell(&mut state, false).unwrap();
        let after = [
            read(&background, "picture-uri"),
            read(&background, "picture-uri-dark"),
            read(&interface, "icon-theme"),
            read(&interface, "color-scheme"),
        ];
        assert_eq!(after, before);
        assert_eq!(state, State::default(), "nothing left to restore");
        let _ = std::fs::remove_dir_all(&root);
    }
}
