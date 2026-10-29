# Changelog

## 1.5.0 — 2026-10-29

Window:

- The **Recoolor** button is back in the header bar, with Reset… in its dropdown.
- Favorite palettes: a star on each card, and a filter to show only favorites.
- Apps in groups by kind (Terminal, Browsers and Email, GNOME Apps, Flatpak Apps,
  Custom Themes, Other Apps), each shown only when the app is installed.
- One **Restart Apps** dialog lists every open app that needs a restart, Flatpaks
  included, and restarts the ones you tick. Firefox restarts reliably, also twice
  in a row.
- Wallpaper rows on two lines, HEIC previews, and a collapsible Preview.

Apps:

- yazi: Recoolor flavors for light and dark.
- bash: a prompt in the palette, following light/dark at the next prompt.
- fish: a Recoolor theme, and an oh-my-posh prompt recolored, both live in open
  shells.
- Qt Flatpaks on the KDE runtime take the palette when they start.
- VS Code switches light and dark under the screen fade, without a jump.
- Firefox: its own pages (Settings and other `about:` pages), the new tab and the
  Blank Page take the palette, without the new theme's violet.

Desktop:

- Shell: cards are lighter in light style, as in apps, and hovering them shows.
- SearchLightNG is styled only when its widgets are the ones Recoolor knows.
- Login screen: Recoolor shows which palette it has, and **Update…** fills in
  GDM Settings, the lock screen image included.
- Wallpapers tint much faster: at most 4K, light and dark at once. The lock image
  is kept for both halves of a pair, so switching to light is as fast as to dark.
- Fixed: Reset could make the Shell log a missing stylesheet.

Docs: a Scope and limitations section in the README, and
`docs/HOW-RECOOLOR-WORKS.md` on how Recoolor chooses palettes and themes each part
of the desktop.

## 1.4.0 — 2026-10-27

Follows GNOME's design guidelines (HIG):

- Readable text colors: links, accent labels, warnings, errors and destructive
  buttons use libadwaita's own derived text colors instead of the raw fills,
  at least 4.5:1 on the window. Gogh palettes are checked the same way.
- Light palettes: row separators and card edges are visible, and cards,
  boxed lists and popovers sit a step lighter than the window, as in GNOME.
- Toggle groups and the tab overview take the palette.
- High Contrast: the Shell keeps the palette and gets GNOME's high-contrast
  borders and focus rings in its colors. Log out and back in once after
  updating.

Window:

- Adapts to its size: one scrolling column with Apply in a bottom bar when
  narrow (down to 360 px), four cards per line when maximized.
- Apply moves to the top of the side pane, above the Preview; the header keeps
  the title and a main menu. Reset asks first, and Apply shows a spinner.
- The Light and Dark Style lists are searchable.
- After Apply, a reminder when the login screen still shows an older palette.

Screen Fade:

- Switching to light from Quick Settings fades instead of jumping, and quick
  switches fade too. A little faster: closer to GNOME's own transition.
- Fixed: the watch service could hang on a switch to light.

## 1.3.0 — 2026-10-27

- Alt+Tab: app and window titles take the palette's text color (they came out a
  stray warm white).
- libadwaita toasts take the palette: a dark bubble in both styles, as GNOME
  draws them, instead of a fixed grey.
- 179 more palettes, converted from Gogh's terminal themes and kept only where
  they make a readable desktop, including 25 light/dark pairs. A search box
  finds palettes by name in either style.

## 1.2.0 — 2026-10-26

- Firefox and Thunderbird follow light/dark live, without a restart, when
  Follow System Style is on: both palettes go in, the dark one under
  `prefers-color-scheme`. Restart them once after updating and applying.
- GNOME Shell popups and cards are exactly the palette's window and card colors.
- Blur my Shell: popups follow its blur instead of a fixed 90%: its own
  transparency in the palette's colors, solid while its popup blur is off
  (switching live), and readable in its transparent style. SearchLightNG
  matches. Log out and back in once after updating.
- Screen Fade no longer cuts short when Apply and the watch service both ask to
  fade.

## 1.1.0 — 2026-10-26

- Smooth color changes: with Screen Fade on, switching light/dark or the accent
  now crossfades too, not only Apply, and the fade is gentler. The Shell theme
  reloads once, without a flash of GNOME's default theme. Log out and back in
  once after updating.
- The watch service restarts itself after an update, instead of running the old
  version until the next login.
- Light style: the volume OSD, workspace switcher and screenshot panel stay dark
  with light text, as in GNOME (they were light with white text under Blur my
  Shell). The Alt+Tab switcher follows light and dark. The OSD's bar takes the
  accent color.
