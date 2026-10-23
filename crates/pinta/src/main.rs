//! Pinta command line. The GTK window arrives in a later step; until then this is
//! the way to try palettes.

use std::process::ExitCode;

use pinta_core::palette::{self, Palette};
use pinta_core::{CssState, GtkConfig, RestoreOutcome, Toolkit, paths};

const USAGE: &str = "\
Usage: pinta <command>

Commands:
  list               List available palettes
  show <id> [gtk3]   Print the generated GTK4 (or GTK3) CSS
  apply <id>         Apply a palette to GTK3 and GTK4 apps
  restore            Remove Pinta's CSS and bring back what was there before
  status             Show what is applied

Restart apps to see changes.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("pinta: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &[&str]) -> Result<(), String> {
    match args {
        ["list"] => {
            for p in palettes() {
                println!("{:<24} {:<22} {:?}", p.id, p.name, p.variant);
            }
            Ok(())
        }
        ["show", id] | ["show", id, "gtk4"] => {
            print!("{}", Toolkit::Gtk4.render(&find(id)?));
            Ok(())
        }
        ["show", id, "gtk3"] => {
            print!("{}", Toolkit::Gtk3.render(&find(id)?));
            Ok(())
        }
        ["apply", id] => {
            let palette = find(id)?;
            gtk_config()?.apply(&palette).map_err(|e| e.to_string())?;
            println!("Applied {}. Restart apps to see it.", palette.name);
            Ok(())
        }
        ["restore"] => {
            for (toolkit, outcome) in gtk_config()?.restore().map_err(|e| e.to_string())? {
                let what = match outcome {
                    RestoreOutcome::Restored => "restored your previous gtk.css",
                    RestoreOutcome::Removed => "removed Pinta's gtk.css",
                    RestoreOutcome::LeftForeign => "gtk.css is not Pinta's, left it alone",
                    RestoreOutcome::NothingToDo => "nothing to restore",
                };
                println!("{}: {what}", toolkit.dir_name());
            }
            Ok(())
        }
        ["status"] => {
            for s in gtk_config()?.status().map_err(|e| e.to_string())? {
                let css = match s.css {
                    CssState::Missing => "no gtk.css".to_owned(),
                    CssState::Pinta(id) => format!("Pinta palette {id}"),
                    CssState::Foreign => "gtk.css not written by Pinta".to_owned(),
                };
                let backup = if s.has_backup { " (backup kept)" } else { "" };
                println!("{}: {css}{backup}", s.toolkit.dir_name());
            }
            Ok(())
        }
        [] | ["help" | "-h" | "--help"] => {
            println!("{USAGE}");
            Ok(())
        }
        _ => Err(format!("unrecognised arguments\n\n{USAGE}")),
    }
}

fn palettes() -> Vec<Palette> {
    let (palettes, errors) = palette::catalog(paths::user_palettes_dir().as_deref());
    for e in errors {
        eprintln!("pinta: skipping palette {e}");
    }
    palettes
}

fn find(id: &str) -> Result<Palette, String> {
    palettes()
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| format!("no palette {id:?}; run `pinta list`"))
}

fn gtk_config() -> Result<GtkConfig, String> {
    paths::config_home()
        .map(GtkConfig::new)
        .ok_or_else(|| "cannot find your config folder; set HOME or XDG_CONFIG_HOME".to_owned())
}
