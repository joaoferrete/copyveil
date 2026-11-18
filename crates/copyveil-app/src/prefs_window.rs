//! The settings window: General, Detection, Pins and Shortcuts.
//!
//! Every value comes from and goes to the daemon (`Settings1`), which
//! validates it; a refusal shows as a toast and the row goes back to the
//! daemon's value. The rules that never change are shown as locked rows.
//! Shortcuts live in the Shell extension's own settings, read directly.

use crate::daemon::{Daemon, EXTENSION_UUID, SettingChange, SettingsValues};
use crate::i18n::tr;
use crate::prefs::{self, Limits};
use adw::prelude::*;
use copyveil_dbus::ItemSummary;
use gettextrs::gettext;
use gtk::{gio, glib};
use std::cell::Cell;
use std::rc::Rc;

const EXTENSION_SCHEMA: &str = "org.gnome.shell.extensions.copyveil";

/// Opens the settings in a window of their own and returns it.
#[must_use]
pub fn present(daemon: Daemon) -> adw::PreferencesDialog {
    let dialog = adw::PreferencesDialog::new();
    dialog.set_title(&gettext("Copyveil Settings"));
    dialog.set_search_enabled(false);
    dialog.set_content_width(680);
    dialog.set_content_height(700);
    // No parent: libadwaita shows the dialog as a separate window.
    dialog.present(None::<&gtk::Widget>);
    let filled = dialog.clone();
    glib::spawn_future_local(async move {
        let (Some(limits), Some(values), Some(detectors)) = (
            daemon.limits().await,
            daemon.settings().await,
            daemon.detectors().await,
        ) else {
            let page = adw::PreferencesPage::new();
            let group = adw::PreferencesGroup::builder()
                .title(gettext("Copyveil isn’t running"))
                .description(gettext("Start it with “systemctl --user start copyveil-daemon”, then open Settings again."))
                .build();
            page.add(&group);
            dialog.add(&page);
            return;
        };
        let limits = Limits::from_dbus(limits);
        dialog.add(&general_page(&dialog, &daemon, &limits, &values));
        dialog.add(&detection_page(&dialog, &daemon, &detectors, &values));
        dialog.add(&pins_page(&dialog, &daemon).await);
        dialog.add(&shortcuts_page());
    });
    filled
}

fn general_page(
    dialog: &adw::PreferencesDialog,
    daemon: &Daemon,
    limits: &Limits,
    values: &SettingsValues,
) -> adw::PreferencesPage {
    let page = adw::PreferencesPage::builder()
        .title(gettext("General"))
        .icon_name("preferences-system-symbolic")
        .build();

    let history = adw::PreferencesGroup::builder()
        .title(gettext("History"))
        .description(gettext(
            "How long sensitive Items stay, and how much the History keeps.",
        ))
        .build();
    history.add(&spin_row(
        dialog,
        daemon,
        &gettext("Secrets clear after"),
        &gettext("Seconds. Items from a password manager."),
        &limits.secret_seconds,
        5,
        values.secret_expiry,
        SettingChange::SecretExpiry,
    ));
    history.add(&spin_row(
        dialog,
        daemon,
        &gettext("Suspected Items clear after"),
        &gettext("Minutes. Anything that looks like a key or token."),
        &limits.suspected_minutes,
        1,
        prefs::minutes(values.suspected_expiry),
        |minutes| SettingChange::SuspectedExpiry(prefs::seconds_from_minutes(minutes)),
    ));
    history.add(&spin_row(
        dialog,
        daemon,
        &gettext("Keep up to"),
        &gettext("Plain Items. Pins don’t count."),
        &limits.plain_items,
        50,
        values.max_plain_items,
        SettingChange::MaxPlainItems,
    ));
    history.add(&memory_only_row(dialog, daemon, values.memory_only));
    page.add(&history);

    let fixed = adw::PreferencesGroup::builder()
        .title(gettext("Always on"))
        .description(gettext(
            "These protect your secrets and can’t be turned off.",
        ))
        .build();
    for (title, subtitle) in prefs::FIXED.map(|(t, s)| (tr(t), tr(s))) {
        let row = adw::ActionRow::builder()
            .title(title)
            .subtitle(subtitle)
            .activatable(false)
            .build();
        let lock = gtk::Image::from_icon_name("changes-prevent-symbolic");
        lock.set_tooltip_text(Some(&gettext("Fixed")));
        row.add_suffix(&lock);
        fixed.add(&row);
    }
    page.add(&fixed);
    page
}

