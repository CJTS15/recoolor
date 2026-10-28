//! oh-my-posh's prompt, when `config.fish` loads it with `oh-my-posh init fish --config
//! <file>`: Recoolor writes `recoolor.omp.json` next to that file, the same config with
//! only its named `palette` in the palette's colors, and points the line at it. The
//! blocks refer to the palette (`p:text`), so they stay exactly as they were. A pair
//! becomes two `palettes`, `light` and `dark`, picked by `RECOOLOR_COLOR_THEME`, which
//! Recoolor's fish config keeps in step (see `fish`).

use std::path::{Path, PathBuf};

use super::{remove_owned, write_owned};
use crate::apply::{ApplyError, write_atomic};
use crate::color::Color;
use crate::palette::Palette;

/// The file Recoolor writes, next to the user's own config.
pub const FILE: &str = "recoolor.omp.json";

const INIT: &str = "oh-my-posh init fish";

/// The `--config` argument of `config.fish`'s oh-my-posh line, as written.
pub fn config_arg(config_fish: &str) -> Option<&str> {
    config_fish.lines().find_map(|line| {
        let line = line.trim_start();
        if line.starts_with('#') || !line.contains(INIT) {
            return None;
        }
        let (_, rest) = line.split_once("--config")?;
        let rest = rest.trim_start_matches(['=', ' ']);
        let quoted = rest.starts_with(['"', '\'']);
        let rest = rest.trim_start_matches(['"', '\'']);
        let end = if quoted {
            rest.find(['"', '\''])?
        } else {
            rest.find(char::is_whitespace).unwrap_or(rest.len())
        };
        Some(&rest[..end]).filter(|arg| !arg.is_empty())
    })
}

