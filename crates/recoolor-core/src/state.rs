//! What Recoolor changed outside its own files, so restore can put it back:
//! `$XDG_STATE_HOME/recoolor/state.toml`.
//!
//! Each `*_previous` is the value from before Recoolor first changed that setting.
//! It is recorded once and kept across later applies, so restore always returns
//! to the user's own choice rather than to an earlier Recoolor one.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::apply::{ApplyError, io_err, write_atomic};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    /// Flatpak override entries Recoolor added.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub flatpak_added: Vec<String>,
    /// The User Themes extension's theme name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell_previous: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icons_previous: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursors_previous: Option<String>,
    /// The Ptyxis default profile's palette.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ptyxis_previous: Option<String>,
    /// VS Code's `workbench.colorTheme`, as raw JSON string contents; empty when
    /// the key was not in `settings.json`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vscode_previous: Option<String>,
    /// GNOME's `color-scheme`, when Recoolor turned `default` into `prefer-light`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_scheme_previous: Option<String>,
    /// The desktop background before Recoolor set one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background_previous: Option<Background>,
    /// The image the user picked; Recoolor's wallpapers always start from it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallpaper_source: Option<String>,
}

/// The `org.gnome.desktop.background` keys Recoolor changes.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Background {
    pub picture_uri: String,
    pub picture_uri_dark: String,
    pub picture_options: String,
}

/// An exclusive lock on `recoolor/state.lock`, held until the file is dropped,
/// so two processes never read and write `state.toml` over each other.
pub fn lock(state_home: &Path) -> Result<fs::File, ApplyError> {
    let path = state_home.join("recoolor/state.lock");
    fs::create_dir_all(path.parent().expect("has a parent")).map_err(io_err(&path))?;
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .map_err(io_err(&path))?;
    file.lock().map_err(io_err(&path))?;
    Ok(file)
}

pub fn path(state_home: &Path) -> PathBuf {
    state_home.join("recoolor/state.toml")
}

impl State {
    /// A missing file is an empty state.
    pub fn load(path: &Path) -> Result<Self, ApplyError> {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(io_err(path)(e)),
        };
        toml::from_str(&text).map_err(|e| ApplyError::Invalid {
            path: path.to_owned(),
            message: e.to_string(),
        })
    }

    /// Writes the state, or removes the file once there is nothing left to restore.
    pub fn save(&self, path: &Path) -> Result<(), ApplyError> {
        if *self == Self::default() {
            return match fs::remove_file(path) {
                Err(e) if e.kind() != io::ErrorKind::NotFound => Err(io_err(path)(e)),
                _ => Ok(()),
            };
        }
        let text = toml::to_string(self).expect("state serializes");
        write_atomic(
            path,
            &format!("# Written by Recoolor; `recoolor restore` reads it.\n{text}"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_removes_when_empty() {
        let home = tempfile::tempdir().unwrap();
        let path = path(home.path());
        assert_eq!(State::load(&path).unwrap(), State::default());

        let state = State {
            flatpak_added: vec!["xdg-config/gtk-4.0:ro".into()],
            shell_previous: Some(String::new()),
            icons_previous: Some("Adwaita".into()),
            cursors_previous: None,
            ptyxis_previous: Some("Tokyo Night".into()),
            vscode_previous: Some(String::new()),
            color_scheme_previous: Some("default".into()),
            background_previous: Some(Background {
                picture_uri: "file:///usr/share/backgrounds/a.jxl".into(),
                picture_uri_dark: "file:///usr/share/backgrounds/a-d.jxl".into(),
                picture_options: "zoom".into(),
            }),
            wallpaper_source: Some("/home/me/Pictures/tree.png".into()),
        };
        state.save(&path).unwrap();
        assert_eq!(State::load(&path).unwrap(), state);

        State::default().save(&path).unwrap();
        assert!(!path.exists());
        State::default().save(&path).unwrap();
    }

    #[test]
    fn lock_is_exclusive_until_dropped() {
        let home = tempfile::tempdir().unwrap();
        let held = lock(home.path()).unwrap();
        let other = fs::File::open(home.path().join("recoolor/state.lock")).unwrap();
        assert!(other.try_lock().is_err());
        drop(held);
        assert!(other.try_lock().is_ok());
    }

    #[test]
    fn corrupt_file_is_an_error_not_an_empty_state() {
        let home = tempfile::tempdir().unwrap();
        let path = path(home.path());
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "icons_previous = [").unwrap();
        assert!(matches!(
            State::load(&path),
            Err(ApplyError::Invalid { .. })
        ));
    }
}
