//! The parts of Recoolor that talk to the running desktop: GNOME settings, the
//! Shell's built-in stylesheet, and the `recoolor watch` systemd user service.

use std::path::{Path, PathBuf};
use std::process::Command;

use gtk::gio;
use gtk::gio::prelude::*;
use gtk::glib;
use recoolor_core::apps::{fish, shell_fade};
use recoolor_core::{Palette, Variant, css, paths, service, shell};

const INTERFACE_SCHEMA: &str = "org.gnome.desktop.interface";
const USER_THEME_SCHEMA: &str = "org.gnome.shell.extensions.user-theme";
pub const USER_THEME_UUID: &str = "user-theme@gnome-shell-extensions.gcampax.github.com";

/// The XDG folders Recoolor writes to.
#[derive(Clone, Debug)]
pub struct Dirs {
    pub config: PathBuf,
    pub data: PathBuf,
    pub state: PathBuf,
}

impl Dirs {
    pub fn find() -> Result<Self, String> {
        match (
            paths::config_home(),
            paths::data_home(),
            paths::state_home(),
        ) {
            (Some(config), Some(data), Some(state)) => Ok(Self {
                config,
                data,
                state,
            }),
            _ => Err("cannot find your home folders; set HOME or the XDG_*_HOME variables".into()),
        }
    }
}

/// A settings schema from the default source, or else from one of `extra_dirs`
/// (extensions ship their schemas in their own folder). `None` rather than the
/// abort `gio::Settings::new` gives for a missing schema. `path` is for
/// relocatable schemas, such as one Ptyxis profile.
fn settings(schema_id: &str, extra_dirs: &[PathBuf], path: Option<&str>) -> Option<gio::Settings> {
    let default = gio::SettingsSchemaSource::default();
    let schema = default
        .as_ref()
        .and_then(|source| source.lookup(schema_id, true))
        .or_else(|| {
            extra_dirs.iter().find_map(|dir| {
                gio::SettingsSchemaSource::from_directory(dir, default.as_ref(), false)
                    .ok()?
                    .lookup(schema_id, false)
            })
        })?;
    Some(gio::Settings::new_full(
        &schema,
        None::<&gio::SettingsBackend>,
        path,
    ))
}

/// GNOME's desktop background settings, or `None` off GNOME.
pub fn background_settings() -> Option<gio::Settings> {
    settings("org.gnome.desktop.background", &[], None)
}

/// GNOME Text Editor's settings, or `None` without it.
pub fn text_editor_settings() -> Option<gio::Settings> {
    settings("org.gnome.TextEditor", &[], None)
}

/// GNOME's interface settings, or `None` off GNOME.
pub fn interface_settings() -> Option<gio::Settings> {
    settings(INTERFACE_SCHEMA, &[], None)
}

pub fn prefers_dark(settings: &gio::Settings) -> bool {
    settings.string("color-scheme") == "prefer-dark"
}

/// Whether GNOME is in dark style. `false` off GNOME.
pub fn system_prefers_dark() -> bool {
    interface_settings().is_some_and(|s| prefers_dark(&s))
}

/// The accent picked in Settings → Appearance, or `None` while it is still GNOME's
/// default (or on GNOME before 47, which has no accent setting).
pub fn accent_choice(settings: &gio::Settings) -> Option<String> {
    if !settings.settings_schema()?.has_key("accent-color") {
        return None;
    }
    settings
        .user_value("accent-color")?
        .str()
        .map(str::to_owned)
}

/// Sets a string key and waits for it to reach dconf, so a command that exits
/// right after does not lose it.
pub fn set_string(settings: &gio::Settings, key: &str, value: &str) -> Result<(), String> {
    settings
        .set_string(key, value)
        .map_err(|e| format!("cannot set {key}: {e}"))?;
    gio::Settings::sync();
    Ok(())
}

/// The User Themes extension's settings, or `None` when it is not installed.
pub fn user_theme_settings(data_home: &Path) -> Option<gio::Settings> {
    let dirs: Vec<PathBuf> = std::iter::once(data_home.to_owned())
        .chain(glib::system_data_dirs())
        .map(|d| {
            d.join("gnome-shell/extensions")
                .join(USER_THEME_UUID)
                .join("schemas")
        })
        .collect();
    settings(USER_THEME_SCHEMA, &dirs, None)
}

/// Whether GNOME Shell has the User Themes extension turned on.
pub fn user_theme_enabled() -> bool {
    settings("org.gnome.shell", &[], None).is_some_and(|s| {
        !s.boolean("disable-user-extensions")
            && s.strv("enabled-extensions")
                .iter()
                .any(|uuid| uuid == USER_THEME_UUID)
    })
}

