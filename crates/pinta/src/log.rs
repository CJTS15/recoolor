//! A record of what Pinta did and what went wrong, appended to
//! `$XDG_STATE_HOME/pinta/pinta.log` (normally `~/.local/state/pinta/pinta.log`).

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use gtk::glib;
use pinta_core::paths;

pub fn path() -> Option<PathBuf> {
    paths::state_home().map(|d| d.join("pinta/pinta.log"))
}

pub fn info(message: &str) {
    append("INFO", message);
}

pub fn error(message: &str) {
    append("ERROR", message);
}

// ponytail: grows without rotation; fine for personal testing, trim or rotate if it gets big.
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