/// `config_fish` with the oh-my-posh line's `--config` argument replaced by `arg`.
pub fn set_config_arg(config_fish: &str, arg: &str) -> String {
    let Some(old) = config_arg(config_fish) else {
        return config_fish.to_owned();
    };
    config_fish
        .lines()
        .map(|line| {
            if !line.trim_start().starts_with('#') && line.contains(INIT) {
                line.replacen(old, arg, 1)
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + if config_fish.ends_with('\n') {
            "\n"
        } else {
            ""
        }
}

/// The argument for Recoolor's file in the same folder, written the same way
/// (`$HOME/.config/ohmyposh/catppuccin.omp.json` → `$HOME/.config/ohmyposh/recoolor.omp.json`).
pub fn sibling_arg(arg: &str) -> String {
    match arg.rsplit_once('/') {
        Some((dir, _)) => format!("{dir}/{FILE}"),
        None => FILE.to_owned(),
    }
}

/// The argument as a path: `$HOME`, `${HOME}` and a leading `~` expanded.
pub fn expand(arg: &str, home: &Path) -> PathBuf {
    let home = home.display().to_string();
    let arg = arg.replace("${HOME}", &home).replace("$HOME", &home);
    match arg.strip_prefix('~') {
        Some(rest) => PathBuf::from(format!("{home}{rest}")),
        None => PathBuf::from(arg),
    }
}

/// The span of the top-level `"palette": { ... }` member in `json`, if any.
fn palette_span(json: &str) -> Option<(usize, usize)> {
    let key = json.find("\"palette\"")?;
    let open = key + json[key..].find('{')?;
    // Palette values are strings without braces, so the first `}` closes it.
    let close = open + json[open..].find('}')?;
    Some((key, close + 1))
}

/// Recoolor's color for a palette entry: by its name when that names a role
/// (`text`, `overlay0`, `peach`, `lavender`…), else by its original color.
fn mapped(name: &str, original: Color, p: &Palette) -> Color {
    let c = &p.colors;
    let name = name.to_ascii_lowercase();
    let name = name.trim_end_matches(|ch: char| ch.is_ascii_digit());
    match name {
        "text" | "fg" | "foreground" => return c.text,
        "subtext" => return c.subtext,
        "overlay" | "comment" | "gray" | "grey" => return c.overlay,
        "surface" | "selection" => return c.surface,
        "base" | "bg" | "background" => return c.base,
        "mantle" => return c.mantle,
        "crust" => return c.crust,
        "red" | "maroon" | "error" => return c.red,
        "orange" | "peach" => return c.orange,
        "yellow" | "warning" => return c.yellow,
        "green" | "success" => return c.green,
        "teal" | "cyan" | "aqua" | "sky" | "sapphire" => return c.teal,
        "blue" => return c.blue,
        "lavender" | "accent" | "primary" => return c.accent,
        "purple" | "mauve" | "violet" | "magenta" => return c.purple,
        "pink" | "rosewater" | "flamingo" => return c.pink,
        _ => {}
    }
    let [l, a, b] = original.to_oklab();
    if a.hypot(b) < 0.04 {
        // A grey, read as on a dark terminal like most prompt themes: the lighter, the
        // more it is text.
        return match l {
            l if l >= 0.7 => c.text,
            l if l >= 0.55 => c.subtext,
            l if l >= 0.4 => c.overlay,
            _ => c.surface,
        };
    }
    let hue = |color: Color| {
        let [_, a, b] = color.to_oklab();
        b.atan2(a)
    };
    let target = b.atan2(a);
    let distance = |color: Color| {
        let d = (hue(color) - target).abs();
        d.min(std::f32::consts::TAU - d)
    };
    [
        c.red, c.orange, c.yellow, c.green, c.teal, c.blue, c.purple, c.pink,
    ]
    .into_iter()
    .min_by(|x, y| distance(*x).total_cmp(&distance(*y)))
    .unwrap_or(c.accent)
}

/// The palette's entries in order, as (name, value).
fn entries(object: &str) -> Vec<(String, String)> {
    serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(object)
        .map(|map| {
            map.into_iter()
                .filter_map(|(k, v)| v.as_str().map(|v| (k, v.to_owned())))
                .collect()
        })
        .unwrap_or_default()
}

/// One palette object in `palette`'s colors, keeping values that are not colors.
fn object(entries: &[(String, String)], palette: &Palette, indent: &str) -> String {
    let lines: Vec<String> = entries
        .iter()
        .map(|(name, value)| {
            let value = Color::from_hex(value)
                .map(|c| mapped(name, c, palette).to_string())
                .unwrap_or_else(|_| value.clone());
            format!("{indent}  {}: {}", json(name), json(&value))
        })
        .collect();
    format!("{{\n{}\n{indent}}}", lines.join(",\n"))
}

fn json(s: &str) -> String {
    serde_json::Value::String(s.to_owned()).to_string()
}

/// `recoolor.omp.json` from the user's config `source` (named `source_name`), or why it
/// cannot be made.
pub fn render(
    source: &str,
    source_name: &str,
    now: &Palette,
    pair: Option<(&Palette, &Palette)>,
) -> Result<String, String> {
    let (start, end) = palette_span(source)
        .ok_or("its colors are not in a named palette, so Recoolor leaves it alone")?;
    let open = start + source[start..].find('{').unwrap_or(0);
    let entries = entries(&source[open..end]);
    if entries.is_empty() {
        return Err("its palette has no colors".into());
    }
    let indent = &source[source[..start].rfind('\n').map_or(0, |n| n + 1)..start];
    let member = match pair {
        None => format!("\"palette\": {}", object(&entries, now, indent)),
        Some((light, dark)) => {
            let inner = format!("{indent}    ");
            format!(
                "\"palettes\": {{\n\
                 {indent}  \"template\": \"{{{{ if eq .Env.RECOOLOR_COLOR_THEME \\\"light\\\" }}}}light{{{{ else }}}}dark{{{{ end }}}}\",\n\
                 {indent}  \"list\": {{\n\
                 {inner}\"light\": {},\n\
                 {inner}\"dark\": {}\n\
                 {indent}  }}\n\
                 {indent}}}",
                object(&entries, light, &inner),
                object(&entries, dark, &inner),
            )
        }
    };
    let body = format!("{}{member}{}", &source[..start], &source[end..]);
    // JSON has no comments: the marker is a key on the second line.
    let first = body.find('{').ok_or("it is not a JSON object")?;
    Ok(format!(
        "{}{{\n  \"$comment\": \"Generated by Recoolor from {source_name}. Reset in Recoolor removes it.\",{}",
        &body[..first],
        &body[first + 1..]
    ))
}

/// `config.fish` itself, after its one line changed (Recoolor records the old
/// argument and puts it back on Reset).
pub fn config_fish(config_home: &Path) -> PathBuf {
    config_home.join("fish/config.fish")
}

pub fn write_config(path: &Path, text: &str) -> Result<(), ApplyError> {
    write_atomic(path, text)
}

pub fn write(path: &Path, contents: &str) -> Result<(), ApplyError> {
    write_owned(path, contents)
}

pub fn remove(path: &Path) -> Result<bool, ApplyError> {
    remove_owned(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::bundled;

    const SOURCE: &str = r##"{
  "$schema": "https://raw.githubusercontent.com/JanDeDobbeleer/oh-my-posh/main/themes/schema.json",
  "palette": {
    "os": "#ACB0BE",
    "text": "#939AB7",
    "pink": "#F5BDE6",
    "lavender": "#B7BDF8",
    "blue": "#8AADF4"
  },
  "blocks": [ { "segments": [ { "foreground": "p:pink", "template": " {{ .Path }} " } ] } ],
  "version": 3
}
"##;

    #[test]
    fn finds_and_replaces_the_config_argument() {
        let fish = "# OhMyPosh theme\noh-my-posh init fish --config $HOME/.config/ohmyposh/catppuccin.omp.json | source\nset x 1\n";
        let arg = config_arg(fish).unwrap();
        assert_eq!(arg, "$HOME/.config/ohmyposh/catppuccin.omp.json");
        let ours = sibling_arg(arg);
        assert_eq!(ours, "$HOME/.config/ohmyposh/recoolor.omp.json");
        let changed = set_config_arg(fish, &ours);
        assert_eq!(config_arg(&changed), Some(ours.as_str()));
        assert_eq!(set_config_arg(&changed, arg), fish);
        assert_eq!(
            config_arg("# oh-my-posh init fish --config x | source\n"),
            None
        );
        assert_eq!(
            config_arg("oh-my-posh init fish --config '/a b/c.json' | source"),
            Some("/a b/c.json")
        );
        assert_eq!(
            expand("$HOME/x.json", Path::new("/home/me")),
            PathBuf::from("/home/me/x.json")
        );
    }

    #[test]
    fn only_the_palette_changes() {
        let p = |id: &str| bundled().into_iter().find(|p| p.id == id).unwrap();
        let (latte, mocha) = (p("catppuccin-latte"), p("catppuccin-mocha"));

        let single = render(SOURCE, "catppuccin.omp.json", &mocha, None).unwrap();
        assert!(
            single.starts_with(
                "{\n  \"$comment\": \"Generated by Recoolor from catppuccin.omp.json."
            )
        );
        // Everything but the palette is byte for byte the same.
        let (before, after) = single.split_once("\"palette\"").unwrap();
        assert!(SOURCE.starts_with(&before.replace(
            "  \"$comment\": \"Generated by Recoolor from catppuccin.omp.json. Reset in Recoolor removes it.\",\n",
            ""
        )));
        assert!(after.ends_with(SOURCE.split_once("  },\n").unwrap().1));
        let value: serde_json::Value = serde_json::from_str(&single).unwrap();
        let palette = &value["palette"];
        assert_eq!(palette["text"], mocha.colors.text.to_string());
        assert_eq!(palette["pink"], mocha.colors.pink.to_string());
        assert_eq!(palette["lavender"], mocha.colors.accent.to_string());
        // `os` is a light grey: text-like.
        assert_eq!(palette["os"], mocha.colors.text.to_string());

        let pair = render(
            SOURCE,
            "catppuccin.omp.json",
            &mocha,
            Some((&latte, &mocha)),
        )
        .unwrap();
        let value: serde_json::Value = serde_json::from_str(&pair).unwrap();
        assert!(value.get("palette").is_none());
        assert_eq!(
            value["palettes"]["list"]["light"]["text"],
            latte.colors.text.to_string()
        );
        assert_eq!(
            value["palettes"]["list"]["dark"]["text"],
            mocha.colors.text.to_string()
        );
        assert!(
            value["palettes"]["template"]
                .as_str()
                .unwrap()
                .contains("RECOOLOR_COLOR_THEME")
        );

        assert!(render("{ \"blocks\": [] }", "x.omp.json", &mocha, None).is_err());
    }
}