/// Makes the Shell load Recoolor's theme again: through Recoolor's extension when it
/// runs, in one load. Otherwise set the User Themes name away and back, the only
/// change it reloads on, which loads the default theme in between (a flash).
pub fn reload_shell_theme(settings: &gio::Settings) -> Result<(), String> {
    let reloaded = running(shell_fade::BUS_NAME)
        && session_call(
            shell_fade::BUS_NAME,
            shell_fade::OBJECT_PATH,
            shell_fade::BUS_NAME,
            "ReloadTheme",
            None,
        )
        .is_ok();
    if reloaded {
        return Ok(());
    }
    set_string(settings, "name", "")?;
    set_string(settings, "name", shell::THEME_NAME)
}

/// The installed Shell's own stylesheet for `variant`, from its gresource.
pub fn shell_default_css(variant: Variant) -> Result<String, String> {
    shell_stylesheet(match variant {
        Variant::Dark => "gnome-shell-dark.css",
        Variant::Light => "gnome-shell-light.css",
    })
}

/// One of the installed Shell's stylesheets, from its gresource.
pub fn shell_stylesheet(file: &str) -> Result<String, String> {
    glib::system_data_dirs()
        .iter()
        .filter_map(|dir| {
            gio::Resource::load(dir.join("gnome-shell/gnome-shell-theme.gresource")).ok()
        })
        .find_map(|resource| {
            resource
                .lookup_data(
                    &format!("/org/gnome/shell/theme/{file}"),
                    gio::ResourceLookupFlags::NONE,
                )
                .ok()
        })
        .ok_or_else(|| format!("cannot find GNOME Shell's {file}"))
        .and_then(|bytes| String::from_utf8(bytes.to_vec()).map_err(|e| e.to_string()))
}

fn session_call(
    dest: &str,
    path: &str,
    iface: &str,
    method: &str,
    args: Option<glib::Variant>,
) -> Result<glib::Variant, String> {
    gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE)
        .and_then(|bus| {
            bus.call_sync(
                Some(dest),
                path,
                iface,
                method,
                args.as_ref(),
                None,
                gio::DBusCallFlags::NONE,
                -1,
                gio::Cancellable::NONE,
            )
        })
        .map_err(|e| e.to_string())
}

/// Whether something owns `name` on the session bus. Checked before calling an
/// app that is D-Bus activatable, so the call does not start it.
fn running(name: &str) -> bool {
    session_call(
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
        "NameHasOwner",
        Some((name,).to_variant()),
    )
    .is_ok_and(|reply| reply.get::<(bool,)>().is_some_and(|(owned,)| owned))
}