#[expect(
    clippy::too_many_arguments,
    reason = "one call per setting reads better than a builder here"
)]
fn spin_row(
    dialog: &adw::PreferencesDialog,
    daemon: &Daemon,
    title: &str,
    subtitle: &str,
    range: &std::ops::RangeInclusive<u32>,
    step: u32,
    value: u32,
    change: impl Fn(u32) -> SettingChange + 'static,
) -> adw::SpinRow {
    let row = adw::SpinRow::with_range(
        f64::from(*range.start()),
        f64::from(*range.end()),
        f64::from(step),
    );
    row.set_title(title);
    row.set_subtitle(subtitle);
    row.set_value(f64::from(value));
    let (dialog, daemon) = (dialog.clone(), daemon.clone());
    let last = Rc::new(Cell::new(value));
    row.connect_value_notify(move |row| {
        let new = spin_value(row);
        if new == last.get() {
            return;
        }
        let (dialog, daemon, last, row, setting) = (
            dialog.clone(),
            daemon.clone(),
            Rc::clone(&last),
            row.clone(),
            change(new),
        );
        glib::spawn_future_local(async move {
            match daemon.change(setting).await {
                Ok(()) => last.set(new),
                Err(message) => {
                    dialog.add_toast(adw::Toast::new(&message));
                    row.set_value(f64::from(last.get()));
                }
            }
        });
    });
    row
}

fn spin_value(row: &adw::SpinRow) -> u32 {
    // The row's range is the daemon's limit, so this is a small whole number.
    let value = row.value().round().max(0.0);
    format!("{value:.0}").parse().unwrap_or(0)
}

fn memory_only_row(dialog: &adw::PreferencesDialog, daemon: &Daemon, on: bool) -> adw::SwitchRow {
    let row = adw::SwitchRow::builder()
        .title(gettext("Keep History in memory only"))
        .subtitle(gettext(
            "Nothing is saved to disk. The History clears when you log out.",
        ))
        .active(on)
        .build();
    let (dialog, daemon) = (dialog.clone(), daemon.clone());
    let settling = Rc::new(Cell::new(false));
    row.connect_active_notify(move |row| {
        if settling.get() {
            return;
        }
        let wanted = row.is_active();
        let (dialog, daemon, row, settling) = (
            dialog.clone(),
            daemon.clone(),
            row.clone(),
            Rc::clone(&settling),
        );
        glib::spawn_future_local(async move {
            if wanted && !confirm_memory_only(&dialog).await {
                settling.set(true);
                row.set_active(false);
                settling.set(false);
                return;
            }
            if let Err(message) = daemon.change(SettingChange::MemoryOnly(wanted)).await {
                dialog.add_toast(adw::Toast::new(&message));
                settling.set(true);
                row.set_active(!wanted);
                settling.set(false);
            }
        });
    });
    row
}

async fn confirm_memory_only(dialog: &adw::PreferencesDialog) -> bool {
    let (heading, body) = (
        tr(prefs::MEMORY_ONLY_CONFIRM.0),
        tr(prefs::MEMORY_ONLY_CONFIRM.1),
    );
    let alert = adw::AlertDialog::new(Some(&heading), Some(&body));
    alert.add_responses(&[
        ("cancel", &gettext("Cancel")),
        ("erase", &gettext("Erase and Keep in Memory")),
    ]);
    alert.set_response_appearance("erase", adw::ResponseAppearance::Destructive);
    alert.set_default_response(Some("cancel"));
    alert.set_close_response("cancel");
    alert.choose_future(Some(dialog)).await == "erase"
}

