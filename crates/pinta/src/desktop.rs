//! The parts of Pinta that talk to the running desktop: GNOME settings, the
//! Shell's built-in stylesheet, and the `pinta watch` systemd user service.

use std::path::{Path, PathBuf};
use std::process::Command;

use gtk::gio;
use gtk::gio::prelude::*;
use gtk::glib;
use pinta_core::apps::shell_fade;
use pinta_core::{Variant, paths, service, shell};

const INTERFACE_SCHEMA: &str = "org.gnome.desktop.interface";
const USER_THEME_SCHEMA: &str = "org.gnome.shell.extensions.user-theme";
pub const USER_THEME_UUID: &str = "user-theme@gnome-shell-extensions.gcampax.github.com";

/// The XDG folders Pinta writes to.
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

/// Makes the Shell load Pinta's theme again. The extension only reloads when the
/// theme name changes, so set it away and back.
pub fn reload_shell_theme(settings: &gio::Settings) -> Result<(), String> {
    set_string(settings, "name", "")?;
    set_string(settings, "name", shell::THEME_NAME)
}

/// The installed Shell's own stylesheet for `variant`, from its gresource.
pub fn shell_default_css(variant: Variant) -> Result<String, String> {
    let file = match variant {
        Variant::Dark => "gnome-shell-dark.css",
        Variant::Light => "gnome-shell-light.css",
    };
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

/// Quits Files (Nautilus) if it is running. It stays in the background after its
/// windows close, so without this it keeps the styles it started with; the next
/// window opens with the new ones.
pub fn quit_files() -> Result<(), String> {
    const NAME: &str = "org.gnome.Nautilus";
    if !running(NAME) {
        return Ok(());
    }
    // `kill`, what `nautilus -q` runs; `quit` only closes the windows.
    let no_params: Vec<glib::Variant> = Vec::new();
    let no_platform: std::collections::HashMap<String, glib::Variant> = Default::default();
    session_call(
        NAME,
        "/org/gnome/Nautilus",
        "org.gtk.Actions",
        "Activate",
        Some(("kill", no_params, no_platform).to_variant()),
    )
    .map(|_| ())
    .map_err(|e| format!("cannot quit Files: {e}"))
}

/// Asks Pinta's Shell extension to `Freeze` or `Fade` the screen. Does nothing
/// when the extension is not running; a failure only costs the animation.
pub fn screen_fade(method: &str) {
    if !running(shell_fade::BUS_NAME) {
        return;
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

/// The themes fish can choose (`fish_config theme list`), or none without fish.
pub fn fish_themes() -> Vec<String> {
    Command::new("fish")
        .args(["-c", "fish_config theme list"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

pub fn fish_installed() -> bool {
    Command::new("fish")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// VS Code keeps its user settings here once it has run.
pub fn vscode_installed(config_home: &Path) -> bool {
    config_home.join("Code/User").is_dir()
}

pub fn home() -> Result<PathBuf, String> {
    std::env::home_dir().ok_or_else(|| "cannot find your home folder".to_owned())
}

pub fn flatpak_installed() -> bool {
    Command::new("flatpak")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Turns the `pinta watch` user service on or off. On writes the unit and (re)starts
/// it; off stops it and removes the unit. Off does nothing if it was never on.
pub fn set_watch(config_home: &Path, on: bool) -> Result<(), String> {
    if on {
        let exe = std::env::current_exe().map_err(|e| format!("cannot find pinta itself: {e}"))?;
        let exe = exe
            .to_str()
            .ok_or("the path to pinta is not valid UTF-8, so systemd cannot run it")?;
        service::install(config_home, exe).map_err(|e| e.to_string())?;
        systemctl(&["daemon-reload"])?;
        systemctl(&["enable", service::UNIT_NAME])?;
        // restart, not start, so a running watcher picks up a new binary.
        systemctl(&["restart", service::UNIT_NAME])
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
