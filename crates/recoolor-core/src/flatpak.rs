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

/// Makes Qt apps on the KDE runtime use KDE's platform theme, which reads
/// `~/.config/kdeglobals` (Recoolor's palette); under GNOME they would pick the GTK or
/// portal one, which ignores it.
pub const QT_THEME: (&str, &str) = ("QT_QPA_PLATFORMTHEME", "kde");

const ENV_GROUP: &str = "[Environment]";

/// Sets `key=value` for every Flatpak app, unless the user set `key` already. Returns
/// whether it added it.
pub fn set_env(path: &Path, (key, value): (&str, &str)) -> Result<bool, ApplyError> {
    let mut lines = split(&read(path)?);
    let prefix = format!("{key}=");
    let (start, end) = match group(&lines, ENV_GROUP) {
        Some(span) => span,
        None => {
            if lines.last().is_some_and(|l| !l.trim().is_empty()) {
                lines.push(String::new());
            }
            lines.push(ENV_GROUP.to_owned());
            (lines.len() - 1, lines.len())
        }
    };
    if (start + 1..end).any(|i| lines[i].starts_with(&prefix)) {
        return Ok(false);
    }
    lines.insert(start + 1, format!("{prefix}{value}"));
    write_atomic(path, &join(&lines))?;
    Ok(true)
}

/// Takes `key=value` out again, only while it still has Recoolor's value, and the
/// `[Environment]` group with it when that leaves it empty.
pub fn unset_env(path: &Path, (key, value): (&str, &str)) -> Result<(), ApplyError> {
    let text = read(path)?;
    let mut lines = split(&text);
    let Some((start, end)) = group(&lines, ENV_GROUP) else {
        return Ok(());
    };
    let ours = format!("{key}={value}");
    let Some(i) = (start + 1..end).find(|&i| lines[i].trim() == ours) else {
        return Ok(());
    };
    lines.remove(i);
    if (start + 1..end - 1).all(|i| lines[i].trim().is_empty()) {
        lines.drain(start..end - 1);
        while lines.last().is_some_and(|l| l.trim().is_empty()) {
            lines.pop();
        }
    }
    let new = if lines.is_empty() {
        String::new()
    } else {
        join(&lines)
    };
    write_atomic(path, &new)
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
    group(lines, GROUP)
}

/// A group (`[Context]`, `[Environment]`) as (header line index, end exclusive).
fn group(lines: &[String], name: &str) -> Option<(usize, usize)> {
    let start = lines.iter().position(|l| l.trim() == name)?;
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
    fn qt_theme_comes_and_goes_and_leaves_the_users_own() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("global");
        let theirs = "[Context]\nfilesystems=xdg-config/gtk-3.0:ro;\n";
        std::fs::write(&path, theirs).unwrap();
        assert!(set_env(&path, QT_THEME).unwrap());
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            format!("{theirs}\n[Environment]\nQT_QPA_PLATFORMTHEME=kde\n")
        );
        // Already there: nothing more.
        assert!(!set_env(&path, QT_THEME).unwrap());
        unset_env(&path, QT_THEME).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), theirs);

        // The user's own value is never replaced, nor taken out.
        std::fs::write(&path, "[Environment]\nQT_QPA_PLATFORMTHEME=qt6ct\n").unwrap();
        assert!(!set_env(&path, QT_THEME).unwrap());
        unset_env(&path, QT_THEME).unwrap();
        assert!(std::fs::read_to_string(&path).unwrap().contains("=qt6ct"));
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
