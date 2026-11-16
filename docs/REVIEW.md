# Code review: Recoolor 1.5.3

Reviewed on 2026-11-16 at `b04a4f0` (Release 1.5.3). The last review covered up to
v0.1.2, so this one looks hardest at what came after (wallpapers, lock and login
screen, the Shell extension, the restart dialog, the per-app themes, the watch
service), but it covers the whole codebase.

**How it was checked**

- Read every Rust file in both crates, plus the embedded Shell extension JS.
- `cargo test -p recoolor-core`: 93 passed, 1 ignored. `cargo clippy -p recoolor-core --all-targets`: clean.
- The `recoolor` crate was **not** built here: the review container has no GTK 4 or
  libadwaita dev packages. Findings in that crate come from reading the code.
- The VS Code finding (H2) was reproduced with a throwaway test against
  `recoolor_core::apps::vscode`.

Each finding is labelled **confirmed** (reproduced, or certain from the code path),
**likely** (strong reading of the code, not run on a desktop) or **possible**.

---

## High

### H1. Apply and Reset close Files and Calendar without asking (confirmed)

- `crates/recoolor/src/targets.rs:192` (Apply) and `:245` (Reset) call
  `desktop::restart_background_apps()` every time.
- `crates/recoolor/src/desktop.rs:253-289` activates Nautilus's `kill` action (it quits
  Files and closes every open Files window) and SIGTERMs Calendar.

Since 1.5.0 the **Restart Apps** dialog is meant to be the one place that restarts
apps, and only the ones the user ticks. But Files is already gone by the time the
dialog opens, so the `org.gnome.Nautilus.desktop` entry in `CHECKED`
(`crates/recoolor/src/ui/window.rs:998`) never matters, and Files windows are lost
without being reopened. Nautilus's quit may also cancel a copy in progress (possible;
not tested). Reset does the same thing.

**Fix:** remove `restart_background_apps()` from `apply`/`reset`. Let the dialog
restart Files through `RestartApps` like any other app. If the windowless Nautilus
or Calendar background service still needs restarting, offer it as a ticked row in
the same dialog ("Files (background)", "Calendar reminders"), and do it only when
nothing of theirs has a window open.

### H2. VS Code never switches theme when a glob comes before `workbench.colorTheme` (confirmed)

`crates/recoolor-core/src/apps/vscode.rs:316` decides whether a key sits inside a
block comment with `rfind("/*") > rfind("*/")`. Globs such as `"**/target/**"` or
`"**/*.log"` in `files.exclude` / `files.watcherExclude` contain `/*`, so every key
after them counts as commented out. The test showed:

```text
current = None
{
    "workbench.colorTheme": "Recoolor Dark",          <- inserted by Recoolor
    "files.watcherExclude": { "**/target/**": true },
    "workbench.colorTheme": "Default Dark Modern"     <- user's, last one wins
}
```

VS Code uses the last duplicate key, so the Recoolor theme never takes effect,
`vscode_previous` is recorded as "no key", and Apply reports success.

**Fix:** scan the JSONC once and track state (in a string, in `//`, in `/* */`)
instead of `rfind`. Only accept a key match at object depth 1, outside strings and
comments. Add the case above as a test. The same `value_start` helper backs
`auto_detects`, so that check has the same bug.

### H3. The state file is written only after every change, but the docs say "records before it changes" (confirmed)

`set_extras` (`crates/recoolor/src/targets.rs:257-359`) records each `*_previous`
in memory and changes GSettings and files straight away, but calls `state.save` only
at the end (`:356`). If the process dies partway, the user's previous values are gone
for good, and Reset can no longer put back the wallpaper, icon theme, Shell theme,
`color-scheme` or Text Editor scheme. Ways it can die partway: a panic (caught by
`spawn_blocking`, but the in-memory `State` is dropped), logging out during Apply,
systemd restarting `recoolor-watch`, or the watcher's own `process::exit(1)` after
an update.

`docs/HOW-RECOOLOR-WORKS.md` §5 says "Records before it changes … recorded once …
with its old value", which is not what the code does.

