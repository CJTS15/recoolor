//! Writing generated CSS into the user's GTK config, and undoing it.
//!
//! The first apply moves an existing `gtk.css` that Recoolor did not write to
//! `gtk.css.recoolor-backup` in the same folder, so the move is a rename that also
//! preserves symlinks. Later applies overwrite only Recoolor's own file. Restore removes
//! Recoolor's file and moves the backup back. A `gtk.css` that is not Recoolor's is never
//! overwritten or deleted.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::css;
use crate::palette::Palette;

const CSS_FILE: &str = "gtk.css";
const BACKUP_FILE: &str = "gtk.css.recoolor-backup";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Toolkit {
    Gtk3,
    Gtk4,
}

impl Toolkit {
    pub const ALL: [Toolkit; 2] = [Toolkit::Gtk3, Toolkit::Gtk4];

    pub fn dir_name(self) -> &'static str {
        match self {
            Toolkit::Gtk3 => "gtk-3.0",
            Toolkit::Gtk4 => "gtk-4.0",
        }
    }

    pub fn render(self, palette: &Palette) -> String {
        match self {
            Toolkit::Gtk3 => css::gtk3(palette),
            Toolkit::Gtk4 => css::gtk4(palette),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ApplyError {
    #[error("{}: {source}", path.display())]
    Io { path: PathBuf, source: io::Error },
    #[error(
        "{} was not written by Recoolor, and an older backup already exists at {}. \
         Move one of them aside and try again.",
        css.display(),
        backup.display()
    )]
    BackupExists { css: PathBuf, backup: PathBuf },
    #[error("{} was not written by Recoolor, so Recoolor leaves it alone.", path.display())]
    Foreign { path: PathBuf },
    #[error("{}: {message}", path.display())]
    Invalid { path: PathBuf, message: String },
}

