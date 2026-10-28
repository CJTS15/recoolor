//! Favorite palettes, by id, one per line in `$XDG_CONFIG_HOME/recoolor/favorites`. A
//! preference, not something Recoolor changed, so Reset leaves it alone.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::apply::{ApplyError, write_atomic};
use crate::paths;

pub fn path() -> Option<PathBuf> {
    paths::config_home().map(|d| d.join("recoolor/favorites"))
}

/// The favorites in `path`; none when there is no file yet.
pub fn load(path: &Path) -> BTreeSet<String> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
        .collect()
}

pub fn save(path: &Path, ids: &BTreeSet<String>) -> Result<(), ApplyError> {
    let text: String = ids.iter().map(|id| format!("{id}\n")).collect();
    write_atomic(path, &text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("recoolor/favorites");
        assert!(load(&path).is_empty());
        let ids: BTreeSet<String> = ["nord", "catppuccin-mocha"].map(str::to_owned).into();
        save(&path, &ids).unwrap();
        assert_eq!(load(&path), ids);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "catppuccin-mocha\nnord\n"
        );
    }
}
