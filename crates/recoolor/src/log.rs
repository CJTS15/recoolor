//! A record of what Recoolor did and what went wrong, appended to
//! `$XDG_STATE_HOME/recoolor/recoolor.log` (normally `~/.local/state/recoolor/recoolor.log`).

use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
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
    // Logging must never break the app, so a failed write is dropped.
    let _ = fs::create_dir_all(path.parent().expect("log path has a parent")).and_then(|()| {
        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .open(&path)?;
        // The window, the watcher and the command line share the log: the lock keeps
        // one from appending while another cuts it short.
        file.lock()?;
        // Keep the last 256 KiB once the log passes 1 MiB.
        if file.metadata()?.len() > 1 << 20 {
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)?;
            let tail = &bytes[bytes.len() - (256 << 10)..];
            // From the start of a line, which is also a character boundary.
            let tail = tail
                .iter()
                .position(|&b| b == b'\n')
                .map_or(tail, |i| &tail[i + 1..]);
            let tail = tail.to_vec();
            file.set_len(0)?;
            file.write_all(&tail)?;
        }
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