/// What a toolkit's `gtk.css` currently is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CssState {
    Missing,
    /// Written by Recoolor with this palette.
    Recoolor(String),
    /// Written by Recoolor with a light/dark pair that follows the system style.
    RecoolorPair {
        light: String,
        dark: String,
    },
    /// Anything else: the user's own CSS or another theme's.
    Foreign,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Status {
    pub toolkit: Toolkit,
    pub css: CssState,
    pub has_backup: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RestoreOutcome {
    /// Recoolor's file was replaced by the backup.
    Restored,
    /// Recoolor's file was removed; there was nothing to restore.
    Removed,
    /// The current `gtk.css` is not Recoolor's, so it and any backup were left alone.
    LeftForeign,
    NothingToDo,
}

/// The GTK config folders under one config home, normally `~/.config`.
#[derive(Clone, Debug)]
pub struct GtkConfig {
    config_home: PathBuf,
    adw_gtk3: bool,
}

impl GtkConfig {
    pub fn new(config_home: impl Into<PathBuf>) -> Self {
        Self {
            config_home: config_home.into(),
            adw_gtk3: false,
        }
    }

    /// Writes GTK3's file for the adw-gtk3 theme (named colors only).
    pub fn with_adw_gtk3(mut self, on: bool) -> Self {
        self.adw_gtk3 = on;
        self
    }

    fn gtk3(&self, palette: &Palette) -> String {
        if self.adw_gtk3 {
            css::gtk3_adw(palette)
        } else {
            css::gtk3(palette)
        }
    }

    pub fn css_path(&self, toolkit: Toolkit) -> PathBuf {
        self.config_home.join(toolkit.dir_name()).join(CSS_FILE)
    }

    pub fn backup_path(&self, toolkit: Toolkit) -> PathBuf {
        self.config_home.join(toolkit.dir_name()).join(BACKUP_FILE)
    }

    pub fn status(&self) -> Result<Vec<Status>, ApplyError> {
        Toolkit::ALL
            .into_iter()
            .map(|toolkit| {
                Ok(Status {
                    toolkit,
                    css: css_state(&self.css_path(toolkit))?,
                    has_backup: exists(&self.backup_path(toolkit))?,
                })
            })
            .collect()
    }

    /// Writes `palette` for both toolkits.
    pub fn apply(&self, palette: &Palette) -> Result<(), ApplyError> {
        self.write([
            (Toolkit::Gtk3, self.gtk3(palette)),
            (Toolkit::Gtk4, css::gtk4(palette)),
        ])
    }

    /// Writes a pair that follows GNOME's light/dark style. GTK4 switches by itself;
    /// GTK3 has no media query, so it gets the palette for `dark_now` and has to be
    /// rewritten on each switch (`recoolor watch`).
    pub fn apply_pair(
        &self,
        light: &Palette,
        dark: &Palette,
        dark_now: bool,
    ) -> Result<(), ApplyError> {
        let gtk3 = self.gtk3(if dark_now { dark } else { light });
        self.write([
            (Toolkit::Gtk3, gtk3),
            (Toolkit::Gtk4, css::gtk4_pair(light, dark)),
        ])
    }

    /// Checks every toolkit before changing any, so a backup conflict leaves
    /// everything untouched.
    fn write(&self, files: [(Toolkit, String); 2]) -> Result<(), ApplyError> {
        let mut to_back_up = Vec::new();
        for toolkit in Toolkit::ALL {
            let (css, backup) = (self.css_path(toolkit), self.backup_path(toolkit));
            if css_state(&css)? == CssState::Foreign {
                if exists(&backup)? {
                    return Err(ApplyError::BackupExists { css, backup });
                }
                to_back_up.push((css, backup));
            }
        }

        for (css, backup) in to_back_up {
            fs::rename(&css, &backup).map_err(io_err(&css))?;
        }
        for (toolkit, contents) in files {
            write_atomic(&self.css_path(toolkit), &contents)?;
        }
        Ok(())
    }

    /// Undoes [`apply`](Self::apply) for both toolkits.
    pub fn restore(&self) -> Result<Vec<(Toolkit, RestoreOutcome)>, ApplyError> {
        Toolkit::ALL
            .into_iter()
            .map(|toolkit| Ok((toolkit, self.restore_one(toolkit)?)))
            .collect()
    }

    fn restore_one(&self, toolkit: Toolkit) -> Result<RestoreOutcome, ApplyError> {
        let (css, backup) = (self.css_path(toolkit), self.backup_path(toolkit));
        let had_recoolor = match css_state(&css)? {
            CssState::Foreign => return Ok(RestoreOutcome::LeftForeign),
            CssState::Recoolor(_) | CssState::RecoolorPair { .. } => true,
            CssState::Missing => false,
        };
        let has_backup = exists(&backup)?;

        if has_backup {
            // rename() replaces Recoolor's file atomically, so there is no moment without one.
            fs::rename(&backup, &css).map_err(io_err(&backup))?;
            Ok(RestoreOutcome::Restored)
        } else if had_recoolor {
            fs::remove_file(&css).map_err(io_err(&css))?;
            Ok(RestoreOutcome::Removed)
        } else {
            Ok(RestoreOutcome::NothingToDo)
        }
    }
}

pub(crate) fn css_state(path: &Path) -> Result<CssState, ApplyError> {
    if !exists(path)? {
        return Ok(CssState::Missing);
    }
    // A dangling symlink or unreadable file is still something we must not clobber.
    let Ok(bytes) = fs::read(path) else {
        return Ok(CssState::Foreign);
    };
    let text = String::from_utf8_lossy(&bytes);
    Ok(match css::applied_palette_id(&text) {
        Some(id) => match id.split_once(css::PAIR_SEPARATOR) {
            Some((light, dark)) => CssState::RecoolorPair {
                light: light.to_owned(),
                dark: dark.to_owned(),
            },
            None => CssState::Recoolor(id.to_owned()),
        },
        None => CssState::Foreign,
    })
}

/// Whether anything (including a dangling symlink) is at `path`.
pub(crate) fn exists(path: &Path) -> Result<bool, ApplyError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(io_err(path)(e)),
    }
}

/// Writes through a temp file and a rename, so apps starting mid-write never read
/// half a stylesheet.
pub(crate) fn write_atomic(path: &Path, contents: &str) -> Result<(), ApplyError> {
    let dir = path.parent().expect("path has a parent");
    fs::create_dir_all(dir).map_err(io_err(dir))?;
    let name = path
        .file_name()
        .expect("path has a file name")
        .to_string_lossy();
    let tmp = dir.join(format!(".{name}.recoolor-tmp"));
    let result = (|| {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(contents.as_bytes())?;
        file.sync_all()?;
        fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result.map_err(io_err(path))
}

/// Reads a text file; a missing one is empty.
pub(crate) fn read_or_empty(path: &Path) -> Result<String, ApplyError> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(text),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(io_err(path)(e)),
    }
}

pub(crate) fn io_err(path: &Path) -> impl FnOnce(io::Error) -> ApplyError + '_ {
    move |source| ApplyError::Io {
        path: path.to_owned(),
        source,
    }
}