/// Restarts apps that keep running in the background after their windows close,
/// so they reopen with the new styles (GTK reads them only at startup). Files is
/// quit (`kill`, what `nautilus -q` runs; `quit` only closes its windows).
/// Calendar's service shows event reminders, so it is started again, through
/// D-Bus activation so that a Flatpak Calendar comes back as well.
pub fn restart_background_apps() -> Result<(), String> {
    let mut problems = Vec::new();
    if running("org.gnome.Nautilus")
        && let Err(e) = activate_action("org.gnome.Nautilus", "/org/gnome/Nautilus", "kill")
    {
        problems.push(format!("cannot quit Files: {e}"));
    }
    const CALENDAR: &str = "org.gnome.Calendar";
    if running(CALENDAR) {
        // Its `quit` action keeps the reminder service alive, so stop the process.
        let restarted = terminate(CALENDAR).and_then(|()| {
            // A new service would only hand over to the old one while it lives.
            for _ in 0..60 {
                if !running(CALENDAR) {
                    return session_call(
                        "org.freedesktop.DBus",
                        "/org/freedesktop/DBus",
                        "org.freedesktop.DBus",
                        "StartServiceByName",
                        Some((CALENDAR, 0u32).to_variant()),
                    )
                    .map(drop);
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err("it did not quit".into())
        });
        if let Err(e) = restarted {
            problems.push(format!("cannot restart Calendar: {e}"));
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("; "))
    }
}

/// Sends SIGTERM to the process that owns `name` on the session bus.
fn terminate(name: &str) -> Result<(), String> {
    let reply = session_call(
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
        "GetConnectionUnixProcessID",
        Some((name,).to_variant()),
    )?;
    let (pid,) = reply.get::<(u32,)>().ok_or("no process id")?;
    let status = Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status()
        .map_err(|e| e.to_string())?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| format!("kill failed ({status})"))
}

fn activate_action(name: &str, path: &str, action: &str) -> Result<(), String> {
    let no_params: Vec<glib::Variant> = Vec::new();
    let no_platform: std::collections::HashMap<String, glib::Variant> = Default::default();
    session_call(
        name,
        path,
        "org.gtk.Actions",
        "Activate",
        Some((action, no_params, no_platform).to_variant()),
    )
    .map(drop)
}

/// Asks Recoolor's Shell extension to `Freeze` or `Fade` the screen. Does nothing
/// when the extension is not running; a failure only costs the animation.
/// The running apps with windows, as (desktop id, name), from Recoolor's Shell
/// extension; empty without it, since GNOME Shell keeps its own list private.
pub fn running_apps() -> Vec<(String, String)> {
    if !running(shell_fade::BUS_NAME) {
        return Vec::new();
    }
    session_call(
        shell_fade::BUS_NAME,
        shell_fade::OBJECT_PATH,
        shell_fade::BUS_NAME,
        "RunningApps",
        None,
    )
    .ok()
    .and_then(|reply| reply.get::<(Vec<(String, String)>,)>())
    .map(|(apps,)| apps)
    .unwrap_or_default()
}

/// Restarts these apps (desktop ids) through Recoolor's Shell extension: each quits
/// as from the dash, then opens again.
pub fn restart_apps(ids: &[String]) -> Result<(), String> {
    session_call(
        shell_fade::BUS_NAME,
        shell_fade::OBJECT_PATH,
        shell_fade::BUS_NAME,
        "RestartApps",
        Some((ids.to_vec(),).to_variant()),
    )
    .map(|_| ())
}

/// Tells every open fish shell to take Recoolor's colors again, in `style` (`light` or
/// `dark`), or `None` to stop (Reset). fish shares universal variables with all its
/// shells; the count makes each change a change.
pub fn reload_fish(style: Option<&str>) {
    let command = match style {
        Some(style) => {
            let count = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos());
            format!("set --universal {} '{style} {count}'", fish::RELOAD)
        }
        None => format!("set --erase --universal {}", fish::RELOAD),
    };
    if let Err(e) = Command::new("fish").args(["-c", &command]).status() {
        crate::log::error(&format!("cannot tell fish shells to recolor: {e}"));
    }
}

pub fn screen_fade(method: &str) {
    if !running(shell_fade::BUS_NAME) {
        return;
    }
    // GSettings writes leave asynchronously; fading before they land (the switch
    // to light rewrites `color-scheme` to `prefer-light`) shows the Shell and apps
    // restyling mid-fade.
    if method == "Fade" {
        gio::Settings::sync();
    }
    if let Err(e) = session_call(
        shell_fade::BUS_NAME,
        shell_fade::OBJECT_PATH,
        shell_fade::BUS_NAME,
        method,
        None,
    ) {
        crate::log::error(&format!("screen fade: {e}"));
    }
}

/// Adds `uuid` to, or takes it out of, the Shell's enabled extensions. A newly
/// installed extension is only picked up after logging in again on Wayland.
pub fn set_extension_enabled(uuid: &str, on: bool) -> Result<(), String> {
    let shell = settings("org.gnome.shell", &[], None).ok_or("GNOME Shell is not installed")?;
    let mut enabled: Vec<String> = shell
        .strv("enabled-extensions")
        .iter()
        .map(|s| s.to_string())
        .collect();
    let present = enabled.iter().any(|u| u == uuid);
    if on == present {
        return Ok(());
    }
    if on {
        enabled.push(uuid.to_owned());
    } else {
        enabled.retain(|u| u != uuid);
    }
    let enabled: Vec<&str> = enabled.iter().map(String::as_str).collect();
    shell
        .set_strv("enabled-extensions", enabled.as_slice())
        .map_err(|e| format!("cannot change enabled extensions: {e}"))?;
    gio::Settings::sync();
    Ok(())
}

/// The settings of Ptyxis's default profile, or `None` without Ptyxis.
pub fn ptyxis_profile() -> Option<gio::Settings> {
    let uuid = settings("org.gnome.Ptyxis", &[], None)?.string("default-profile-uuid");
    if uuid.is_empty() {
        return None;
    }
    settings(
        "org.gnome.Ptyxis.Profile",
        &[],
        Some(&format!("/org/gnome/Ptyxis/Profiles/{uuid}/")),
    )
}

