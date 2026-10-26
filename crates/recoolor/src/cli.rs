//! Recoolor command line.

use recoolor_core::palette::{self, Palette, Variant};
use recoolor_core::{CssState, GtkConfig, RestoreOutcome, Toolkit, paths, shell};

use crate::desktop::{self, Dirs};
use crate::targets::{self, Choice};

const USAGE: &str = "\
Usage: recoolor <command>

Commands:
  list               List available palettes
  show <id> [gtk3|shell]
                     Print the generated GTK4 (or GTK3, or GNOME Shell) CSS
  apply <id>         Apply a palette to GTK apps, and to whatever else Recoolor
                     already themes (Flatpak, Shell, icons, cursors)
  apply <light> <dark>
                     Apply a pair that follows GNOME's light/dark style
  restore            Undo everything Recoolor changed
  status             Show what is applied
  watch              Keep everything in step with GNOME's light/dark style and
                     accent color (started for you as a user service by `apply`)

Run `recoolor` with no command to open the window, where Flatpak, Shell, icons and
cursors are turned on. Restart apps to see changes.";

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
        ["show", id, "shell"] => {
            let palette = find(id)?;
            print!("{}", desktop::shell_css(&palette, &Dirs::find()?.data)?);
            Ok(())
        }
        ["apply", id] => apply(Choice::Single(&find(id)?)),
        ["apply", light, dark] => {
            let (light, dark) = (find(light)?, find(dark)?);
            if (light.variant, dark.variant) != (Variant::Light, Variant::Dark) {
                return Err("give a light palette first, then a dark one".to_owned());
            }
            apply(Choice::Pair {
                light: &light,
                dark: &dark,
            })
        }
        ["watch"] => targets::watch(Dirs::find()?),
        ["restore"] => {
            let (outcomes, problems) = targets::reset(&Dirs::find()?);
            for (toolkit, outcome) in outcomes {
                let what = match outcome {
                    RestoreOutcome::Restored => "restored your previous gtk.css",
                    RestoreOutcome::Removed => "removed Recoolor's gtk.css",
                    RestoreOutcome::LeftForeign => "gtk.css is not Recoolor's, left it alone",
                    RestoreOutcome::NothingToDo => "nothing to restore",
                };
                println!("{}: {what}", toolkit.dir_name());
            }
            report(problems)
        }
        ["status"] => {
            for s in gtk_config()?.status().map_err(|e| e.to_string())? {
                let css = match s.css {
                    CssState::Missing => "no gtk.css".to_owned(),
                    CssState::Recoolor(id) => format!("Recoolor palette {id}"),
                    CssState::RecoolorPair { light, dark } => {
                        format!("Recoolor palettes {light} (light) and {dark} (dark)")
                    }
                    CssState::Foreign => "gtk.css not written by Recoolor".to_owned(),
                };
                let backup = if s.has_backup { " (backup kept)" } else { "" };
                println!("{}: {css}{backup}", s.toolkit.dir_name());
            }
            let dirs = Dirs::find()?;
            let extras = targets::current(&dirs);
            let shell = match shell::status(&dirs.data) {
                Ok(CssState::Recoolor(id)) => format!("Recoolor palette {id}"),
                _ => "not themed by Recoolor".to_owned(),
            };
            let or_unset = |v: Option<String>| v.unwrap_or_else(|| "not set by Recoolor".into());
            let on = |b: bool| {
                if b {
                    "can read the styles"
                } else {
                    "cannot read the styles"
                }
            };
            println!("flatpak: {}", on(extras.flatpak));
            println!("shell: {shell}");
            println!("icons: {}", or_unset(extras.icons));
            println!("wallpaper: {}", or_unset(extras.wallpaper));
            println!("dark wallpaper: {}", or_unset(extras.wallpaper_dark));
            println!("cursors: {}", or_unset(extras.cursors));
            match desktop::interface_settings().and_then(|s| desktop::accent_choice(&s)) {
                Some(name) => println!("accent: the palette's {name} (picked in Settings)"),
                None => println!("accent: the palette's own"),
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
        eprintln!("recoolor: skipping palette {e}");
    }
    palettes
}

fn find(id: &str) -> Result<Palette, String> {
    palettes()
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| format!("no palette {id:?}; run `recoolor list`"))
}

fn gtk_config() -> Result<GtkConfig, String> {
    Dirs::find().map(|d| GtkConfig::new(d.config))
}

/// Applies to GTK and to the extras Recoolor already manages, so the command line
/// recolors everything the window set up.
fn apply(choice: Choice) -> Result<(), String> {
    let dirs = Dirs::find()?;
    let extras = targets::current(&dirs);
    let problems = targets::apply(&dirs, choice, &extras, desktop::system_prefers_dark())
        .map_err(|e| e.to_string())?;
    println!("Applied {}. Restart apps to see it.", choice.name());
    report(problems)
}

/// Prints each problem; fails if there were any.
fn report(problems: Vec<String>) -> Result<(), String> {
    for p in &problems {
        eprintln!("recoolor: {p}");
    }
    match problems.len() {
        0 => Ok(()),
        n => Err(format!("{n} problem(s); see above")),
    }
}
