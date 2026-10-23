//! Letting Flatpak apps read Recoolor's stylesheets.
//!
//! Sandboxed apps cannot see `~/.config/gtk-{3,4}.0` unless granted. Recoolor adds
//! read-only grants to the user's global override file, the same file
//! `flatpak override --user` writes, and records exactly which entries it added
//! so restore can take out those and nothing else. (`--nofilesystem` would add a
//! deny instead of removing a grant, and `--reset` would wipe the user's own.)

use std::path::{Path, PathBuf};

use crate::apply::{ApplyError, read_or_empty, write_atomic};

/// The grants Recoolor wants.
pub const ENTRIES: [&str; 2] = ["xdg-config/gtk-3.0:ro", "xdg-config/gtk-4.0:ro"];

const GROUP: &str = "[Context]";
const KEY: &str = "filesystems=";

/// `$XDG_DATA_HOME/flatpak/overrides/global`.
pub fn overrides_path(data_home: &Path) -> PathBuf {
    data_home.join("flatpak/overrides/global")
}

/// Adds the [`ENTRIES`] not already covered and returns the ones it added. A path the
/// user already grants, or explicitly denies (`!path`), is left as it is.
pub fn grant(path: &Path) -> Result<Vec<String>, ApplyError> {
    let text = read(path)?;
    let (text, added) = add_entries(&text, &ENTRIES);
    if !added.is_empty() {
        write_atomic(path, &text)?;
    }
    Ok(added)
}

/// Removes exactly `entries` (what [`grant`] returned earlier).
pub fn revoke(path: &Path, entries: &[String]) -> Result<(), ApplyError> {
    if entries.is_empty() {
        return Ok(());
    }
    let text = read(path)?;
    let new = remove_entries(&text, entries);
    if new != text {
        write_atomic(path, &new)?;
    }
    Ok(())
}

/// Overrides an administrator set for every user.
pub const SYSTEM_OVERRIDES: &str = "/var/lib/flatpak/overrides/global";

/// Whether Flatpak apps can already read both folders, granted in any of `files`
/// (normally the user's and [`SYSTEM_OVERRIDES`]) and denied in none.
pub fn covered(files: &[&Path]) -> bool {
    let all: Vec<String> = files
        .iter()
        .flat_map(|f| filesystems(&read(f).unwrap_or_default()))
        .collect();
    ENTRIES.iter().all(|e| {
        let matching = || all.iter().filter(|c| bare(c) == bare(e));
        matching().any(|c| !c.starts_with('!')) && !matching().any(|c| c.starts_with('!'))
    })
}

fn filesystems(text: &str) -> Vec<String> {
    let lines = split(text);
    context(&lines)
        .and_then(|(start, end)| (start + 1..end).find(|&i| lines[i].starts_with(KEY)))
        .map(|i| values(&lines[i]))
        .unwrap_or_default()
}

fn read(path: &Path) -> Result<String, ApplyError> {
    read_or_empty(path)
}

/// `xdg-config/gtk-4.0` from `!xdg-config/gtk-4.0:ro`.
fn bare(entry: &str) -> &str {
    let entry = entry.trim_start_matches('!');
    entry.rsplit_once(':').map_or(entry, |(path, _)| path)
}

/// The `[Context]` group as (header line index, end line index exclusive).
fn context(lines: &[String]) -> Option<(usize, usize)> {
    let start = lines.iter().position(|l| l.trim() == GROUP)?;
    let end = lines[start + 1..]
        .iter()
        .position(|l| l.trim_start().starts_with('['))
        .map_or(lines.len(), |n| start + 1 + n);
    Some((start, end))
}

fn split(text: &str) -> Vec<String> {
    text.lines().map(str::to_owned).collect()
}

