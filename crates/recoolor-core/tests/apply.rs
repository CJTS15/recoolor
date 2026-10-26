use std::fs;
use std::path::Path;

use recoolor_core::palette::bundled;
use recoolor_core::{ApplyError, CssState, GtkConfig, Palette, RestoreOutcome, Toolkit};

fn palette(id: &str) -> Palette {
    bundled().into_iter().find(|p| p.id == id).unwrap()
}

fn write(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

fn states(cfg: &GtkConfig) -> Vec<CssState> {
    cfg.status().unwrap().into_iter().map(|s| s.css).collect()
}

#[test]
fn apply_into_empty_config_then_restore_removes_files() {
    let home = tempfile::tempdir().unwrap();
    let cfg = GtkConfig::new(home.path());

    cfg.apply(&palette("catppuccin-mocha")).unwrap();
    let id = CssState::Recoolor("catppuccin-mocha".into());
    assert_eq!(states(&cfg), [id.clone(), id]);

    let outcomes = cfg.restore().unwrap();
    assert!(outcomes.iter().all(|(_, o)| *o == RestoreOutcome::Removed));
    assert_eq!(states(&cfg), [CssState::Missing, CssState::Missing]);
}

#[test]
fn existing_css_is_backed_up_once_and_restored() {
    let home = tempfile::tempdir().unwrap();
    let cfg = GtkConfig::new(home.path());
    let mine = "/* my tweaks */\nwindow { padding: 2px; }\n";
    write(&cfg.css_path(Toolkit::Gtk4), mine);

    cfg.apply(&palette("dracula")).unwrap();
    // A second apply must not back up Recoolor's own file over the user's.
    cfg.apply(&palette("tokyo-night")).unwrap();
    assert_eq!(
        fs::read_to_string(cfg.backup_path(Toolkit::Gtk4)).unwrap(),
        mine
    );
    assert_eq!(states(&cfg)[1], CssState::Recoolor("tokyo-night".into()));

    let outcomes = cfg.restore().unwrap();
    assert_eq!(
        outcomes,
        [
            (Toolkit::Gtk3, RestoreOutcome::Removed),
            (Toolkit::Gtk4, RestoreOutcome::Restored)
        ]
    );
    assert_eq!(
        fs::read_to_string(cfg.css_path(Toolkit::Gtk4)).unwrap(),
        mine
    );
    assert!(!cfg.backup_path(Toolkit::Gtk4).exists());
    assert!(!cfg.css_path(Toolkit::Gtk3).exists());
}

#[test]
fn conflicting_backup_aborts_without_changes() {
    let home = tempfile::tempdir().unwrap();
    let cfg = GtkConfig::new(home.path());
    write(&cfg.css_path(Toolkit::Gtk3), "/* gtk3 original */");
    write(&cfg.css_path(Toolkit::Gtk4), "/* new hand edits */");
    write(&cfg.backup_path(Toolkit::Gtk4), "/* old original */");

    let err = cfg.apply(&palette("dracula")).unwrap_err();
    assert!(matches!(err, ApplyError::BackupExists { .. }));
    // GTK3 was checked first but must not have been touched.
    assert_eq!(
        fs::read_to_string(cfg.css_path(Toolkit::Gtk3)).unwrap(),
        "/* gtk3 original */"
    );
    assert!(!cfg.backup_path(Toolkit::Gtk3).exists());
    assert_eq!(
        fs::read_to_string(cfg.css_path(Toolkit::Gtk4)).unwrap(),
        "/* new hand edits */"
    );
}

#[test]
fn restore_leaves_foreign_css_alone() {
    let home = tempfile::tempdir().unwrap();
    let cfg = GtkConfig::new(home.path());
    write(&cfg.css_path(Toolkit::Gtk4), "/* someone else's */");
    write(&cfg.backup_path(Toolkit::Gtk4), "/* backup */");

    let outcomes = cfg.restore().unwrap();
    assert_eq!(outcomes[1], (Toolkit::Gtk4, RestoreOutcome::LeftForeign));
    assert_eq!(
        fs::read_to_string(cfg.css_path(Toolkit::Gtk4)).unwrap(),
        "/* someone else's */"
    );
    assert!(cfg.backup_path(Toolkit::Gtk4).exists());
}

#[cfg(unix)]
#[test]
fn symlinked_css_is_backed_up_as_a_symlink() {
    let home = tempfile::tempdir().unwrap();
    let cfg = GtkConfig::new(home.path());
    let theme = home.path().join("theme.css");
    write(&theme, "/* installed theme */");
    let link = cfg.css_path(Toolkit::Gtk4);
    fs::create_dir_all(link.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&theme, &link).unwrap();

    cfg.apply(&palette("catppuccin-latte")).unwrap();
    assert!(
        fs::symlink_metadata(cfg.backup_path(Toolkit::Gtk4))
            .unwrap()
            .is_symlink()
    );
    assert_eq!(fs::read_to_string(&theme).unwrap(), "/* installed theme */");

    cfg.restore().unwrap();
    assert!(fs::symlink_metadata(&link).unwrap().is_symlink());
}

#[test]
fn pair_marks_gtk4_with_both_ids_and_gtk3_with_the_current_one() {
    let home = tempfile::tempdir().unwrap();
    let cfg = GtkConfig::new(home.path());
    let (latte, mocha) = (palette("catppuccin-latte"), palette("catppuccin-mocha"));

    cfg.apply_pair(&latte, &mocha, true).unwrap();
    let pair = CssState::RecoolorPair {
        light: "catppuccin-latte".into(),
        dark: "catppuccin-mocha".into(),
    };
    assert_eq!(
        states(&cfg),
        [CssState::Recoolor("catppuccin-mocha".into()), pair.clone()]
    );

    cfg.apply_pair(&latte, &mocha, false).unwrap();
    assert_eq!(
        states(&cfg),
        [CssState::Recoolor("catppuccin-latte".into()), pair]
    );

    let outcomes = cfg.restore().unwrap();
    assert!(outcomes.iter().all(|(_, o)| *o == RestoreOutcome::Removed));
}

#[test]
fn adw_gtk3_writes_named_colors_only() {
    let home = tempfile::tempdir().unwrap();
    let cfg = GtkConfig::new(home.path()).with_adw_gtk3(true);
    cfg.apply(&palette("catppuccin-mocha")).unwrap();
    let gtk3 = fs::read_to_string(cfg.css_path(Toolkit::Gtk3)).unwrap();
    assert!(gtk3.contains("@define-color window_bg_color #1e1e2e;"));
    assert!(gtk3.contains("@define-color accent_bg_color #cba6f7;"));
    assert!(!gtk3.contains("button"));
    assert_eq!(
        states(&cfg)[0],
        CssState::Recoolor("catppuccin-mocha".into())
    );
}