fn detection_page(
    dialog: &adw::PreferencesDialog,
    daemon: &Daemon,
    detectors: &[(String, String, String)],
    values: &SettingsValues,
) -> adw::PreferencesPage {
    let page = adw::PreferencesPage::builder()
        .title(gettext("Detection"))
        .icon_name("security-medium-symbolic")
        .build();
    let group = adw::PreferencesGroup::builder()
        .title(gettext("Flag as Suspected"))
        .description(gettext("What Copyveil looks for in what you copy. A flagged Item is masked, kept in memory only and cleared on its own."))
        .build();
    let disabled = Rc::new(std::cell::RefCell::new(values.disabled_detectors.clone()));
    for (name, title, description) in detectors {
        let row = adw::SwitchRow::builder()
            .title(title.as_str())
            .subtitle(description.as_str())
            .active(!values.disabled_detectors.contains(name))
            .build();
        let (dialog, daemon, disabled, name) = (
            dialog.clone(),
            daemon.clone(),
            Rc::clone(&disabled),
            name.clone(),
        );
        row.connect_active_notify(move |row| {
            {
                let mut list = disabled.borrow_mut();
                list.retain(|n| *n != name);
                if !row.is_active() {
                    list.push(name.clone());
                }
            }
            let names = disabled.borrow().clone();
            let (dialog, daemon) = (dialog.clone(), daemon.clone());
            glib::spawn_future_local(async move {
                if let Err(message) = daemon.change(SettingChange::DisabledDetectors(names)).await {
                    dialog.add_toast(adw::Toast::new(&message));
                }
            });
        });
        group.add(&row);
    }
    page.add(&group);
    page
}

async fn pins_page(dialog: &adw::PreferencesDialog, daemon: &Daemon) -> adw::PreferencesPage {
    let page = adw::PreferencesPage::builder()
        .title(gettext("Pins"))
        .icon_name("view-pin-symbolic")
        .build();
    let group = adw::PreferencesGroup::builder()
        .title(gettext("Pin slots"))
        .description(gettext("Super+Alt+number pastes a Pin slot without opening the History. Only Plain Items can be pinned, so it can never paste a secret."))
        .build();
    page.add(&group);
    fill_pins(&group, dialog, daemon).await;
    page
}

/// Lists the Pins in slot order, each with buttons to move it.
async fn fill_pins(
    group: &adw::PreferencesGroup,
    dialog: &adw::PreferencesDialog,
    daemon: &Daemon,
) {
    let mut pins: Vec<ItemSummary> = daemon
        .list()
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|item| item.pin_slot > 0)
        .collect();
    pins.sort_by_key(|item| item.pin_slot);
    let rows = Rc::new(std::cell::RefCell::new(Vec::<adw::ActionRow>::new()));
    if pins.is_empty() {
        let row = adw::ActionRow::builder()
            .title(gettext("No Pins yet"))
            .subtitle(gettext("Pin a Plain Item with Ctrl+P in the History."))
            .build();
        group.add(&row);
        rows.borrow_mut().push(row);
        return;
    }
    let count = u32::try_from(pins.len()).unwrap_or(u32::MAX);
    for item in &pins {
        let title = if item.preview.is_empty() {
            gettext("Image · {mime}").replace("{mime}", &item.mime)
        } else {
            item.preview.lines().next().unwrap_or_default().to_owned()
        };
        let row = adw::ActionRow::builder()
            .title(glib::markup_escape_text(&title).as_str())
            .subtitle(format!("Super+Alt+{}", item.pin_slot).as_str())
            .build();
        for (icon, label, delta) in [
            ("go-up-symbolic", gettext("Move up"), -1),
            ("go-down-symbolic", gettext("Move down"), 1),
        ] {
            let button = gtk::Button::from_icon_name(icon);
            button.set_tooltip_text(Some(&label));
            button.set_valign(gtk::Align::Center);
            button.add_css_class("flat");
            let target = prefs::nudged_slot(item.pin_slot, delta, count);
            button.set_sensitive(target.is_some());
            let (group, dialog, daemon, rows, id) = (
                group.clone(),
                dialog.clone(),
                daemon.clone(),
                Rc::clone(&rows),
                item.id,
            );
            button.connect_clicked(move |_| {
                let Some(slot) = target else { return };
                let (group, dialog, daemon, rows) = (
                    group.clone(),
                    dialog.clone(),
                    daemon.clone(),
                    Rc::clone(&rows),
                );
                glib::spawn_future_local(async move {
                    if daemon.move_pin(id, slot).await.is_none() {
                        dialog.add_toast(adw::Toast::new(&gettext("Couldn’t move the Pin.")));
                    }
                    for row in rows.borrow_mut().drain(..) {
                        group.remove(&row);
                    }
                    Box::pin(fill_pins(&group, &dialog, &daemon)).await;
                });
            });
            row.add_suffix(&button);
        }
        group.add(&row);
        rows.borrow_mut().push(row);
    }
}