fn join(lines: &[String]) -> String {
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

fn values(line: &str) -> Vec<String> {
    line[KEY.len()..]
        .split(';')
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
        .collect()
}

fn line_for(values: &[String]) -> String {
    format!("{KEY}{};", values.join(";"))
}

fn add_entries(text: &str, entries: &[&str]) -> (String, Vec<String>) {
    let mut lines = split(text);
    let (start, end) = context(&lines).unwrap_or_else(|| {
        if lines.last().is_some_and(|l| !l.trim().is_empty()) {
            lines.push(String::new());
        }
        lines.push(GROUP.to_owned());
        (lines.len() - 1, lines.len())
    });
    let key_line = (start + 1..end).find(|&i| lines[i].starts_with(KEY));
    let mut current = key_line.map(|i| values(&lines[i])).unwrap_or_default();

    let added: Vec<String> = entries
        .iter()
        .filter(|e| !current.iter().any(|c| bare(c) == bare(e)))
        .map(|e| (*e).to_owned())
        .collect();
    if added.is_empty() {
        return (text.to_owned(), added);
    }
    current.extend(added.iter().cloned());
    match key_line {
        Some(i) => lines[i] = line_for(&current),
        None => lines.insert(start + 1, line_for(&current)),
    }
    (join(&lines), added)
}

fn remove_entries(text: &str, entries: &[String]) -> String {
    let mut lines = split(text);
    let Some((start, end)) = context(&lines) else {
        return text.to_owned();
    };
    let Some(i) = (start + 1..end).find(|&i| lines[i].starts_with(KEY)) else {
        return text.to_owned();
    };
    let kept: Vec<String> = values(&lines[i])
        .into_iter()
        .filter(|v| !entries.contains(v))
        .collect();
    if kept.is_empty() {
        lines.remove(i);
    } else {
        lines[i] = line_for(&kept);
    }
    join(&lines)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn grants_into_empty_file_and_revokes_cleanly() {
        let (text, added) = add_entries("", &ENTRIES);
        assert_eq!(
            text,
            "[Context]\nfilesystems=xdg-config/gtk-3.0:ro;xdg-config/gtk-4.0:ro;\n"
        );
        assert_eq!(added, ENTRIES);
        assert_eq!(remove_entries(&text, &added), "[Context]\n");
    }

    #[test]
    fn keeps_user_entries_other_keys_and_groups() {
        let user = "[Context]\n\
                    shared=network;\n\
                    filesystems=~/Music:ro;!xdg-config/gtk-3.0;\n\
                    \n\
                    [Environment]\n\
                    GTK_THEME=Adwaita\n";
        let (text, added) = add_entries(user, &ENTRIES);
        // The user's explicit deny for gtk-3.0 wins; only gtk-4.0 is added.
        assert_eq!(added, ["xdg-config/gtk-4.0:ro"]);
        assert!(
            text.contains("filesystems=~/Music:ro;!xdg-config/gtk-3.0;xdg-config/gtk-4.0:ro;\n")
        );
        assert!(text.contains("[Environment]\nGTK_THEME=Adwaita\n"));
        assert_eq!(remove_entries(&text, &added), user);
    }

    #[test]
    fn existing_grant_is_not_duplicated_or_removed() {
        let user = "[Context]\nfilesystems=xdg-config/gtk-4.0;\n";
        let (text, added) = add_entries(user, &ENTRIES);
        assert_eq!(added, ["xdg-config/gtk-3.0:ro"]);
        assert_eq!(remove_entries(&text, &added), user);
    }

    #[test]
    fn file_round_trip() {
        let home = tempfile::tempdir().unwrap();
        let path = overrides_path(home.path());
        let system = home.path().join("system-global");
        assert!(!covered(&[&path, &system]));
        let added = grant(&path).unwrap();
        assert!(covered(&[&path, &system]));
        assert_eq!(grant(&path).unwrap(), Vec::<String>::new());
        revoke(&path, &added).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "[Context]\n");
    }

    #[test]
    fn system_grant_counts_but_a_user_deny_wins() {
        let home = tempfile::tempdir().unwrap();
        let (user, system) = (home.path().join("user"), home.path().join("system"));
        fs::write(
            &system,
            "[Context]\nfilesystems=xdg-config/gtk-3.0;xdg-config/gtk-4.0;xdg-data/themes;\n",
        )
        .unwrap();
        assert!(covered(&[&user, &system]));
        fs::write(&user, "[Context]\nfilesystems=!xdg-config/gtk-4.0;\n").unwrap();
        assert!(!covered(&[&user, &system]));
    }
}
