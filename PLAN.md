# Recoolor plan

Working plan for Recoolor after v0.1.2. Tick boxes as you go. It replaces the
old Pinta plan (steps 1 to 3 are done and shipped; see `CHANGELOG.md`).

## Where things are (v0.1.2)

- GTK3 and GTK4/libadwaita through generated `gtk.css`, light/dark pairs,
  GNOME's accent mapped onto the palette, and `recoolor watch` keeping it all in
  sync.
- Extras: Flatpak access, GNOME Shell (recolored from the installed Shell's own
  stylesheet), icons, cursors, Ptyxis, fish, VS Code, Firefox, Spotifast, Chrome,
  Slack colors, GDM through GDM Settings, and the Screen Fade extension.
- RPM built by the Release workflow on each `v*` tag.
- Not started: the upstream theme installer (old step 4). See "Dropped or
  deferred" below.

## Ground rules (unchanged)

- `recoolor-core` never depends on GTK. Files, settings values as data, CSS and
  pixel work live there and are unit-tested on temp dirs. GTK, GIO, D-Bus and
  image decoding live in the `recoolor` crate.
- Never overwrite or delete something Recoolor did not write: a marker (or a
  Recoolor-owned folder) to recognise its output, the previous value recorded
  once in `state.toml`, and a restore that undoes exactly that.
- One Reset (and `recoolor restore`) undoes everything.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and
  `cargo test` stay clean on every commit.

## Order at a glance

| # | Step | Why this order |
|---|------|----------------|
| 1 | Fixes from the v0.1.2 review | Wallpapers add slow image work, so Apply must already run off the main thread. |
| 2 | Wallpapers: desktop, lock screen, tinted to the palette | CJ's idea, and the biggest visible gap left. |
| 3 | Palette from a wallpaper | Reuses step 2's image pipeline; the other direction of the same feature. |
| 4 | Full GTK3 coverage through adw-gtk3 | Closes the last coverage gap for far less work than the upstream installer. |
| 5 | More apps: Text Editor/Builder, terminals, Thunderbird | Small, self-contained targets on the existing `apps/` pattern. |
| 6 | Palette editor and base16/base24 import | Makes the palette list open-ended. |
| 7 | COPR repository | `dnf upgrade` instead of downloading each RPM. |

---

## Step 1: Fixes from the v0.1.2 review

Review: https://claude.ai/artifact/ETxPYPBKr6A9fY71pdPbiP

- [ ] Apply and Reset off the main thread: insensitive buttons and a spinner
      while they run, file and subprocess work in `gio::spawn_blocking`,
      GSettings writes back on the main thread, then refresh. *(In progress in a
      separate PR.)*
- [ ] Restart Calendar with D-Bus activation
      (`StartServiceByName("org.gnome.Calendar", 0)`) so a Flatpak Calendar comes
      back too. *(Same PR.)*
- [ ] `set_ptyxis`: never record Recoolor's own `PALETTE_ID` as
      `ptyxis_previous`; fall back to Ptyxis's default.
- [ ] Lock `state.toml`: an `flock` on `recoolor/state.lock` around
      `set_extras`, taken by the GUI, the CLI and `recoolor watch`.
- [ ] Smaller ones: Flatpak Firefox profile root, say (or set through a marked
      `user.js` line) that Firefox needs
      `toolkit.legacyUserProfileCustomizations.stylesheets`, VS Code block
      comments and `window.autoDetectColorScheme`, `ConditionPathExists=` on the
      watch unit, the hidden selected card when its variant is filtered out, log
      trimming, and the leftover Pinta mention in `css/shell.rs`.

---

## Step 2: Wallpapers

Set the desktop and lock screen wallpapers from Recoolor, and optionally recolor
them to the palette, so the whole desktop matches.

### What GNOME actually does (checked against gnome-shell `main`)

- The desktop uses `org.gnome.desktop.background`: `picture-uri` in light style,
  `picture-uri-dark` in dark style, plus `picture-options`, `primary-color`,
  `secondary-color` and `color-shading-type`. The Shell switches between the two
  URIs by itself (`js/ui/background.js`).
- **The lock screen has no setting of its own.** Since GNOME 3.36 the unlock
  dialog draws the desktop background through the same `BackgroundManager`,
  blurred (radius 90, brightness 0.65; `js/ui/unlockDialog.js`).
  `org.gnome.desktop.screensaver picture-uri` still exists in the schema, but
  the Shell never reads it, so setting it alone changes nothing.
- So a separate lock screen image needs a Shell extension. Recoolor already
  ships one (Screen Fade), which can carry this too.
- GNOME's own wallpapers are often JPEG XL (`.jxl`), SVG, or XML slideshows, so
  decoding must go through the system's image loaders, not a Rust crate that only
  knows JPEG and PNG.
- The Shell caches backgrounds by file. Write each result under a new file name
  rather than overwriting one in place.

### 2.1 Pick and set (no tinting yet)

