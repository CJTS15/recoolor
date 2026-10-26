//! The libadwaita window.

mod accent;
mod extras;
mod palette_card;
mod preview;
mod window;

use std::process::ExitCode;

use adw::prelude::*;
use gtk::gio;

pub const APP_ID: &str = "io.github.cjts15.Recoolor";
const REPOSITORY: &str = "https://github.com/CJTS15/recoolor";

pub fn run() -> ExitCode {
    crate::log::capture_glib_warnings();
    crate::log::info("opened the window");
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(|app| match app.active_window() {
        Some(window) => window.present(),
        None => window::build(app).present(),
    });
    let quit = gio::ActionEntry::builder("quit")
        .activate(|app: &adw::Application, _, _| app.quit())
        .build();
    let about = gio::ActionEntry::builder("about")
        .activate(|app: &adw::Application, _, _| {
            adw::AboutDialog::builder()
                .application_name("Recoolor")
                .application_icon(APP_ID)
                .version(env!("CARGO_PKG_VERSION"))
                .developer_name("CJ Sabijon")
                .comments("Recolor your whole GNOME desktop from one palette.")
                .license_type(gtk::License::Gpl30)
                .website(REPOSITORY)
                .issue_url(format!("{REPOSITORY}/issues"))
                .build()
                .present(app.active_window().as_ref());
        })
        .build();
    app.add_action_entries([quit, about]);
    app.set_accels_for_action("app.quit", &["<Control>q"]);
    app.set_accels_for_action("window.close", &["<Control>w"]);

    // The command line owns argv; GTK must not try to parse it.
    let code = app.run_with_args::<&str>(&[]);
    ExitCode::from(code.get())
}
