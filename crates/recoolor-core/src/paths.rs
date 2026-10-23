//! XDG base directories.

use std::env;
use std::path::PathBuf;

/// `$XDG_CONFIG_HOME`, or `~/.config`.
pub fn config_home() -> Option<PathBuf> {
    xdg_dir("XDG_CONFIG_HOME", ".config")
}

/// `$XDG_DATA_HOME`, or `~/.local/share`.
pub fn data_home() -> Option<PathBuf> {
    xdg_dir("XDG_DATA_HOME", ".local/share")
}

/// `$XDG_STATE_HOME`, or `~/.local/state`.
pub fn state_home() -> Option<PathBuf> {
    xdg_dir("XDG_STATE_HOME", ".local/state")
}

/// Where the user keeps their own palettes: `$XDG_DATA_HOME/recoolor/palettes`.
pub fn user_palettes_dir() -> Option<PathBuf> {
    data_home().map(|d| d.join("recoolor/palettes"))
}

fn xdg_dir(var: &str, fallback: &str) -> Option<PathBuf> {
    // The spec says relative values are invalid and must be ignored.
    env::var_os(var)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| {
            env::home_dir()
                .filter(|h| h.is_absolute())
                .map(|h| h.join(fallback))
        })
}