fn shortcuts_page() -> adw::PreferencesPage {
    let page = adw::PreferencesPage::builder()
        .title(gettext("Shortcuts"))
        .icon_name("preferences-desktop-keyboard-shortcuts-symbolic")
        .build();
    let group = adw::PreferencesGroup::builder()
        .title(gettext("Shortcuts"))
        .description(gettext(
            "Global shortcuts are provided by the Copyveil GNOME Shell extension.",
        ))
        .build();
    page.add(&group);

    let Some(settings) = extension_settings() else {
        group.add(
            &adw::ActionRow::builder()
                .title(gettext("The GNOME Shell extension isn’t installed"))
                .subtitle(gettext(
                    "Install it with “just ext-install” to capture copies and use shortcuts.",
                ))
                .build(),
        );
        return page;
    };
    let popup = settings.strv("popup-shortcut");
    let current = popup
        .first()
        .map(|a| prefs::accelerator_label(a))
        .unwrap_or_default();
    group.add(
        &adw::ActionRow::builder()
            .title(gettext("Open the History"))
            .subtitle(current.as_str())
            .build(),
    );
    let super_v = adw::SwitchRow::builder()
        .title(gettext("Use Super+V"))
        .subtitle(gettext(
            "GNOME opens notifications with Super+V; they stay on Super+M.",
        ))
        .active(settings.boolean("take-super-v"))
        .build();
    let settings_for_switch = settings.clone();
    super_v.connect_active_notify(move |row| {
        let take = row.is_active();
        let shortcut: &[&str] = if take {
            &["<Super>v"]
        } else {
            &["<Super><Shift>v"]
        };
        let _ = settings_for_switch.set_strv("popup-shortcut", shortcut);
        let _ = settings_for_switch.set_boolean("take-super-v", take);
    });
    group.add(&super_v);
    group.add(
        &adw::ActionRow::builder()
            .title(gettext("Paste a Pin slot"))
            .subtitle(
                prefs::accelerator_label(
                    &settings
                        .strv("paste-pin-1")
                        .first()
                        .map(ToString::to_string)
                        .unwrap_or_default(),
                )
                .replace('1', "1…9")
                .as_str(),
            )
            .build(),
    );
    page
}

/// The extension's settings, from wherever GNOME Shell installed it.
pub(crate) fn extension_settings() -> Option<gio::Settings> {
    let mut dirs = vec![glib::user_data_dir()];
    dirs.extend(glib::system_data_dirs());
    let default = gio::SettingsSchemaSource::default();
    dirs.iter().find_map(|dir| {
        let schemas = dir
            .join("gnome-shell/extensions")
            .join(EXTENSION_UUID)
            .join("schemas");
        let source =
            gio::SettingsSchemaSource::from_directory(&schemas, default.as_ref(), false).ok()?;
        let schema = source.lookup(EXTENSION_SCHEMA, false)?;
        Some(gio::Settings::new_full(
            &schema,
            None::<&gio::SettingsBackend>,
            None,
        ))
    })
}
