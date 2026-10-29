# How Recoolor works

Recoolor recolors a whole GNOME desktop from one palette: GTK and libadwaita apps,
Flatpaks, GNOME Shell, the wallpaper, lock and login screens, and apps with themes of
their own. This document explains how, and why it is built the way it is, as of
version 1.5.0. For installing and using it, see the [README](../README.md); for what
it cannot do, see its [Scope and limitations](../README.md#scope-and-limitations).

1. [Choosing the palettes](#1-choosing-the-palettes)
2. [GNOME Shell](#2-gnome-shell)
3. [GTK, libadwaita and Flatpak apps](#3-gtk-libadwaita-and-flatpak-apps)
4. [Apps with themes of their own](#4-apps-with-themes-of-their-own)
5. [Safety, security and performance](#5-safety-security-and-performance)
6. [The Recoolor window](#6-the-recoolor-window)

## 1. Choosing the palettes

### Colors by role

A palette is a small TOML file of colors by role, not by widget:

| Role | Used for |
|---|---|
| `base`, `mantle`, `crust` | Window, side bars and header bars, the darkest (or lightest) edges |
| `surface`, `overlay` | Cards and raised areas, borders and dim icons |
| `text`, `subtext` | Body text and secondary text |
| `accent` | The palette's signature color: buttons, selection, focus |
| `red`, `orange`, `yellow`, `green`, `blue`, `teal`, `purple`, `pink` | Errors, warnings, success, links, and the accent choices |

Every app is themed from these same roles, so a palette looks like one design
everywhere. A palette can also name the version of itself that other apps already
ship (Ptyxis's Catppuccin Mocha, a fish theme), and Recoolor uses that when it is
installed, so the terminal matches what the palette's own authors made.

### Recoolor's own palettes

Recoolor ships 21 palettes from well-known families (Catppuccin, Dracula, Everforest,
Gruvbox, Kanagawa, Nord, One Dark, Rosé Pine, Solarized, Tokyo Night), with their
colors taken from the authors' published values. Light and dark members of a family
(Catppuccin Latte and Mocha, Rosé Pine Dawn and Moon) pair up for **Follow System
Style**.

Where a family's colors fall just under a contrast target, Recoolor keeps the
authors' colors rather than change the palette people chose it for. The few cases are
listed in the README's limitations.

### Palettes from Gogh

179 more come from [Gogh](https://github.com/Gogh-Co/Gogh)'s terminal themes.
Terminal themes are made for text on a dark or light background, not for a whole
desktop, so `scripts/import-gogh.py` keeps only those that make a readable desktop:

| Check | Threshold |
|---|---|
| Text on the window | 7:1 (WCAG AAA) |
| Secondary text | 4.5:1 (WCAG AA) |
| Each color on the window (icons, large text) | 3:1 |
| Each color on cards | 2.5:1 |
| Text on accent buttons | 4.5:1 |
| Accent, red, green and yellow where libadwaita shows them as text | 4.5:1 |
| Window color | Near neutral (OKLab chroma under 0.06) |
| Dark windows | OKLab lightness 0.16 to 0.36: no pure black, not dim |
| Light windows | OKLab lightness 0.88 or more |
| The six main colors | Clear (chroma 0.06 or more) and at least 18° apart in hue |

A color that misses by a little may move up to 0.15 in OKLab lightness to pass; a
theme that would need more is left out. Themes that are near copies of each other
(OKLab distance under 0.03) and families Recoolor already ships are skipped. The
script also finds 25 light/dark pairs among them.

A test in `crates/recoolor-core/src/palette.rs` holds every bundled palette to these
thresholds, so a change to the converter cannot quietly let an unreadable palette in.

### Following libadwaita's own color rules

libadwaita does not use a fill color like red or the accent as text directly. It
derives a darker form in light style and a lighter one in dark style, so links,
warnings, errors and destructive buttons stay readable. Recoolor gives libadwaita the
palette's colors and lets it derive those forms, and checks that they reach 4.5:1 on
the window. Recoolor also follows GNOME's layout rules ([HIG](https://developer.gnome.org/hig/)):
cards sit a step lighter than the window in light style, row separators and card edges
stay visible, and toggle groups and the tab overview take the palette instead of stock
white or grey. With High Contrast on, the palette stays, and GNOME's high-contrast
borders and focus rings are drawn in its colors.

## 2. GNOME Shell

### The Shell theme

Recoolor does not ship a fixed Shell theme. It reads the installed Shell's own
stylesheet and replaces its colors with the palette's, so the theme always matches the
GNOME version on the machine, and a GNOME update does not leave it with missing parts.
The result is a user theme named Recoolor, loaded through the User Themes extension:
the top bar, quick settings, the calendar and notifications, the overview, Alt+Tab and
the on-screen displays.

The details that made it look native:

- Popups and cards take the palette's window and card colors, with the same lighter
  cards in light style as GTK apps, and a hover color mixed from the card toward the
  text, as GNOME Settings does.
- GNOME keeps the Shell dark unless the style is `prefer-light`, so while Recoolor
  themes the Shell, light style is stored as `prefer-light`. Reset puts back what was
  there.
- Blur my Shell and SearchLightNG are followed: popups are as see-through as Blur my
  Shell makes them, and solid when its popup blur is off.

### Recoolor's Shell extension

Some things can only be done from inside the Shell, so Recoolor installs a small
extension of its own (`recoolor@cjts15.github.io`). It talks to Recoolor over D-Bus
(`io.github.cjts15.Recoolor.Shell`):

- **Screen Fade.** `Freeze` covers the screen with a still image of itself, Recoolor
  applies the change underneath, and `Fade` fades the still away, close to GNOME's own
  Dark Style transition. The still is removed after 3 seconds at most, so the screen
  can never stay frozen.
- **ReloadTheme** reloads the Shell theme in place, without a log out.
- **Lock screen image.** GNOME draws the lock screen from the desktop wallpaper; the
  extension draws Recoolor's own image instead, sharp or blurred.
- **RunningApps** and **RestartApps** list the open apps and restart the ones you
  choose (see section 6). Before reopening an app, the extension waits until the old
  one's D-Bus name is free, so the new copy does not just hand over to the old one.
- It also adds classes the theme uses for solid popups and High Contrast.

### Light and dark, live

A small user service, `recoolor-watch.service`, listens to GNOME's Dark Style and
accent settings. When either changes, from Quick Settings, Settings or anywhere else,
it freezes the screen, switches every themed part to the other half of the pair (Shell,
GTK, terminal, editors, VS Code, wallpaper), and fades back. Switching back and forth
quickly fades each time.

### Wallpaper, lock screen and login screen

- **Wallpaper:** a light and a dark wallpaper, optionally tinted. **Match Palette**
  keeps the image and moves its colors to the palette's; **Mono** makes a gradient
  map. Tinting uses a lookup table, done once, for the light and dark images in
  parallel, on images up to 4K.
- **Lock screen:** an image of its own, through the extension.
- **Login screen:** through GDM Settings, so Recoolor never writes system files.
  **Set Up…** explains the one-time link that lets GDM Settings see Recoolor's theme.
  **Update…** then fills in GDM Settings' choices (theme, accent, icons, cursors, the
  background image) while it is closed, and opens it. Recoolor reads which palette the
  login screen shows from the stylesheet GDM keeps, and reminds you when it is out of
  date.

## 3. GTK, libadwaita and Flatpak apps

### libadwaita (GTK 4) apps

libadwaita draws everything from a set of named colors (`window_bg_color`,
`accent_bg_color`, `card_bg_color` and so on). Recoolor writes
`~/.config/gtk-4.0/gtk.css`, which sets those named colors to the palette's roles, once
for light and once for dark. Since both halves are in the file, a light/dark switch is
live; a new palette shows when an app restarts, because GTK 4 reads `gtk.css` only at
startup. A test loads the generated CSS into GTK's own parser, so a typo cannot ship.

### GTK 3 apps

GTK 3 apps get the same palette in `~/.config/gtk-3.0/gtk.css`. With **adw-gtk3 for
GTK3 Apps** on (it needs `adw-gtk3-theme`), GTK 3 switches to the libadwaita look and
takes the same named colors, so GTK 3 and GTK 4 apps match. Without it, the common
widgets are recolored directly.

### Flatpak apps

Flatpak apps cannot see `~/.config` by default. Recoolor adds read-only access to just
the two GTK folders (`xdg-config/gtk-3.0:ro` and `xdg-config/gtk-4.0:ro`) to your
Flatpak overrides, only if they are not already there, and records what it added so
Reset removes only that.

### Qt and KDE apps

**Qt and KDE Apps** writes a KDE color scheme to `~/.config/kdeglobals`, which KDE apps
and Qt Flatpaks such as Kdenlive read. Other Qt apps use Qt's GTK 3 theme on GNOME, so
they follow the GTK 3 colors.

## 4. Apps with themes of their own

Many apps ignore GTK and have a theme system of their own. For each, Recoolor uses the
app's own extension point, the way a user would install a theme by hand, and never
patches the app:

| App | Approach |
|---|---|
| Spotifast | A `Recoolor.json` custom theme in its themes folder. Spotifast watches that folder, so a selected Recoolor theme updates live. Works for the Flatpak and a native install. |
| VS Code | A local theme extension with Recoolor Light and Recoolor Dark. Recoolor switches the theme under the screen fade, and only rewrites the extension when its colors change, so a switch is smooth. |
| Firefox, Thunderbird | `userChrome.css`, and `userContent.css` for the browser's own pages, built on the browser's own color variables and firefox-gnome-theme's when it is installed. The new tab and blank page take the palette too. Light/dark follows the system live. |
| Chrome | Its GTK mode, so it takes the GTK colors. |
| Ptyxis | Gogh's version of the palette when Ptyxis has it, else a generated palette. |
| fish | Your installed fish theme for the palette, else generated colors; an oh-my-posh prompt gets a copy of your config in the palette's colors. Open shells follow. |
| bash | A prompt in `~/.bashrc.d/recoolor.sh`, which open shells read again at their next prompt. |
| btop | A generated `recoolor.theme`; btop reloads its config on a signal. |
| yazi | Light and dark flavors; yazi picks one by the terminal's background. |
| Text Editor | A GtkSourceView style scheme for light and dark. |
| Slack | Slack has no theme file, so Recoolor shows the four colors to paste. |

Each of these is a switch in the window, and each is undone by Reset.

## 5. Safety, security and performance

### Your setup is safe

- **Never overwrites what it did not write.** Every generated file starts with a
  "Generated by Recoolor" line (or is Recoolor's by name, where the format has no
  comments). A file of your own in the way, such as a `gtk.css` or yazi's
  `theme.toml`, is renamed to a `.recoolor-backup` and moved back on Reset.
- **Records before it changes.** Each setting Recoolor changes (wallpaper, icon and
  GTK themes, the Shell theme, the Text Editor scheme, Ptyxis's palette, btop's,
  VS Code's and fish's config lines) is recorded once in
  `~/.local/state/recoolor/state.toml`, with its old value. Reset puts back exactly
  those, and nothing else.
- **Atomic writes.** Files are written to a temporary file and renamed into place, so
  a crash never leaves a half-written config.
- **One Reset undoes everything**, including the Flatpak overrides it added and the
  watch service.

### Security

- **No system files, no root.** Recoolor works in your home folder. The one-time
  login-screen link is a `sudo` command Recoolor shows you to run yourself.
- **No network.** Recoolor makes no network requests; palettes and themes are bundled.
- **Least access for Flatpaks.** Only read-only access to the two GTK folders.
- **Checked input.** Palette ids are checked so a user palette cannot write outside
  Recoolor's folders, unknown keys are rejected, and paths written into other apps'
  settings are quoted.
- **Few dependencies.** The core uses `serde`, `toml`, `serde_json` and `thiserror`;
  the app adds only GTK 4 and libadwaita.

### Performance

- **Only writes what changed.** A light/dark switch rewrites only what differs, and
  apps are only told to reload when their theme really changed, so a switch takes
  milliseconds and does not flicker.
- **Under the fade.** Slow steps (VS Code, the Shell reload) run first, under the
  frozen screen, so the screen shows one clean change.
- **Fast wallpapers.** Tinting uses a precomputed color table, runs light and dark in
  parallel, and caps images at 4K. The image for the other half of a pair is kept, so
  switching back does not tint again. This took the first Recoolor after a Reset from
  over 25 seconds (in a debug build) to a few.
- **A light service.** The watch service waits on GNOME's settings signals and does
  nothing in between.

## 6. The Recoolor window

The window is a libadwaita app that follows GNOME's own patterns:

- **Live preview.** Picking a palette restyles Recoolor's own window, so you see it
  before applying.
- **Recoolor button** in the header bar, with **Reset…** in its dropdown. Reset asks
  first, and the button shows a spinner while it works.
- **Search and favorites.** A search box finds palettes by name in light or dark, the
  Light and Dark Style lists are searchable, and a star on a card keeps it among your
  favorites. **Show All** lists both styles.
- **System Style.** **Follow System Style** pairs a light and a dark palette that
  switch with GNOME's Dark Style.
- **Accent color** from the palette's own colors, kept in step with Settings →
  Appearance in both directions.
- **Grouped switches.** Apps are grouped as Terminal, Browsers and Email, GNOME Apps,
  Flatpak Apps, Custom Themes and Other Apps. An app's row shows only while the app is
  installed; one that needs something first says what.
- **Restart Apps.** After a new palette, a dialog lists the open apps that need a
  restart. Each quits the way its own Quit does, saving its session, and comes back as
  it was.
- **Adaptive layout.** One column on a narrow window (down to 360 px), four cards per
  line when maximized.
- **Readable errors and a log.** Errors say what to do, and everything is logged to
  `~/.local/state/recoolor/recoolor.log` and the journal.
- The same works from the command line: `recoolor list`, `apply`, `status`,
  `restore`, `show`.