**Fix:** save the state right after each `get_or_insert`/`*_previous = Some(..)` and
before the change it guards. Simplest way: a `record(&mut state, &path, |s| ...)`
helper that saves at once. The writes are tiny and already atomic.

### H4. Browser restart can leave Firefox or Thunderbird closed (confirmed for the error path, likely for Flatpak)

`crates/recoolor/src/restart.rs:167-205`:

- The kill loop returns on the first `kill` failure (`?` at `:171`). One instance
  exiting between `main_pids()` and `kill` (a race) means the earlier instances have
  already been SIGTERMed and are never started again.
- The reopen loop also returns on the first spawn error (`?` at `:201`), so the
  remaining instances are not reopened. That contradicts the doc comment ("the
  others come back anyway").
- For a **Flatpak** Firefox or Thunderbird (`org.mozilla.firefox.desktop` is in the
  list), `/proc/<pid>/cmdline` is the sandbox path (`/app/lib/firefox/firefox`) and
  `environ` is the sandbox's own environment. Re-executing that on the host fails
  with ENOENT, so the browser is closed and stays closed.

**Fix:** collect errors per instance instead of using `?`. If an instance was
sandboxed (`FLATPAK_ID` in its environ, or `/proc/<pid>/root` is not `/`), or its
cmdline cannot be run, reopen it through its desktop entry (`reopen_from_desktop`).

---

## Medium

### M1. Atomic writes replace symlinked configs (confirmed)

`write_atomic` (`crates/recoolor-core/src/apply.rs:252-270`) renames a temp file
over the path. If the path is a symlink, which is common for dotfiles managed with
stow or chezmoi (`~/.config/Code/User/settings.json`, `config.fish`, `btop.conf`,
`~/.local/share/flatpak/overrides/global`), the link is replaced by a regular file.
The user's dotfiles repo stops getting changes, and Reset does not bring the link
back. `write_owned` (`apps/mod.rs:37`) uses `path.exists()`, which follows links,
so a dangling link counts as "missing" and is overwritten.

**Fix:** in `write_atomic`, resolve the path with `fs::canonicalize` when it is a
symlink. Write the temp file next to the *target* and rename onto the target.

### M2. Races between the window, the CLI and the watcher (likely)

- `write_atomic` always uses the same temp name, `.{name}.recoolor-tmp`
  (`apply.rs:259`). Two processes writing the same file share that temp file, so one
  can truncate it while the other renames it into place.
- `write_gtk` runs **outside** the state lock in both `apply`
  (`targets.rs:185`) and the watcher's `sync` (`targets.rs:1206`).
- The watcher reads `current(dirs)` (`targets.rs:1184`) before it takes the lock.
  During a window Apply that flips `color-scheme` from `default` to `prefer-light`
  (`set_light_shell`), the watcher wakes mid-apply. It reads half-updated extras,
  waits for the lock, and then applies them again. That can turn an extra the user
  just switched off back on (for example Firefox, which comes after "Light style" in
  the list). This is possible, and narrow: it needs the first Apply with the Shell
  themed.

**Fix:** use a unique temp name (pid plus a counter, or `O_TMPFILE`). Take the state
lock before `write_gtk`. In `sync`, take the lock first and read `current()` inside
it, and skip the sync if the lock is held by an Apply that is about to finish (or
re-read after acquiring).

### M3. Tinted wallpapers ignore EXIF orientation (likely)

`crates/recoolor/src/wallpaper.rs:42-46` loads with `Pixbuf::from_file` /
`from_file_at_scale` and never calls `apply_embedded_orientation()`. Portrait phone
photos come out sideways once tinted. With **Tint: None** the original file is used
and GNOME rotates it correctly, so the bug shows only with Match or Mono.

**Fix:** `let image = image.apply_embedded_orientation().unwrap_or(image);` before
reading the pixels. Swap the 3840×2160 bound for portrait sources if you want the
cap to apply to the rotated size.

### M4. The wallpaper cache trusts half-written files (confirmed)

`render` reuses any existing `<stem>.jpg|png` (`wallpaper.rs:24-29`), but `savev`
writes straight to that final name (`:71-76`). A crash or kill mid-save leaves a
truncated image that every later Apply reuses until the palette or settings change.
The window and the watcher can also render the same stem at once.

**Fix:** save to `<stem>.tmp.<ext>` and rename into place.

### M5. Blocking work on the GTK main thread (confirmed, impact likely small)

`run_busy` moves Apply and Reset off the main thread. These still run on it:

- `offer_restart` → `desktop::running_apps()` (`ui/window.rs:1000`): two synchronous
  D-Bus calls with the default 25 s timeout (`desktop.rs:198`, `-1`), plus a full
  `/proc` scan per browser (`restart::is_running`, `:995`). If the Shell extension is
  slow or stuck, the window freezes.
- `update_login_screen` (`ui/window.rs:854-881`): spawns `flatpak ps`
  (`login_screen.rs:67`), copies the lock image (can be tens of MB) and writes the
  key file.
- `refresh_installed` is "called again whenever the window comes back", and runs
  `fish --version` (`ui/extras.rs:348` → `desktop.rs:436`) and `AppInfo::all()` each
  time.
- `refresh_login` → `palette_name` re-parses the whole palette catalog (200 TOML
  files plus user palettes) on every Apply (`login_screen.rs:37`), even though the
  window already holds `palettes`.

**Fix:** run `running_apps`, `is_running` and the login-screen preparation through
`gio::spawn_blocking` (or use async `call` with a 2-3 s timeout). Cache
`fish_installed()` (`find_program_in_path("fish")` is enough). Look up names in
`self.palettes`.

### M6. The Shell extension, the riskiest code, has no CI check

The 1.5.3 fix ("locking the screen … could leave the desktop unresponsive") came from
the embedded JS in `crates/recoolor-core/src/apps/shell_fade.rs`. That JS is a Rust
string, so no tool ever parses it. CI also builds on `fedora:44`
(`.github/workflows/ci.yml:12`), while you run GNOME 51 on Fedora 45.

**Fix:**
- Add a test that writes `EXTENSION` to a temp file. In CI, run
  `gjs -m --check` on it, or `node --check` after replacing the `gi://` and
  `resource://` imports.
- Move the JS to `crates/recoolor-core/src/apps/shell_fade/extension.js` with
  `include_str!`, so editors and linters see it.
- Add a `fedora:45` job, or a matrix with both versions.

---

## Low

- **L1. A newer `state.toml` blocks Reset on an older Recoolor.** `State` has
  `deny_unknown_fields` (`crates/recoolor-core/src/state.rs:17`). After a downgrade,
  or a dev build next to the RPM, `State::load` fails and `set_extras` "leaves the
  extras alone", Reset included. Consider ignoring unknown keys but keeping them on save.
- **L2. Wallpaper cache names are not stable across toolchains.** `output_stem` hashes
  with `DefaultHasher` and the `Debug` format of the colors
  (`crates/recoolor-core/src/wallpaper.rs:49-58`). Neither is guaranteed stable, so a
  Rust update can change every name, re-tint every wallpaper and prune the old ones.
  Use a fixed hash (FNV or SipHash with fixed keys) over the hex colors.
- **L3. GDM Settings key file escaping.** `prefill` escapes `'` but not `\` in GVariant
  text (`crates/recoolor/src/login_screen.rs:106-123`), and icon or cursor theme names
  (folder names) are not escaped at all. Use `glib::Variant::from(value).print(false)`.
- **L4. `QT_QPA_PLATFORMTHEME=kde` is set for every Flatpak** (`flatpak.rs:51`), not only
  KDE-runtime apps. Qt apps on other runtimes then look for a `kde` platform plugin and
  log a warning. Per-app overrides for the installed KDE-runtime apps would be tighter.
- **L5. Log rotation** (`crates/recoolor/src/log.rs:62-72`) reads and rewrites the file
  without a lock, so lines from the watcher and the window can be lost while it rotates.
  The docs also say errors go to "the journal", but only the watcher's stderr does.
- **L6. The extension's D-Bus methods accept any caller,** also while the screen is
  locked (`session-modes` includes `unlock-dialog`). Any process in the session can keep
  calling `Freeze` or `RestartApps`. It runs as the same user, so this is not a
  privilege boundary, but you could refuse `RestartApps` while
  `Main.sessionMode.isLocked`.
- **L7. `set_config_arg` uses `line.replacen(old, arg, 1)`**
  (`crates/recoolor-core/src/apps/ohmyposh.rs:49`). That replaces the first match
  anywhere on the line, which may not be the `--config` value. Replace the exact span
  that `config_arg` found.
- **L8. `shell::login_background` deletes the old copy before the new copy succeeds**
  (`crates/recoolor-core/src/shell.rs`, `login_background`). Copy to a temp file, then
  rename.
- **L9. `Lut::new` takes about 36k OKLab round-trips** per palette, tint and strength,
  and Apply builds it again for each image even when light and dark share a palette.
  This is small next to decoding the image, so only worth it if you profile again.

---

## Docs and plan drift

- **PLAN.md is not in this repo.** The history starts at PR #21; the roadmap merged as
  PR #13 lived in the old `pinta` repo. Against that roadmap:
  - Done: 1 (review fixes), 2 (wallpapers, plus lock and login screen), 4 (adw-gtk3),
    5 (GtkSourceView, Ptyxis, Thunderbird).
  - Not started: 3 (palette from wallpaper), 6 (palette editor and base16 import),
    7 (COPR).
  - Added outside the plan: 179 Gogh palettes, favorites, Firefox, VS Code, fish and
    oh-my-posh, bash, btop, yazi, Spotifast, KDE/Qt, Restart Apps, GDM Settings
    integration, Screen Fade, Blur my Shell and SearchLightNG.

  Suggest a short `docs/ROADMAP.md` so the next review has something to check against.
- `docs/HOW-RECOOLOR-WORKS.md`:
  - It says "as of version 1.5.0"; the code is 1.5.3.
  - "Records before it changes" is wrong (see H3).
  - "Atomic writes" does not hold for wallpapers (M4), the login background (L8), the
    log, or the VS Code theme rewrite (`targets.rs`, `std::fs::write` in `set_vscode`).
  - "Logged to … the journal" is only partly true (L5).
- `crates/recoolor-core/src/apps/shell_fade.rs:7` says the still fades "after five
  seconds at most"; `MAX_FREEZE_MS` is 3000 (and the docs say 3 s).
- `crates/recoolor/src/ui/window.rs:43-44`: the doc comment on `apply` ends with an
  orphaned line ("that only the narrow layout shows.").
- `crates/recoolor/src/desktop.rs:324-327`: the `screen_fade` doc comment sits on
  `running_apps` instead.
- `crates/recoolor-core/src/apps/mod.rs:1` lists only some of the apps now in the module.
- The GTK parse test (`ui/window.rs:1137`) covers only the GTK4 CSS. The GTK3 and adw-gtk3
  stylesheets go through the same parser and could be checked in the same loop.

---

## What is in good shape

- Ownership markers and the backup-then-rename scheme are consistent across every
  target. `write_owned` and `remove_owned` never touch a file that is not Recoolor's,
  and the tests cover that for each app.
- No shell interpolation: every `Command` takes an argument array. The one `fish -c`
  only sees constants and a number.
- Palette ids are validated, and names are escaped wherever they reach XML.
- The systemd unit quotes `ExecStart`, and the lock image path reaches the extension's
  CSS as a URI, so quotes cannot break out of the string.
- Apply and Reset run off the main thread with `app.hold()`, so closing the window
  cannot cut them off halfway.
- The tests run `reset_restores_inner` on the in-memory GSettings backend, which is
  the right way to test settings changes.

## Suggested order

1. H1 and H4, because users lose windows.
2. H3, so Reset cannot lose the user's settings.
3. H2.
4. M1 and M2 together: both are in `write_atomic` and the locking.
5. M3 and M4 together: both are in `wallpaper::render`.
6. M6, so the next lock-screen regression is caught in CI.
7. The rest as you touch the code.
