//! Sample widgets, so a palette shows on more than the grid. They do nothing. They sit
//! in a "Preview" group, collapsed until its header button opens it, so they don't
//! push the settings down.

use adw::prelude::*;

pub fn build() -> gtk::Widget {
    let rows = adw::PreferencesGroup::new();
    let row = adw::ActionRow::builder()
        .title("List row")
        .subtitle("With a subtitle")
        .build();
    let switch = gtk::Switch::builder()
        .active(true)
        .valign(gtk::Align::Center)
        .build();
    row.add_suffix(&switch);
    row.set_activatable_widget(Some(&switch));
    rows.add(&row);
    rows.add(&adw::EntryRow::builder().title("Entry").build());

    // Each as wide as its label, so the row still fits a narrow window.
    let buttons = gtk::Box::builder().spacing(6).build();
    for (label, class) in [
        ("Suggested", "suggested-action"),
        ("Button", ""),
        ("Destructive", "destructive-action"),
    ] {
        let button = gtk::Button::builder().label(label).hexpand(true).build();
        if !class.is_empty() {
            button.add_css_class(class);
        }
        buttons.append(&button);
    }

    let progress = gtk::ProgressBar::builder().fraction(0.6).build();

    let card_text = gtk::Label::builder()
        .label("A card, as used for boxed content")
        .wrap(true)
        .margin_top(18)
        .margin_bottom(18)
        .margin_start(12)
        .margin_end(12)
        .build();
    let card = gtk::Box::builder().css_classes(["card"]).build();
    card.append(&card_text);

    let hint = gtk::Label::builder()
        .label("This is how GTK4 and libadwaita apps will look. GTK3 apps get most of it, but some widgets keep their usual colors.")
        .wrap(true)
        .xalign(0.0)
        .css_classes(["caption", "dim-label"])
        .build();

    let page = gtk::Box::new(gtk::Orientation::Vertical, 12);
    page.append(&rows);
    page.append(&buttons);
    page.append(&progress);
    page.append(&card);
    page.append(&hint);

    // An accordion in the group's header, not an expander row: a row's padding
    // would make the samples too wide for a narrow window.
    let revealer = gtk::Revealer::builder().child(&page).build();
    let toggle = gtk::ToggleButton::builder()
        .icon_name("pan-down-symbolic")
        .tooltip_text("Show Preview")
        .valign(gtk::Align::Center)
        .css_classes(["flat"])
        .build();
    toggle.update_property(&[gtk::accessible::Property::Label("Show Preview")]);
    let shown = revealer.clone();
    toggle.connect_toggled(move |toggle| {
        let open = toggle.is_active();
        shown.set_reveal_child(open);
        toggle.set_icon_name(if open {
            "pan-up-symbolic"
        } else {
            "pan-down-symbolic"
        });
        toggle.set_tooltip_text(Some(if open { "Hide Preview" } else { "Show Preview" }));
    });
    let group = adw::PreferencesGroup::builder()
        .title("Preview")
        .description("How GTK apps look with the selected palette")
        .header_suffix(&toggle)
        .build();
    group.add(&revealer);
    group.upcast()
}
