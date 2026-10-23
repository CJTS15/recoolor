//! Everything Recoolor changes, applied and undone together. The GTK stylesheets are
//! always written; Flatpak access, the Shell, icon and cursor themes, and the app
//! configs (Ptyxis, fish, VS Code) are extras the user turns on. Shared by the
//! window and the command line.

use std::path::Path;
use std::rc::Rc;

use gtk::gio;
use gtk::gio::prelude::*;
use gtk::glib;
use recoolor_core::apps::{self, firefox, fish, ptyxis, shell_fade, spotifast, vscode};
use recoolor_core::palette::{self, Palette};
use recoolor_core::state::{self, State};
use recoolor_core::{
    ApplyError, CssState, GtkConfig, RestoreOutcome, Toolkit, css, flatpak, paths, shell,
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

impl Choice<'_> {
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
    pub cursors: Option<String>,
    pub ptyxis: bool,
    pub fish: bool,
    pub vscode: bool,
    pub firefox: bool,
    pub spotifast: bool,
    pub fade: bool,
}

/// The extras as they are now, read from disk, the state file and GNOME settings.
pub fn current(dirs: &Dirs) -> Extras {
    let state = State::load(&state::path(&dirs.state)).unwrap_or_default();
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
        cursors: interface(&state.cursors_previous, "cursor-theme"),
        // A built-in palette leaves no file, so the record alone says it is on.
        ptyxis: state.ptyxis_previous.is_some(),
        fish: apps::is_ours(&fish::path(&dirs.config)),
        vscode: state.vscode_previous.is_some()
            && desktop::home().is_ok_and(|h| vscode::extension_installed(&h)),
        firefox: desktop::home()
            .is_ok_and(|h| firefox::profiles(&h).iter().any(|p| firefox::is_applied(p))),
        fade: shell_fade::is_installed(&dirs.data),
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
    desktop::screen_fade("Freeze");
    let applied = write_gtk(&GtkConfig::new(&dirs.config), look, dark_now).map(|()| {
        log::info(&format!(
            "applied {} (accent {}) with {extras:?}",
            choice.name(),
            accent.as_deref().unwrap_or("palette's own")
        ));
        let mut problems = Vec::new();
        if let Err(e) = desktop::quit_files() {
            problems.push(format!("Files: {e}"));
        }
        problems.extend(set_extras(dirs, extras, Some(look)));
        problems
    });
    desktop::screen_fade("Fade");
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
    if let Err(e) = desktop::quit_files() {
        problems.push(format!("Files: {e}"));
    }
    problems.extend(set_extras(dirs, &Extras::default(), None));
    log::info("reset");
    (outcomes, problems)
}

/// `look` is what to show, or `None` to undo everything.
fn set_extras(dirs: &Dirs, extras: &Extras, look: Option<Look>) -> Vec<String> {
    let when = |on: bool| look.filter(|_| on);
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
        (
            "GNOME Shell",
            set_shell(dirs, &mut state, when(extras.shell).map(|l| l.now)),
        ),
        (
            "Light style",
            set_light_shell(&mut state, extras.shell && look.is_some()),
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
            "VS Code",
            set_vscode(dirs, &mut state, when(extras.vscode).map(|l| l.now)),
        ),
        ("Firefox", set_firefox(when(extras.firefox).map(|l| l.now))),
        ("Screen fade", set_fade(dirs, extras.fade)),
        (
            "Spotifast",
            set_spotifast(dirs, when(extras.spotifast).map(|l| l.now)),
        ),
    ];
    let mut problems: Vec<String> = results
        .into_iter()
        .filter_map(|(what, result)| result.err().map(|e| format!("{what}: {e}")))
        .collect();
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
    let css = css::shell(palette, &desktop::shell_default_css(palette.variant)?);
    let changed = std::fs::read_to_string(shell::css_path(&dirs.data)).ok() != Some(css.clone());
    shell::write(&dirs.data, &css).map_err(|e| e.to_string())?;
    let name = settings.string("name").to_string();
    state.shell_previous.get_or_insert_with(|| name.clone());
    // Reloading flashes the default theme, so only when there is something new.
    if changed {
        desktop::reload_shell_theme(&settings)
    } else if name != shell::THEME_NAME {
        desktop::set_string(&settings, "name", shell::THEME_NAME)
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
    let previous = state.ptyxis_previous.get_or_insert(current.clone()).clone();
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

fn set_firefox(palette: Option<&Palette>) -> Result<(), String> {
    let profiles = firefox::profiles(&desktop::home()?);
    if palette.is_some() && profiles.is_empty() {
        return Err("no Firefox profile found; open Firefox once first".into());
    }
    for profile in &profiles {
        match palette {
            Some(p) => firefox::apply(profile, p),
            None => firefox::remove(profile),
        }
        .map_err(|e| e.to_string())?;
    }
    Ok(())
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
    let selected = vscode::set_theme(&text, vscode::label(palette));
    if selected != text {
        vscode::write_settings(&path, &selected).map_err(err)?;
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
        let now = (
            desktop::prefers_dark(settings),
            desktop::accent_choice(settings),
        );
        if last.borrow().as_ref() == Some(&now) {
            return;
        }
        *last.borrow_mut() = Some(now);
        if let Err(e) = sync(&dirs, settings) {
            log::error(&format!("recoolor watch: {e}"));
            eprintln!("recoolor watch: {e}");
        }
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
    let cfg = GtkConfig::new(&dirs.config);
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

    let problems = set_extras(dirs, &current(dirs), Some(look));
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