- [ ] New `recoolor-core/src/wallpaper.rs`: the folder Recoolor owns,
      `$XDG_DATA_HOME/recoolor/wallpapers/`, file naming
      (`<source-hash>-<palette-id>-<mode>.<ext>`), pruning old files (keep the
      ones in use plus the last few), and `is_ours(uri)` by path prefix. Images
      cannot carry a text marker, so the folder is the marker.
- [ ] `State` gains `background_previous` (a small struct: `picture-uri`,
      `picture-uri-dark`, `picture-options`, `primary-color`), recorded once like
      the other `*_previous` values. Never record a URI inside Recoolor's own
      folder as "previous" (same trap as the Ptyxis bug).
- [ ] `State` also keeps what the user chose: `wallpaper_source` (path to the
      original image) and `wallpaper_tint` (mode and strength). Retinting always
      starts from the original, so colors never compound across palettes.
- [ ] `targets.rs`: a `wallpaper` extra. On applies (writes the file, sets the
      keys); off restores `background_previous` and deletes Recoolor's files.
      Reset covers it.
- [ ] A pair maps naturally: the light palette's image goes to `picture-uri`, the
      dark one's to `picture-uri-dark`. The Shell switches them itself, so
      `recoolor watch` has nothing to do for light/dark.
- [ ] GUI: a **Wallpaper** group with a thumbnail of the current image, Choose
      Image (`gtk::FileDialog` with an image filter), Use Current Wallpaper
      (resolves the source from `wallpaper_source` when the current URI is
      already Recoolor's), and the tint controls from 2.2. The preview pane can
      show the thumbnail recolored live.
- [ ] CLI: `recoolor wallpaper set <file> [--tint match|mono|none] [--strength N]`,
      `recoolor wallpaper off`, and `status` shows the source.

### 2.2 Tint to the palette

Decode in the `recoolor` crate with `gdk_pixbuf` (it has whatever loaders the
system has: JPEG XL, AVIF, WebP, SVG), pass raw RGBA to core, encode the result
with `gdk_pixbuf` again. Core stays GTK-free and needs no image crate.

- [ ] `recoolor-core/src/wallpaper/tint.rs`: pure functions over `&mut [u8]`
      RGBA, unit-tested on small buffers.
- [ ] **Match** (default): the same idea as the Shell recolor in
      `css/shell.rs`. Greys map onto the palette's crust-to-text ramp by
      lightness; colored pixels move toward the palette color with the nearest
      hue, keeping their own lightness. A strength slider (0 to 100%) blends with
      the original. Move the hue table out of `css/shell.rs::map` into
      `color.rs` (or `palette.rs`) so the Shell and wallpapers agree.
- [ ] **Mono**: a gradient map through the ramp only (base, surface, overlay,
      subtext, text, plus the accent at the bright end). Looks good on photos.
- [ ] **None**: just set the image.
- [ ] Do the color math in OKLab, but bake it into a 3D lookup table (for
      example 33×33×33 RGB with trilinear lookup) built once per palette and
      mode, so a 4K image takes well under a second. Measure on a 3840×2160
      image in a test marked `#[ignore]`.
- [ ] Output: JPEG at quality 92 for photos, PNG when the source was PNG or SVG
      (flat art bands badly in JPEG). Keep the source's resolution; SVGs render
      at the largest monitor's size.
- [ ] Run the tint inside the `spawn_blocking` Apply from step 1, and cache by
      file name so reapplying the same palette is instant.
- [ ] `recoolor watch`: on an accent change, retint only when the mode uses
      the accent (Mono), and only in the background.

### 2.3 Lock screen

- [ ] Extend the Recoolor Shell extension (rename it "Recoolor" in metadata,
      same UUID so existing installs update in place):
      - `"session-modes": ["user", "unlock-dialog"]`, so it stays enabled while
        locked. Check that Screen Fade's D-Bus name survives the mode change.
      - Patch `UnlockDialog.prototype._createBackground`: when Recoolor's lock
        image is set, draw it with `Meta.Background.set_file(file, ZOOM)` in
        place of the `BackgroundManager`, and keep the blur effect.
      - Read the image path and blur choice from a small key file Recoolor writes
        (`$XDG_DATA_HOME/recoolor/lockscreen.conf`), watched with a
        `Gio.FileMonitor`. No GSettings schema needed.
- [ ] Options in the GUI: **Same as Desktop** (default; nothing to install),
      **Separate Image** (its own source and tint), and **Blur** on or off.
      With blur on at GNOME's radius, the image is barely recognisable, so
      default to off for a separate image.
- [ ] Also write `org.gnome.desktop.screensaver picture-uri` (recorded and
      restored) so other tools that read it agree; it costs nothing.
- [ ] `_createBackground` is a private method. Note the Shell version tested in
      the extension's `shell-version` and recheck after each GNOME upgrade, as
      for the Shell stylesheet.
- [ ] Without the extension enabled, the switch explains that the lock screen
      shows the desktop wallpaper, blurred.

### 2.4 Nice to have

- [ ] Generated wallpapers when there is no image: a solid color, a two-stop
      gradient, or a simple pattern from the palette, written as SVG so it is
      sharp at any size.
- [ ] Show Recoolor's wallpapers in Settings → Appearance by writing a marked
      `$XDG_DATA_HOME/gnome-background-properties/recoolor.xml`.
- [ ] Login screen: GDM Settings can set its background; add the image to the
      existing Copy Command.

**Done when:** one Apply sets a palette-tinted wallpaper for light and dark,
the lock screen can show its own image, and Reset brings back the user's
previous wallpaper and removes every file Recoolor wrote.

---

## Step 3: Palette from a wallpaper

The reverse of step 2: pick an image and get a palette that matches it.

- [ ] Core: quantize (k-means in OKLab on a downscaled copy, about 256×256),
      then assign roles. The darkest cluster with low chroma becomes the base
      ramp (dark variant) or the lightest one does (light variant); the rest map
      to red/orange/…/pink by hue, filling gaps by rotating hue at the image's
      average chroma.
- [ ] Enforce contrast: text on base at least 7:1, accent on base at least
      3:1, using `Color::contrast`. Nudge lightness until met.
- [ ] Save as a normal user palette TOML in `~/.local/share/recoolor/palettes/`
      (family "From Wallpaper"), so everything else works unchanged.
- [ ] GUI: **New Palette from Image** in the palette grid's menu, with a light
      and dark result side by side.

---

## Step 4: Full GTK3 coverage through adw-gtk3

GTK3 is the one target with partial coverage. adw-gtk3 is a GTK3 port of the
libadwaita look that is packaged on Fedora (`adw-gtk3-theme`) and draws from the
same named colors Recoolor already writes.

- [ ] Verify which named colors the installed adw-gtk3 reads from `gtk.css`
      (it was made to work with Gradience's output), and adjust `css/gtk3.rs` to
      emit exactly those.
- [ ] When adw-gtk3 is installed, offer **Use adw-gtk3 for GTK3 apps**: set
      `org.gnome.desktop.interface gtk-theme` to `adw-gtk3` / `adw-gtk3-dark`,
      record the previous value, and switch it on light/dark in `recoolor watch`.
- [ ] Flatpak: GTK3 Flatpaks need `org.gtk.Gtk3theme.adw-gtk3` installed;
      detect it and say so, like the icon theme note.

---

## Step 5: More apps

Each one is a module in `recoolor-core/src/apps/` with a marked file, recorded
previous value and exact undo, like the existing ones.

- [ ] **GNOME Text Editor and Builder**: a GtkSourceView 5 style scheme in
      `~/.local/share/gtksourceview-5/styles/recoolor.xml` (plus `-dark`), set
      through `org.gnome.TextEditor style-scheme` (and Builder's key). Flatpak
      Text Editor reads it from its own data dir; check.
- [ ] **Terminals**: Ghostty, kitty, Alacritty, foot. All take a palette file
      plus one include line, from the same 16 `ansi()` colors Ptyxis uses.
- [ ] **Thunderbird**: the Firefox module with a different profile root.
- [ ] **Qt apps**, later: a qt6ct/Kvantum color scheme. Only if you use Qt apps.

---

## Step 6: Palette editor and import

- [ ] Import base16 and base24 YAML (the tinted-theming scheme format) into the
      palette TOML, mapping `base00`…`base0F` to roles. Opens hundreds of
      existing schemes at once.
- [ ] Editor: start from any palette, edit each role with a color button, live
      preview on Recoolor's own window, contrast warnings, save to the user
      palettes folder.

---

## Step 7: COPR repository

- [ ] A Fedora COPR project fed from the existing spec file (`packaging/`), so
      updates arrive through `dnf upgrade`. The Release workflow can trigger the
      COPR build with a webhook.

---

## Dropped or deferred

- **Upstream theme installer** (old step 4): deferred, probably for good.
  Generated themes now cover everything the installer was meant to, step 4
  closes the GTK3 gap, and the installer would bring downloads, checksums,
  archive extraction and per-theme quirks to maintain. Revisit only if a
  specific upstream theme is wanted.
- **`Target` trait**: still not needed; `targets.rs` listing extras one by one
  is readable at this size. Revisit around 20 extras.
- **Recoolor as a Flatpak**: it writes host config and GSettings, which would
  need sandbox holes that defeat the point.
- **Other desktops**: out of scope.
- **Translations**: when someone other than CJ uses it.

## Useful references

- gnome-shell background and lock screen: `js/ui/background.js`,
  `js/ui/unlockDialog.js` in https://gitlab.gnome.org/GNOME/gnome-shell
- GSettings schemas: `gsettings list-recursively org.gnome.desktop.background`
- Shell extension session modes: https://gjs.guide/extensions/topics/session-modes.html
- libadwaita named colors: https://gnome.pages.gitlab.gnome.org/libadwaita/doc/main/css-variables.html
- GtkSourceView style schemes: https://gnome.pages.gitlab.gnome.org/gtksourceview/gtksourceview5/style-reference.html
- base16/base24 spec: https://github.com/tinted-theming/home
