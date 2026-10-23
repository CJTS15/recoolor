//! The parts of applying a palette that talk to the running desktop: GNOME's
//! light/dark setting and the `pinta watch` systemd user service. Shared by the
//! command line and the window.

use std::path::{Path, PathBuf};
use std::process::Command;

use gtk::gio;
use gtk::gio::prelude::*;
use gtk::glib;
use pinta_core::palette::{self, Palette};
use pinta_core::{CssState, GtkConfig, Toolkit, paths, service};

use crate::log;

const INTERFACE_SCHEMA: &str = "org.gnome.desktop.interface";

pub fn config_home() -> Result<PathBuf, String> {
    paths::config_home()
        .ok_or_else(|| "cannot find your config folder; set HOME or XDG_CONFIG_HOME".to_owned())
}

/// GNOME's interface settings, or `None` off GNOME, where `gio::Settings::new` would abort.
fn interface_settings() -> Option<gio::Settings> {
    let schema = gio::SettingsSchemaSource::default()?.lookup(INTERFACE_SCHEMA, true)?;
    Some(gio::Settings::new_full(
        &schema,
        None::<&gio::SettingsBackend>,
        None,
    ))
}

fn prefers_dark(settings: &gio::Settings) -> bool {
    settings.string("color-scheme") == "prefer-dark"
}

/// Whether GNOME is in dark style. `false` off GNOME.
pub fn system_prefers_dark() -> bool {
    interface_settings().is_some_and(|s| prefers_dark(&s))
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

/// `pinta watch`: rewrites the GTK3 stylesheet whenever GNOME switches between light
/// and dark, as long as a light/dark pair is applied. Runs until killed.
pub fn watch(config_home: PathBuf) -> Result<(), String> {
    let settings = interface_settings().ok_or("GNOME's interface settings are not installed")?;
    let cfg = GtkConfig::new(config_home);
    let sync = move |settings: &gio::Settings| {
        if let Err(e) = sync_gtk3(&cfg, prefers_dark(settings)) {
            log::error(&format!("pinta watch: {e}"));
            eprintln!("pinta watch: {e}");
        }
    };
    // Reading the key here also subscribes to its changes.
    sync(&settings);
    settings.connect_changed(Some("color-scheme"), move |s, _| sync(s));
    glib::MainLoop::new(None, false).run();
    Ok(())
}

fn sync_gtk3(cfg: &GtkConfig, dark_now: bool) -> Result<(), String> {
    let statuses = cfg.status().map_err(|e| e.to_string())?;
    let gtk4 = statuses.iter().find(|s| s.toolkit == Toolkit::Gtk4);
    // After a restore or a single-palette apply there is nothing to follow.
    let Some(CssState::PintaPair { light, dark }) = gtk4.map(|s| &s.css) else {
        return Ok(());
    };
    let (palettes, _) = palette::catalog(paths::user_palettes_dir().as_deref());
    let find = |id: &str| -> Result<&Palette, String> {
        palettes
            .iter()
            .find(|p| p.id == id)
            .ok_or_else(|| format!("palette {id:?} is gone"))
    };
    let (light, dark) = (find(light)?, find(dark)?);
    cfg.apply_pair(light, dark, dark_now)
        .map_err(|e| e.to_string())?;
    let now = if dark_now { dark } else { light };
    log::info(&format!("pinta watch: GTK3 now uses {}", now.name));
    Ok(())
}
