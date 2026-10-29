# Recoolor

Recoolor your whole GNOME desktop from one palette. Pick Catppuccin, Dracula, Tokyo
Night, Gruvbox, Rosé Pine or another of 200 bundled palettes, and Recoolor
themes GTK and libadwaita apps, GNOME Shell, the wallpaper and lock screen, Qt
apps, and apps with themes of their own such as Firefox, VS Code and the terminal.
One Reset puts everything back the way it was.

Requires GNOME 47 or newer (GTK 4.16, libadwaita 1.6). Built and tested on GNOME 50.

## Install

**Fedora:** download `recoolor-<version>.rpm` from the
[latest release](https://github.com/CJTS15/recoolor/releases/latest), then:

```sh
sudo dnf install ./recoolor-*.rpm
```

**From source:** needs `gtk4-devel libadwaita-devel` (Fedora) or
`libgtk-4-dev libadwaita-1-dev` (Debian/Ubuntu), and a Rust toolchain (1.89+).

```sh
make build
sudo make install            # into /usr/local
make install PREFIX=~/.local # or just for you; ~/.local/bin must be on PATH
```

To build the RPM yourself, install `rpm-build desktop-file-utils appstream` and
run `packaging/build-rpm.sh`; it builds from the committed tree into
`target/rpmbuild`.

**Uninstall:** Reset in the window (or `recoolor restore`) first, so everything
Recoolor changed is put back, then `sudo dnf remove recoolor` or
`sudo make uninstall`.

## Using it

Open **Recoolor**, pick a palette card to preview it on Recoolor's own window, turn
on what else it should theme, and press **Recoolor** in the header bar. **Reset…**,
in its dropdown, undoes all of it after asking. The search box above the palettes
finds one by name, in light or dark; **Show All** lists both styles, and the star on
a card keeps it among your favorites. The main menu (☰) has **About Recoolor**. The
window adapts: one column when narrow, four cards per line when maximized.

- **Light and dark:** turn on **Follow System Style** to use one palette in light
  style and another in dark (both lists are searchable). Everything switches with
  GNOME's Dark Style toggle.
- **Accent color:** the Accent Color row picks among the palette's own colors:
  Blue gives the palette's blue, Pink its pink, and so on. It sets GNOME's accent
  too, so Settings → Appearance shows the same choice, and picking there works as
  well. **Palette's Own** uses the palette's signature accent.
- **Restart Apps:** apps read their theme when they start. After a new palette,
  Recoolor lists the open apps that need a restart and restarts the ones you tick:
  each quits the way its own Quit does, saving its session, and comes back as it
  was. Files and Calendar keep running in the background, so Recoolor restarts
  those by itself.

The same works from the command line:

```sh
recoolor list                                     # bundled and user palettes
recoolor apply catppuccin-mocha                   # apply, keeping the extras that are on
recoolor apply catppuccin-latte catppuccin-mocha  # a light/dark pair
recoolor status                                   # what is applied
recoolor restore                                  # undo everything
recoolor show catppuccin-mocha [gtk3|shell]       # print the generated CSS
```

## What it themes

| | How |
|---|---|
| **libadwaita apps** (most GNOME apps) | Fully, through the named colors libadwaita draws everything from, used as libadwaita intends (see below). Restart them to see a change; light/dark switches live. |
| **GTK3 apps** | Fully with **adw-gtk3 for GTK3 Apps** on (needs `adw-gtk3-theme`), which switches GTK3 to the libadwaita look. Without it, the common widgets are recolored directly. |
| **Qt and KDE apps** | **Qt and KDE Apps** writes a KDE color scheme (`~/.config/kdeglobals`) for KDE apps and Qt Flatpaks such as Kdenlive. Other Qt apps use Qt's GTK3 theme on GNOME, so they follow the GTK3 colors. |
| **Flatpak apps** | Through read-only access to the GTK settings folders. Recoolor adds it to your Flatpak overrides if it is not already there. |
| **GNOME Shell** | The top bar, quick settings, calendar and overview, recolored from the installed Shell's own stylesheet. Needs the User Themes extension. Popups and cards take the palette's window and card colors; the Alt+Tab switcher follows light and dark, and the volume OSD's bar takes the accent. With Blur my Shell's "Preserve shell theme" popups follow its blur: see-through as much as it makes them, solid when its popup blur is off. SearchLightNG follows the palette too; if its blur shares a Blur my Shell pipeline and looks darker than the other popups, give it a pipeline of its own with brightness 1.0. With High Contrast on, the palette stays and GNOME's high-contrast borders and focus rings are drawn in its colors. |
| **Wallpaper** | A light and a dark wallpaper, optionally tinted to the palette: **Match Palette** keeps the image and moves its colors to the palette's, **Mono** makes a gradient map. |
| **Lock screen** | An image of its own, sharp or blurred (GNOME otherwise shows the desktop wallpaper, blurred). |
| **Icons and cursors** | Any installed theme. |
| **Login screen** | Through GDM Settings: **Set Up…** explains the one-time step, then **Update…** fills in GDM Settings' choices (theme, accent, icons, cursors, background) and opens it. GDM keeps a copy of the theme from when GDM Settings last applied it, so after a palette change Recoolor reminds you to apply it there again. |

Apps with themes of their own, in the Terminal, Browsers and Email, GNOME Apps,
Custom Themes and Other Apps groups:

| App | How | Live? |
|---|---|---|
| Terminal (Ptyxis) | Gogh's palette for it when there is one, else a generated one | Yes |
| fish | Your fish theme for the palette, else generated colors; an oh-my-posh prompt gets the palette too | Yes |
| bash | A prompt in the palette's colors (`~/.bashrc.d/recoolor.sh`) | At the next prompt |
| btop | A generated `recoolor.theme`, selected in `btop.conf` | Yes |
| yazi | `recoolor-light` and `recoolor-dark` flavors | New windows |
| Text Editor | A style scheme for light and dark | Yes |
| VS Code | A local Recoolor theme extension | Yes |
| Firefox, Thunderbird | `userChrome.css` (needs `toolkit.legacyUserProfileCustomizations.stylesheets`, and their theme following the system) | One restart; then light/dark switches live |
| Chrome | Its GTK mode (Settings → Appearance → GTK) | After a restart |
| Spotifast | A Recoolor custom theme | Yes |
| Slack | Shows the four colors to paste in its theme settings | Manual |

The optional **Screen Fade** crossfades the screen when Apply runs and when you
switch light/dark or the accent, from Quick Settings or Settings, however quickly. It, the lock screen image and the Blur my Shell and
SearchLightNG colors use a small Recoolor Shell extension; log out and back in once
after it is first installed or updated.

## Follows GNOME's design rules

Recoolor uses libadwaita's colors for what they are meant for, so palettes read
the way GNOME's [design guidelines](https://developer.gnome.org/hig/) expect:

- Links, accent labels, warnings, errors and destructive buttons use the darker
  (light style) or lighter (dark style) form libadwaita derives for text, at
  least 4.5:1 on the window.
- Row separators and card edges stay visible in light palettes, and cards sit a
  step lighter than the window, as in GNOME.
- Toggle groups and the tab overview take the palette, not stock white or grey.
- High Contrast keeps its borders and focus rings, in GTK apps and the Shell.

## Your setup is safe

- Recoolor never overwrites or deletes something it did not write. Its files start
  with a "Generated by Recoolor" line; a `gtk.css` of your own is renamed to
  `gtk.css.recoolor-backup` and moved back on Reset.
- Settings it changes (wallpaper, icon and GTK themes, the Text Editor scheme, the
  Shell theme, Ptyxis's palette, btop's and VS Code's theme lines) are recorded once in
  `~/.local/state/recoolor/state.toml`, and Reset puts back exactly those.
- A small user service, `recoolor-watch.service`, keeps things in step when you
  switch light/dark or pick another accent. Reset stops and removes it.

## Palettes

Each palette is a TOML file of colors by role (see `crates/recoolor-core/palettes`).
Drop your own into `~/.local/share/recoolor/palettes/`; one with the same `id` as a
bundled palette replaces it.

Besides Recoolor's own 21, 179 come from [Gogh](https://github.com/Gogh-Co/Gogh)'s
terminal themes (MIT, see `palettes/gogh/LICENSE-MIT`), including 25 light/dark
pairs. Only those that make a readable desktop are kept: text at 7:1, each color
at 3:1 on windows, the accent, red, green and yellow at 4.5:1 where libadwaita uses
them as text, a clear accent, near-neutral windows that are neither pure
black nor dim, and colors far enough apart to tell apart. `scripts/import-gogh.py`
converts them from Gogh's `data/themes.json`; a test holds every one to those
thresholds.

How the palettes are chosen, and how each part of the desktop is themed, is
written up in [docs/HOW-RECOOLOR-WORKS.md](docs/HOW-RECOOLOR-WORKS.md).

## Scope and limitations

Recoolor themes the GNOME desktop from outside the apps, through the files and
settings each one reads. Where an app offers no such way, or reads it only once,
these are the limits:

**GTK and libadwaita apps**

- A new palette shows after the app restarts: GTK reads `gtk.css` only at startup.
  Light/dark switches live.
- GTK3 apps without adw-gtk3 get their common widgets recolored, not the full
  libadwaita look.

**Qt and KDE apps**

- Qt Flatpaks (Kdenlive, EasyEffects and the like) take the palette when
  they start, but do not follow a light/dark switch while running: inside Flatpak,
  KDE's platform theme only listens to the settings portal, and only Plasma's portal
  reports those changes. Restart them after a switch.
- Their header bar uses Qt's own Adwaita decoration, whose colors are fixed (white in
  light style, `#2e2e32` in dark) and cannot be set from outside.
- Host (non-Flatpak) Qt apps such as OBS and Fedora Media Writer are not themed; OBS
  has themes of its own.

**GNOME Shell and the screen**

- The Shell theme needs the User Themes extension. Screen Fade, the lock screen image
  and the Blur my Shell and SearchLightNG colors need Recoolor's own Shell
  extension; log out and back in once after it is installed or updated.
- While Recoolor themes the Shell, GNOME's `default` style is stored as
  `prefer-light`, since GNOME keeps the Shell dark otherwise. Reset puts it back.
- SearchLightNG's blur can look darker when it shares a Blur my Shell pipeline; give
  it a pipeline of its own with brightness 1.0.
- Screen Fade holds the screen for at most 3 seconds, then fades even if an app is
  still redrawing.
- The first switch to light after a new palette can take about a second and a half
  longer, while the lock screen image is tinted for that style.
- Tinted wallpapers are made at up to 4K (3840 × 2160); larger images are scaled
  down first.

**Login screen**

- Only through GDM Settings (a Flatpak). The one-time **Set Up…** link into
  `/usr/local/share/themes` needs `sudo`; Recoolor itself never writes system files.
- The login screen shows one palette: the style that was on screen when you pressed
  Apply in GDM Settings. It does not follow light/dark or later palette changes until
  you apply there again.
- A GNOME Shell update replaces the login stylesheet, so the login screen goes back
  to GNOME's look until you apply again. To remove it, reset in GDM Settings.

**Apps with themes of their own**

- Firefox and Thunderbird need `toolkit.legacyUserProfileCustomizations.stylesheets`
  and a theme that follows the system, and one restart. Only the browser is themed,
  not websites. Recoolor waits until a window has been open for 10 seconds before
  restarting it, so a fresh start is not mistaken for a crash.
- Chrome needs its GTK mode and a restart. Slack takes its colors by hand.
- VS Code: with `window.autoDetectColorScheme` on, VS Code ignores the selected
  theme, so Recoolor skips it until that is off. After a new palette, the first
  light/dark switch may repaint once.
- yazi cannot reload its theme while running, so new windows show a change. A
  `theme.toml` of your own would override any flavor, so it is moved to
  `theme.toml.recoolor-backup` and comes back on Reset.
- fish with oh-my-posh: the oh-my-posh config needs a named `palette`, and Recoolor
  changes the `--config` path on the `oh-my-posh init fish` line of `config.fish`
  (Reset puts it back).
- bash: needs a `.bashrc` that reads `~/.bashrc.d` (Fedora's does). A prompt from
  oh-my-posh or starship is left alone.
- Ptyxis: when only one palette is applied and Gogh's version of it has both a light
  and a dark side, Recoolor generates a palette of its own instead.

**Restart Apps**

- Apps that do not save their session open fresh.
- An app still running 15 seconds after it was asked to quit is not reopened.
- Without Recoolor's Shell extension, only the browsers can be restarted.

**Palettes**

A few of Recoolor's own palettes keep their authors' colors even where they fall
just under a target, rather than drifting from the original:

- Tokyo Night Day: green used as text is 4.48:1, under 4.5:1.
- Catppuccin Latte, Everforest Light and Solarized Light: some fills are under 3:1
  on the window.
- Solarized Dark: text on cards is 4.11:1.

## When something goes wrong

Recoolor keeps a log at `~/.local/state/recoolor/recoolor.log`: every error it shows,
each Apply and Reset, the service's switches, and GTK warnings. The service also
logs to the journal (`journalctl --user -u recoolor-watch`).

## Development

```sh
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

- `crates/recoolor-core`: everything that writes files (palettes, CSS, wallpapers,
  app configs, the state record), with no GTK dependency, tested on temporary
  folders.
- `crates/recoolor`: the window (`src/ui`), the command line, the service, and the
  GNOME settings, D-Bus and image glue.
- `data`: desktop file, AppStream metadata and icons. `packaging`: the RPM.

One test loads the generated CSS into GTK's own parser and needs a display; it
skips without one. CI runs it under `xvfb-run`.

## License

GPL-3.0-or-later. See `LICENSE`.
