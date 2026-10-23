//! Installed icon and cursor themes.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Where icon and cursor themes live, most specific first: `~/.local/share/icons`,
/// `~/.icons`, then `icons` under each of `$XDG_DATA_DIRS`.
pub fn search_dirs(data_home: Option<&Path>) -> Vec<PathBuf> {
    let data_dirs = std::env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".to_owned());
    data_home
        .map(|d| d.join("icons"))
        .into_iter()
        .chain(std::env::home_dir().map(|h| h.join(".icons")))
        .chain(
            data_dirs
                .split(':')
                .filter(|d| Path::new(d).is_absolute())
                .map(|d| Path::new(d).join("icons")),
        )
        .collect()
}

/// Folders with an `index.theme` that lists icon directories. `hicolor`, the
/// fallback every theme inherits, is not a choice.
pub fn icon_themes(dirs: &[PathBuf]) -> Vec<String> {
    themes(dirs, |theme| {
        fs::read_to_string(theme.join("index.theme"))
            .is_ok_and(|text| text.lines().any(|l| l.starts_with("Directories=")))
    })
    .into_iter()
    .filter(|name| name != "hicolor")
    .collect()
}

/// Folders with a `cursors` subfolder.
pub fn cursor_themes(dirs: &[PathBuf]) -> Vec<String> {
    themes(dirs, |theme| theme.join("cursors").is_dir())
        .into_iter()
        // `default` is an alias folder, not a theme.
        .filter(|name| name != "default")
        .collect()
}

fn themes(dirs: &[PathBuf], is_theme: impl Fn(&Path) -> bool) -> BTreeSet<String> {
    dirs.iter()
        .filter_map(|dir| fs::read_dir(dir).ok())
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| is_theme(&entry.path()))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tells_icon_and_cursor_themes_apart() {
        let root = tempfile::tempdir().unwrap();
        let (a, b) = (root.path().join("a"), root.path().join("b"));
        let theme = |dir: &Path, name: &str, index: &str, cursors: bool| {
            let t = dir.join(name);
            fs::create_dir_all(&t).unwrap();
            fs::write(t.join("index.theme"), index).unwrap();
            if cursors {
                fs::create_dir(t.join("cursors")).unwrap();
            }
        };
        let icons = "[Icon Theme]\nName=X\nDirectories=16x16/apps\n";
        theme(&a, "Papirus", icons, false);
        theme(&b, "Papirus", icons, false);
        theme(&a, "Bibata", "[Icon Theme]\nName=Bibata\n", true);
        theme(&b, "Adwaita", icons, true);
        theme(&b, "hicolor", icons, false);
        theme(&b, "default", "[Icon Theme]\nInherits=Adwaita\n", true);

        let dirs = [a, b, root.path().join("missing")];
        assert_eq!(icon_themes(&dirs), ["Adwaita", "Papirus"]);
        assert_eq!(cursor_themes(&dirs), ["Adwaita", "Bibata"]);
    }
}
