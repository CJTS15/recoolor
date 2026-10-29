//! The login screen, through GDM Settings: Recoolor never writes system files. GDM
//! Settings copies a theme into the system stylesheet when you press its Apply, and
//! lists only system themes, so Recoolor's theme is linked into
//! `/usr/local/share/themes` once (Set Up…). Update… then fills in GDM Settings' choices
//! (in its Flatpak settings file, which it reads at startup) and opens it.

use std::path::{Path, PathBuf};

use gtk::gio;
use gtk::gio::prelude::*;
use gtk::glib;
use recoolor_core::{palette, paths, shell};

use crate::desktop;

/// GDM Settings' app id; its launcher is this plus `.desktop`.
pub const GDM_SETTINGS: &str = "io.github.realmazharhussain.GdmSettings";

/// What the login screen shows, from the marker GDM Settings copied with the theme.
#[derive(Debug, PartialEq, Eq)]
pub enum Shows {
    /// GNOME's own look: GDM Settings never applied Recoolor's theme, or a GNOME
    /// update replaced the stylesheet since.
    Gnome,
    /// Recoolor's theme with this palette (its id).
    Palette(String),
}

pub fn shows() -> Shows {
    desktop::shell_stylesheet("gdm.css")
        .ok()
        .and_then(|css| shell::copied_palette_id(&css).map(str::to_owned))
        .map_or(Shows::Gnome, Shows::Palette)
}

/// A palette's name for its id, or the id when no palette has it any more.
pub fn palette_name(id: &str) -> String {
    let (palettes, _) = palette::catalog(paths::user_palettes_dir().as_deref());
    palettes
        .into_iter()
        .find(|p| p.id == id)
        .map_or_else(|| id.to_owned(), |p| p.name)
}

pub fn app() -> Option<gio::AppInfo> {
    let id = format!("{GDM_SETTINGS}.desktop");
    gio::AppInfo::all()
        .into_iter()
        .find(|a| a.id().as_deref() == Some(id.as_str()))
}

/// Whether Recoolor's theme is where GDM Settings looks (Set Up… done).
pub fn linked() -> bool {
    ["/usr/local/share/themes", "/usr/share/themes"]
        .iter()
        .any(|dir| {
            Path::new(dir)
                .join(shell::THEME_NAME)
                .join("gnome-shell")
                .is_dir()
        })
}

/// Whether GDM Settings is open; then its settings file is not touched, since it
/// would overwrite it.
pub fn running() -> bool {
    std::process::Command::new("flatpak")
        .args(["ps", "--columns=application"])
        .output()
        .is_ok_and(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .any(|l| l.trim() == GDM_SETTINGS)
        })
}

fn keyfile(home: &Path) -> PathBuf {
    home.join(".var/app")
        .join(GDM_SETTINGS)
        .join("config/glib-2.0/settings/keyfile")
}

/// A system-wide theme of `kind` (`icons`), which GDM Settings can list.
fn system_theme(kind: &str, name: &str) -> bool {
    ["/usr/local/share", "/usr/share"]
        .iter()
        .any(|dir| Path::new(dir).join(kind).join(name).is_dir())
}

/// Fills in GDM Settings' appearance: Recoolor's theme, and the accent, icons,
/// cursors and background image when given (icons and cursors only if installed
/// system-wide). Only while it is closed.
pub fn prefill(
    home: &Path,
    accent: Option<&str>,
    icons: Option<&str>,
    cursors: Option<&str>,
    background: Option<&Path>,
) -> Result<(), String> {
    let path = keyfile(home);
    let file = glib::KeyFile::new();
    // A missing file is fine: GDM Settings has not saved anything yet.
    let _ = file.load_from_file(&path, glib::KeyFileFlags::KEEP_COMMENTS);
    let group = "io/github/realmazharhussain/GdmSettings/appearance";
    // Values are GVariant text: strings in quotes. Theme and accent names have none.
    let set = |key: &str, value: &str| file.set_string(group, key, &format!("'{value}'"));
    set("shell-theme", shell::THEME_NAME);
    if let Some(accent) = accent {
        set("accent-color", accent);
    }
    if let Some(icons) = icons.filter(|n| system_theme("icons", n)) {
        set("icon-theme", icons);
    }
    if let Some(cursors) = cursors.filter(|n| system_theme("icons", n)) {
        set("cursor-theme", cursors);
    }
    if let Some(background) = background {
        set("background-type", "image");
        // A path with a quote would end the GVariant string early.
        set(
            "background-image",
            &background.display().to_string().replace('\'', "\\'"),
        );
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    file.save_to_file(&path)
        .map_err(|e| format!("cannot fill in GDM Settings: {e}"))
}

pub fn open() -> Result<(), String> {
    app()
        .ok_or("GDM Settings is not installed")?
        .launch(&[], gio::AppLaunchContext::NONE)
        .map_err(|e| format!("cannot open GDM Settings: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefill_keeps_what_gdm_settings_saved() {
        let home = std::env::temp_dir().join(format!("recoolor-test-{}", std::process::id()));
        let path = keyfile(&home);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            "[io/github/realmazharhussain/GdmSettings]\ninitial-warning=false\n",
        )
        .unwrap();
        let background = Path::new("/home/me/.local/share/themes/Recoolor/login-background.jpg");
        prefill(
            &home,
            Some("pink"),
            Some("not-installed-anywhere"),
            None,
            Some(background),
        )
        .unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("initial-warning=false"));
        assert!(text.contains("[io/github/realmazharhussain/GdmSettings/appearance]"));
        assert!(text.contains("shell-theme='Recoolor'"));
        assert!(text.contains("accent-color='pink'"));
        assert!(!text.contains("icon-theme"));
        assert!(text.contains("background-type='image'"));
        assert!(text.contains(
            "background-image='/home/me/.local/share/themes/Recoolor/login-background.jpg'"
        ));
        std::fs::remove_dir_all(&home).unwrap();
    }
}
