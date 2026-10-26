# Changelog

## Unreleased

- Smooth color changes: with Screen Fade on, switching light/dark or the accent
  now crossfades too, not only Apply, and the fade is gentler. The Shell theme
  reloads once, without a flash of GNOME's default theme. Log out and back in
  once after updating.
- The watch service restarts itself after an update, instead of running the old
  version until the next login.

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
