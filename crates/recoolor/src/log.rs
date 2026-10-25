//! A record of what Recoolor did and what went wrong, appended to
//! `$XDG_STATE_HOME/recoolor/recoolor.log` (normally `~/.local/state/recoolor/recoolor.log`).

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use gtk::glib;
use recoolor_core::paths;

pub fn path() -> Option<PathBuf> {
    paths::state_home().map(|d| d.join("recoolor/recoolor.log"))
}

pub fn info(message: &str) {
    append("INFO", message);
}

pub fn error(message: &str) {
    append("ERROR", message);
}

fn append(level: &str, message: &str) {
    let Some(path) = path() else {
        return;
    };
    let time = glib::DateTime::now_local()
        .and_then(|t| t.format("%F %T"))
        .map(|t| t.to_string())
        .unwrap_or_default();
    // Indent continuation lines so each entry starts with a timestamp.
    let message = message.replace('\n', "\n    ");
    // Keep the last 256 KiB once the log passes 1 MiB.
    if fs::metadata(&path).is_ok_and(|m| m.len() > 1 << 20)
        && let Ok(text) = fs::read_to_string(&path)
    {
        let cut = text.len() - (256 << 10);
        let cut = (cut..text.len())
            .find(|&i| text.is_char_boundary(i))
            .unwrap_or(cut);
        let tail = &text[cut..];
        let tail = tail.split_once('\n').map_or(tail, |(_, rest)| rest);
        let _ = fs::write(&path, tail);
    }
    // Logging must never break the app, so a failed write is dropped.
    let _ = fs::create_dir_all(path.parent().expect("log path has a parent")).and_then(|()| {
        let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
        writeln!(file, "{time} {level} {message}")
    });
}

/// Also records GLib and GTK warnings, which otherwise only reach the terminal.
/// They are still printed as usual.
pub fn capture_glib_warnings() {
    glib::log_set_writer_func(|level, fields| {
        if matches!(
            level,
            glib::LogLevel::Error | glib::LogLevel::Critical | glib::LogLevel::Warning
        ) {
            let field = |key: &str| {
                fields
                    .iter()
                    .find(|f| f.key() == key)
                    .and_then(|f| f.value_str())
                    .unwrap_or("")
            };
            error(&format!("{}: {}", field("GLIB_DOMAIN"), field("MESSAGE")));
        }
        glib::log_writer_default(level, fields)
    });
}
