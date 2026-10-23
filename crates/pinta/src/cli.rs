//! Pinta command line.

use pinta_core::palette::{self, Palette, Variant};
use pinta_core::{CssState, GtkConfig, RestoreOutcome, Toolkit, paths};

use crate::desktop;

const USAGE: &str = "\
Usage: pinta <command>

Commands:
  list               List available palettes
  show <id> [gtk3]   Print the generated GTK4 (or GTK3) CSS
  apply <id>         Apply a palette to GTK3 and GTK4 apps
  apply <light> <dark>
                     Apply a pair that follows GNOME's light/dark style
  restore            Remove Pinta's CSS and bring back what was there before
  status             Show what is applied
  watch              Keep GTK3 apps in step with the light/dark style
                     (started for you as a user service by `apply <light> <dark>`)

Run `pinta` with no command to open the window. Restart apps to see changes.";

pub fn run(args: &[&str]) -> Result<(), String> {
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
            desktop::set_watch(&desktop::config_home()?, false)?;
            println!("Applied {}. Restart apps to see it.", palette.name);
            Ok(())
        }
        ["apply", light, dark] => {
            let (light, dark) = (find(light)?, find(dark)?);
            if (light.variant, dark.variant) != (Variant::Light, Variant::Dark) {
                return Err("give a light palette first, then a dark one".to_owned());
            }
            let dark_now = desktop::system_prefers_dark();
            gtk_config()?
                .apply_pair(&light, &dark, dark_now)
                .map_err(|e| e.to_string())?;
            println!(
                "Applied {} and {}. Restart apps to see it.",
                light.name, dark.name
            );
            if let Err(e) = desktop::set_watch(&desktop::config_home()?, true) {
                println!("GTK3 apps will not follow light/dark switches: {e}");
            }
            Ok(())
        }
        ["watch"] => desktop::watch(desktop::config_home()?),
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
            desktop::set_watch(&desktop::config_home()?, false)
        }
        ["status"] => {
            for s in gtk_config()?.status().map_err(|e| e.to_string())? {
                let css = match s.css {
                    CssState::Missing => "no gtk.css".to_owned(),
                    CssState::Pinta(id) => format!("Pinta palette {id}"),
                    CssState::PintaPair { light, dark } => {
                        format!("Pinta palettes {light} (light) and {dark} (dark)")
                    }
                    CssState::Foreign => "gtk.css not written by Pinta".to_owned(),
                };
                let backup = if s.has_backup { " (backup kept)" } else { "" };
                println!("{}: {css}{backup}", s.toolkit.dir_name());
            }
            Ok(())
        }
        ["help" | "-h" | "--help"] => {
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
    desktop::config_home().map(GtkConfig::new)
}
