//! Restarting apps that only read their theme at startup (Firefox, Thunderbird,
//! Chrome), so a new palette shows without the user doing it by hand. Each is
//! asked to quit the way its own Quit does (SIGTERM, which they handle by saving
//! their session), and opened again from its desktop entry.

use std::fs;
use std::process::Command;
use std::time::{Duration, Instant};

use gtk::gio;
use gtk::gio::prelude::*;

pub struct App {
    pub name: &'static str,
    /// The main process's name in `/proc/<pid>/comm`.
    comm: &'static str,
    /// Desktop entries to reopen it with, the first one installed wins.
    desktop: &'static [&'static str],
}

pub const FIREFOX: App = App {
    name: "Firefox",
    comm: "firefox",
    desktop: &["org.mozilla.firefox.desktop", "firefox.desktop"],
};
pub const THUNDERBIRD: App = App {
    name: "Thunderbird",
    comm: "thunderbird",
    desktop: &[
        "net.thunderbird.Thunderbird.desktop",
        "org.mozilla.Thunderbird.desktop",
        "thunderbird.desktop",
    ],
};
pub const CHROME: App = App {
    name: "Chrome",
    comm: "chrome",
    desktop: &["google-chrome.desktop", "com.google.Chrome.desktop"],
};

fn comm(pid: &str) -> Option<String> {
    fs::read_to_string(format!("/proc/{pid}/comm"))
        .ok()
        .map(|c| c.trim().to_owned())
}

/// The app's main processes: those whose parent is not the app itself (its
/// helpers are its children). One per running instance.
fn main_pids(app: &App) -> Vec<u32> {
    let Ok(entries) = fs::read_dir("/proc") else {
        return Vec::new();
    };
    entries
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|pid| pid.bytes().all(|b| b.is_ascii_digit()))
        .filter(|pid| comm(pid).as_deref() == Some(app.comm))
        .filter(|pid| {
            let stat = fs::read_to_string(format!("/proc/{pid}/stat")).unwrap_or_default();
            // The parent pid is the second field after the parenthesised name.
            let parent = stat
                .rsplit_once(')')
                .and_then(|(_, rest)| rest.split_whitespace().nth(1))
                .unwrap_or("");
            comm(parent).as_deref() != Some(app.comm)
        })
        .filter_map(|pid| pid.parse().ok())
        .collect()
}

pub fn is_running(app: &App) -> bool {
    !main_pids(app).is_empty()
}

/// Quits every instance of `app`, waits up to 15 s for them to exit, and opens
/// it again once. Blocking; run it off the main thread.
pub fn restart(app: &App) -> Result<(), String> {
    let pids = main_pids(app);
    if pids.is_empty() {
        return Ok(());
    }
    for pid in &pids {
        Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status()
            .map_err(|e| format!("cannot quit {}: {e}", app.name))?;
    }
    let deadline = Instant::now() + Duration::from_secs(15);
    while !main_pids(app).is_empty() {
        if Instant::now() > deadline {
            return Err(format!("{} did not quit; restart it by hand", app.name));
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    let entry = gio::AppInfo::all()
        .into_iter()
        .find(|a| a.id().is_some_and(|id| app.desktop.contains(&id.as_str())))
        .ok_or_else(|| format!("{} quit, but its launcher was not found", app.name))?;
    entry
        .launch(&[], gio::AppLaunchContext::NONE)
        .map_err(|e| format!("cannot reopen {}: {e}", app.name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_this_test_as_a_main_process() {
        // The test binary's own comm, truncated to 15 bytes like the kernel does.
        let me = comm(&std::process::id().to_string()).unwrap();
        let app = App {
            name: "test",
            comm: Box::leak(me.into_boxed_str()),
            desktop: &[],
        };
        assert!(main_pids(&app).contains(&std::process::id()));
    }
}