- SearchLightNG follows the palette in light and dark.
- Window: a heart menu with About Recoolor (version, license, GitHub links).
  Reset moves into Apply's dropdown, and Show All sits above the palettes.

## 1.0.1 — 2026-10-26

- VS Code: follows every Apply and light/dark switch. It stopped reloading the
  theme after the first change, and showed an old palette after switching
  between light and dark.
- GNOME Shell with Blur my Shell: quick settings, the calendar and notification
  cards take the palette's window and card colors at 90%, instead of Blur my
  Shell's own white or grey. This goes through Recoolor's Shell extension (an
  extension's styles outrank any Shell theme), so log out and back in once.

## 1.0.0 — 2026-10-26

- First stable release.
- btop: a generated theme, selected in `btop.conf`; open btop windows switch
  too.

## 0.2.0 — 2026-10-26

- Wallpapers: a light and a dark wallpaper, tinted to the palette (Match
  Palette keeps the image and moves its colors to the palette's; Mono is a
  gradient map), with a strength slider.
- Lock screen: an image of its own, sharp or blurred, through Recoolor's Shell
  extension (now called "Recoolor"; log out and back in once).
- adw-gtk3 for GTK3 apps: full GTK3 coverage through the libadwaita-style theme.
- Qt and KDE apps through a KDE color scheme; other Qt apps follow GTK3.
- GNOME Text Editor (a style scheme for light and dark) and Thunderbird.
- Restart Now: after Apply, Firefox, Thunderbird and Chrome can be restarted
  for you, each instance coming back as it was.
- Fixes: Apply and Reset run off the main thread; the state file is locked
  between the window, the command line and the service; Apply no longer
  reloads systemd when nothing changed; Flathub Firefox and XDG Thunderbird
  profiles are found.

## 1.0.0 — 2026-10-26

- First stable release.

- Apply and Reset run in the background, so the window no longer freezes while
  they wait on systemd, D-Bus and Calendar. The Apply button shows a spinner
  meanwhile, and closing the window does not cut the work off halfway.
- Calendar is started again through D-Bus activation, so a Flatpak Calendar's
  reminders come back after Apply or Reset too.

## 0.1.2 — 2026-10-23

- Accent picker: an Accent Color row with GNOME's nine named accents plus the
  palette's own, each drawn in the color it maps to. It sets GNOME's accent, so
  Settings shows the same choice, and follows changes made there.
- Calendar keeps a background service, so it never reloaded new colors; Apply
  and Reset now restart it, as they already did for Files.
- An accent-colored outline marks the selected palette card and accent swatch.
- Blur my Shell with Preserve shell theme: popup contents (calendar, cards,
  quick toggles) are now translucent in the palette's colors, and the Alt+Tab
  switcher labels stay readable in light mode.

## 0.1.1 — 2026-10-23

- GNOME Shell: after GDM Settings used Recoolor's theme for the login screen,
  Recoolor recolored its own colors again, leaving quick settings unreadable
  and the stylesheet growing with each use. It now ignores what GDM Settings
  appended.
- Light mode: GNOME keeps the Shell dark unless the style is `prefer-light`, so
  a light palette ended up on dark popups (notably with Blur my Shell). While
  the Shell is themed, light mode now uses `prefer-light`; Reset restores yours.
- The watcher no longer re-applies everything several times per change, which
  caused lag after applying.

## 0.1.0 — 2026-10-23

First release.

- Libadwaita window: 21 bundled palettes (Catppuccin, Dracula, Tokyo Night,
  Gruvbox, Rosé Pine, Everforest, Nord, One Dark, Solarized, Kanagawa), live
  preview with a color fade, Apply and Reset. Command line: `recoolor list`,
  `show`, `apply`, `restore`, `status`, `watch`.
- GTK3 and GTK4/libadwaita through generated `gtk.css`, with your own `gtk.css`
  backed up and restored.
- Light/dark pairs that follow GNOME's style switch.
- GNOME Shell (recolored from the installed Shell's own stylesheet), Flatpak
  apps, icons and cursors, and GNOME's accent color mapped to the palette.
- Apps: Ptyxis, fish, VS Code, Firefox (firefox-gnome-theme), Spotifast, Chrome
  (GTK mode), Slack colors to copy, and the login screen through GDM Settings.
- Optional Screen Fade: a small Shell extension crossfades the screen on Apply.
- Everything Recoolor changes is recorded and undone by Reset.
