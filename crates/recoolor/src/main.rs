//! `recoolor` opens the window; `recoolor <command>` runs the command line.

use std::process::ExitCode;

mod cli;
mod desktop;
mod log;
mod login_screen;
mod restart;
mod targets;
mod ui;
mod wallpaper;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        return ui::run();
    }
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match cli::run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            log::error(&format!("recoolor {}: {message}", args.join(" ")));
            eprintln!("recoolor: {message}");
            ExitCode::FAILURE
        }
    }
}