pub fn fish_installed() -> bool {
    Command::new("fish")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// btop writes its config on first run.
pub fn btop_installed(config_home: &Path) -> bool {
    config_home.join("btop/btop.conf").is_file()
}

/// Running btops reread their config, and so the theme, on SIGUSR2. None
/// running is fine.
pub fn reload_btop() {
    let _ = Command::new("pkill").args(["-USR2", "-x", "btop"]).status();
}

/// VS Code keeps its user settings here once it has run.
pub fn vscode_installed(config_home: &Path) -> bool {
    config_home.join("Code/User").is_dir()
}

pub fn home() -> Result<PathBuf, String> {
    std::env::home_dir().ok_or_else(|| "cannot find your home folder".to_owned())
}

/// The Shell theme for `palette`: the installed Shell's stylesheet recolored, plus
/// Blur my Shell's translucent popup rules when it is installed.
pub fn shell_css(palette: &Palette, data_home: &Path) -> Result<String, String> {
    let mut css = css::shell(palette, &shell_default_css(palette.variant)?);
    if let Some(bms) = blur_my_shell_css(data_home) {
        css.push_str(&css::shell_blur(palette, &bms));
    }
    Ok(css)
}

/// The installed Blur my Shell's stylesheet.
pub fn blur_my_shell_css(data_home: &Path) -> Option<String> {
    extension_stylesheet(data_home, "blur-my-shell@aunetx")
}

/// Whether SearchLightNG is installed.
pub fn searchlight_installed(data_home: &Path) -> bool {
    extension_stylesheet(data_home, "search-light-ng@salix.host").is_some()
}

fn extension_stylesheet(data_home: &Path, uuid: &str) -> Option<String> {
    std::iter::once(data_home.to_owned())
        .chain(glib::system_data_dirs())
        .map(|d| {
            d.join("gnome-shell/extensions")
                .join(uuid)
                .join("stylesheet.css")
        })
        .find_map(|p| std::fs::read_to_string(p).ok())
}

/// Whether the adw-gtk3 theme is installed where GTK3 looks for themes.
pub fn adw_gtk3_installed(data_home: &Path) -> bool {
    std::iter::once(data_home.join("themes"))
        .chain(std::env::home_dir().map(|h| h.join(".themes")))
        .chain(
            glib::system_data_dirs()
                .into_iter()
                .map(|d| d.join("themes")),
        )
        .any(|dir| dir.join("adw-gtk3/gtk-3.0/gtk.css").is_file())
}

/// Whether GTK3 Flatpak apps can use adw-gtk3: it is a Flatpak runtime of its own.
/// Looks in Flatpak's user and system installations, without running `flatpak`.
pub fn adw_gtk3_flatpak() -> bool {
    let runtime = "flatpak/runtime/org.gtk.Gtk3theme.adw-gtk3";
    std::iter::once(glib::user_data_dir().join(runtime))
        .chain(std::iter::once(Path::new("/var/lib").join(runtime)))
        .any(|dir| dir.is_dir())
}

pub fn flatpak_installed() -> bool {
    glib::find_program_in_path("flatpak").is_some()
}

/// Turns the `recoolor watch` user service on or off. On writes the unit and (re)starts
/// it; off stops it and removes the unit. Off does nothing if it was never on.
pub fn set_watch(config_home: &Path, on: bool) -> Result<(), String> {
    if on {
        let exe =
            std::env::current_exe().map_err(|e| format!("cannot find recoolor itself: {e}"))?;
        let exe = exe
            .to_str()
            .ok_or("the path to recoolor is not valid UTF-8, so systemd cannot run it")?;
        // The unit names the binary, so an unchanged unit means the running watcher
        // is already the right one: leave it, it follows changes by itself.
        if service::install(config_home, exe).map_err(|e| e.to_string())? {
            systemctl(&["daemon-reload"])?;
            systemctl(&["enable", service::UNIT_NAME])?;
            systemctl(&["restart", service::UNIT_NAME])
        } else if systemctl(&["is-active", "--quiet", service::UNIT_NAME]).is_err() {
            systemctl(&["enable", "--now", service::UNIT_NAME])
        } else {
            Ok(())
        }
    } else if service::is_installed(config_home) {
        // Remove the unit even if systemd no longer knows it, then report.
        let stopped = systemctl(&["disable", "--now", service::UNIT_NAME]);
        service::remove(config_home).map_err(|e| e.to_string())?;
        systemctl(&["daemon-reload"])?;
        stopped
    } else {
        Ok(())
    }
}

fn systemctl(args: &[&str]) -> Result<(), String> {
    let output = Command::new("systemctl")
        .arg("--user")
        .args(args)
        .output()
        .map_err(|e| format!("cannot run systemctl: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "systemctl --user {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}
