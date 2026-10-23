# Changelog

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
