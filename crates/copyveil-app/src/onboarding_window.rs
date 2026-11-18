//! The onboarding window: four steps, the last one a live test.
//!
//! The brand shows here more than anywhere else (`docs/design/README.md`).
//! The live test copies a synthetic token and waits for the daemon to list
//! it as a masked Suspected Item; it can't be skipped the first time.

use crate::daemon::Daemon;
use crate::i18n::tr;
use crate::onboarding::{self, Extension, Step};
use crate::row::{ItemRow, RowContext};
use adw::prelude::*;
use copyveil_dbus::ItemSummary;
use gettextrs::gettext;
use gtk::{gdk, glib};
use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

/// How long the live test waits for the copy to show up.
const LIVE_TEST_TIMEOUT: Duration = Duration::from_secs(10);
/// How long to let a second capture of the test token land before cleaning up.
const SETTLE: Duration = Duration::from_secs(1);

/// Where the "done" mark lives, from the environment.
#[must_use]
pub fn done_marker() -> Option<PathBuf> {
    onboarding::done_marker(
        std::env::var_os("XDG_STATE_HOME").map(PathBuf::from),
        std::env::var_os("HOME").map(PathBuf::from),
    )
}

/// Whether onboarding was completed before.
#[must_use]
pub fn is_done() -> bool {
    done_marker().is_some_and(|path| path.exists())
}

fn mark_done() {
    if let Some(path) = done_marker() {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, b"");
    }
}

struct Pages {
    stack: gtk::Stack,
    back: gtk::Button,
    next: gtk::Button,
    dots: Vec<gtk::Label>,
    step: Cell<Step>,
    passed: Cell<bool>,
    skippable: bool,
}

/// Opens the onboarding window for `app`; `on_done` runs when it finishes.
pub fn present(
    app: &adw::Application,
    daemon: &Daemon,
    on_done: impl Fn() + 'static,
) -> adw::ApplicationWindow {
    let stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::SlideLeftRight)
        .transition_duration(220)
        .vexpand(true)
        .build();
    let live_status = gtk::Box::new(gtk::Orientation::Vertical, 12);
    stack.add_named(&welcome_page(), Some("welcome"));
    stack.add_named(&capture_page(daemon), Some("capture"));
    stack.add_named(&storage_page(daemon), Some("storage"));

    let back = gtk::Button::with_label(&gettext("Back"));
    let next = gtk::Button::with_label(&gettext("Get Started"));
    next.add_css_class("suggested-action");
    next.add_css_class("pill");
    back.add_css_class("pill");
    let dots_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    dots_box.set_hexpand(true);
    dots_box.set_halign(gtk::Align::Center);
    let dots: Vec<gtk::Label> = Step::ALL
        .iter()
        .map(|_| {
            let dot = gtk::Label::new(Some("●"));
            dot.add_css_class("onboarding-dot");
            dots_box.append(&dot);
            dot
        })
        .collect();
    let footer = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    footer.set_margin_start(24);
    footer.set_margin_end(24);
    footer.set_margin_bottom(24);
    footer.set_margin_top(12);
    footer.append(&back);
    footer.append(&dots_box);
    footer.append(&next);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.append(&stack);
    content.append(&footer);
    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    header.set_show_title(false);
    header.set_decoration_layout(Some(":close"));
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&content));

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title(gettext("Welcome to Copyveil"))
        .default_width(640)
        .default_height(660)
        .content(&toolbar)
        .build();
    window.add_css_class("copyveil-onboarding");

    let pages = Rc::new(Pages {
        stack,
        back,
        next,
        dots,
        step: Cell::new(Step::Welcome),
        passed: Cell::new(false),
        skippable: onboarding::live_test_skippable(is_done()),
    });
    let live = live_test_page(daemon, &live_status, Rc::clone(&pages));
    pages.stack.add_named(&live, Some("live"));
    update(&pages);

    {
        let pages = Rc::clone(&pages);
        pages.back.clone().connect_clicked(move |_| {
            if let Some(previous) = pages.step.get().previous() {
                pages.step.set(previous);
                update(&pages);
            }
        });
    }
    {
        let (pages, window) = (Rc::clone(&pages), window.clone());
        let on_done = Rc::new(on_done);
        pages.next.clone().connect_clicked(move |_| {
            if let Some(next) = pages.step.get().next() {
                pages.step.set(next);
                update(&pages);
            } else {
                mark_done();
                window.close();
                on_done();
            }
        });
    }
    window.present();
    window
}

fn update(pages: &Pages) {
    let step = pages.step.get();
    pages.stack.set_visible_child_name(match step {
        Step::Welcome => "welcome",
        Step::Capture => "capture",
        Step::Storage => "storage",
        Step::LiveTest => "live",
    });
    pages.back.set_visible(step.previous().is_some());
    let last = step.next().is_none();
    pages.next.set_label(&match step {
        Step::Welcome => gettext("Get Started"),
        Step::LiveTest => gettext("Finish"),
        _ => gettext("Next"),
    });
    pages
        .next
        .set_sensitive(!last || pages.passed.get() || pages.skippable);
    for (index, dot) in pages.dots.iter().enumerate() {
        if index == step.index() {
            dot.add_css_class("current");
        } else {
            dot.remove_css_class("current");
        }
    }
}

fn page(title: &str, body: &str) -> gtk::Box {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 12);
    page.set_margin_start(48);
    page.set_margin_end(48);
    page.set_margin_top(12);
    page.set_valign(gtk::Align::Center);
    let heading = gtk::Label::new(Some(title));
    heading.add_css_class("title-1");
    heading.set_wrap(true);
    heading.set_justify(gtk::Justification::Center);
    let text = gtk::Label::new(Some(body));
    text.set_wrap(true);
    text.set_justify(gtk::Justification::Center);
    text.add_css_class("dim-label");
    page.append(&heading);
    page.append(&text);
    page
}

fn sample(
    id: u64,
    sensitivity: &str,
    reason: &str,
    expires_in_ms: u64,
    preview: &str,
) -> ItemSummary {
    ItemSummary {
        id,
        kind: "text".into(),
        mime: String::new(),
        sensitivity: sensitivity.into(),
        reason: reason.into(),
        pin_slot: 0,
        age_ms: 0,
        expires_in_ms,
        size: 20,
        preview: preview.into(),
    }
}

fn welcome_page() -> gtk::Box {
    let page = page(
        &gettext("Welcome to Copyveil"),
        &gettext("Remembers what you copy. Forgets your secrets."),
    );
    let icon = gtk::Image::from_icon_name("io.github.joaoferrete.Copyveil");
    icon.set_pixel_size(96);
    page.prepend(&icon);

    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    list.add_css_class("onboarding-sample");
    list.set_selection_mode(gtk::SelectionMode::None);
    list.set_margin_top(12);
    let context = RowContext::now();
    for item in [
        sample(1, "secret", "From password manager", 30_000, ""),
        sample(2, "suspected", "Looks like a GitHub token", 900_000, ""),
        sample(3, "plain", "", 0, "docker compose up -d"),
    ] {
        let row = ItemRow::new(&item, None, &context);
        row.widget.set_activatable(false);
        list.append(&row.widget);
    }
    page.append(&list);
    let legend = gtk::Label::new(Some(&gettext(
        "Secrets from a password manager and anything that looks like a key stay masked, in memory only, and clear on their own. Everything else is encrypted on disk.",
    )));
    legend.set_wrap(true);
    legend.set_justify(gtk::Justification::Center);
    legend.add_css_class("caption");
    legend.add_css_class("dim-label");
    page.append(&legend);
    page
}

fn capture_page(daemon: &Daemon) -> gtk::Box {
    let page = page(&gettext("Where copies come from"), "");
    let status = adw::ActionRow::new();
    let action = gtk::Button::with_label(&gettext("Check Again"));
    action.set_valign(gtk::Align::Center);
    status.add_suffix(&action);
    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk::SelectionMode::None);
    list.append(&status);

    let super_v = adw::SwitchRow::builder()
        .title(gettext("Open the History with Super+V"))
        .subtitle(gettext("GNOME uses Super+V for notifications; they stay on Super+M. Otherwise it’s Super+Shift+V."))
        .build();
    if let Some(settings) = crate::prefs_window::extension_settings() {
        super_v.set_active(settings.boolean("take-super-v"));
        super_v.connect_active_notify(move |row| {
            let take = row.is_active();
            let shortcut: &[&str] = if take {
                &["<Super>v"]
            } else {
                &["<Super><Shift>v"]
            };
            let _ = settings.set_strv("popup-shortcut", shortcut);
            let _ = settings.set_boolean("take-super-v", take);
        });
    } else {
        super_v.set_sensitive(false);
    }
    list.append(&super_v);
    page.append(&list);

    let refresh = {
        let (daemon, status, action) = (daemon.clone(), status.clone(), action.clone());
        move || {
            let (daemon, status, action) = (daemon.clone(), status.clone(), action.clone());
            glib::spawn_future_local(async move {
                let state = match daemon.extension_state().await {
                    None => Extension::NoGnomeShell,
                    Some(state) => Extension::from_state(state),
                };
                let (title, body) = state.describe();
                let (title, body) = (tr(title), tr(body));
                status.set_title(&title);
                status.set_subtitle(&body);
                action.set_label(&if state == Extension::Off {
                    gettext("Turn On")
                } else {
                    gettext("Check Again")
                });
                action.set_visible(state != Extension::On);
            });
        }
    };
    refresh();
    let refresh = Rc::new(refresh);
    {
        let (daemon, refresh) = (daemon.clone(), Rc::clone(&refresh));
        action.connect_clicked(move |button| {
            let (daemon, refresh, turn_on) = (
                daemon.clone(),
                Rc::clone(&refresh),
                button.label().as_deref() == Some(gettext("Turn On").as_str()),
            );
            glib::spawn_future_local(async move {
                if turn_on {
                    daemon.enable_extension().await;
                }
                refresh();
            });
        });
    }
    page
}

fn storage_page(daemon: &Daemon) -> gtk::Box {
    let page = page(&gettext("Where your History is kept"), "");
    let icon = gtk::Image::from_icon_name("channel-secure-symbolic");
    icon.set_pixel_size(64);
    icon.add_css_class("onboarding-brand");
    page.prepend(&icon);
    let row = adw::ActionRow::new();
    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk::SelectionMode::None);
    list.append(&row);
    page.append(&list);
    let daemon = daemon.clone();
    glib::spawn_future_local(async move {
        let storage = daemon.storage().await;
        let (on_disk, notice) = match &storage {
            Some((on_disk, notice)) => (Some(*on_disk), notice.as_str()),
            None => (None, ""),
        };
        let (title, body) = onboarding::describe_storage(on_disk, notice);
        let (title, body) = (tr(title), tr(body));
        row.set_title(&title);
        row.set_subtitle(&body);
    });
    page
}

fn live_test_page(daemon: &Daemon, status: &gtk::Box, pages: Rc<Pages>) -> gtk::Box {
    let page = page(
        &gettext("Try it"),
        &gettext(
            "Copy this made-up token. Copyveil should spot it, mask it and keep it in memory only. Nothing can be done with it.",
        ),
    );
    let token = gtk::Label::new(None);
    token.add_css_class("monospace");
    token.add_css_class("onboarding-token");
    token.set_selectable(false);
    let copy = gtk::Button::with_label(&gettext("Copy Test Token"));
    copy.add_css_class("pill");
    copy.set_halign(gtk::Align::Center);
    page.append(&token);
    page.append(&copy);
    status.set_halign(gtk::Align::Center);
    page.append(status);

    // A new token for every attempt: copying the same one again would only
    // move the existing Item, which the test wouldn't count as new.
    token.set_text(&onboarding::test_token(glib::random_int));

    let (daemon, status) = (daemon.clone(), status.clone());
    copy.connect_clicked(move |button| {
        let (daemon, status, pages, button) = (daemon.clone(), status.clone(), Rc::clone(&pages), button.clone());
        let value = onboarding::test_token(glib::random_int);
        token.set_text(&value);
        button.set_sensitive(false);
        clear(&status);
        status.append(&gtk::Spinner::builder().spinning(true).build());
        glib::spawn_future_local(async move {
            let found = live_test(&daemon, &value).await;
            clear(&status);
            if let Some(item) = found {
                let list = gtk::ListBox::new();
                list.add_css_class("boxed-list");
                list.set_selection_mode(gtk::SelectionMode::None);
                list.append(&ItemRow::new(&item, None, &RowContext::now()).widget);
                status.append(&list);
                status.append(&gtk::Label::new(Some(&gettext("It works. The token is masked and in memory only. Copyveil removed the test Item and cleared the clipboard."))));
                pages.passed.set(true);
            } else {
                let failed = gtk::Label::new(Some(&gettext("Nothing arrived. Check that the GNOME Shell extension is on (step 2) and that Copyveil is running, then try again.")));
                failed.set_wrap(true);
                failed.set_justify(gtk::Justification::Center);
                status.append(&failed);
                button.set_sensitive(true);
            }
            update(&pages);
        });
    });
    page
}

/// The live test: copies `token`, waits for the daemon to list it as a
/// new Suspected Item flagged as a GitHub token, then removes that Item and
/// clears the clipboard. Returns the Item as it was listed (masked), or
/// `None` if nothing arrived in time.
pub async fn live_test(daemon: &Daemon, token: &str) -> Option<ItemSummary> {
    let before: Vec<u64> = daemon
        .list()
        .await
        .unwrap_or_default()
        .iter()
        .map(|item| item.id)
        .collect();
    let clipboard = gdk::Display::default()?.clipboard();
    clipboard.set_text(token);
    let started = Instant::now();
    let found = loop {
        glib::timeout_future(Duration::from_millis(300)).await;
        let now = daemon.list().await.unwrap_or_default();
        if let Some(id) = onboarding::find_test_item(&before, &now) {
            break now.into_iter().find(|item| item.id == id);
        }
        if started.elapsed() > LIVE_TEST_TIMEOUT {
            break None;
        }
    };
    if found.is_some() {
        // Empty the clipboard first, so the token isn't offered again; let
        // any second capture land; then remove every copy of the test Item.
        // `set_text("")` would leave the token in place; an empty provider
        // replaces it, as the extension does after a Secret's Expiry.
        let empty = gdk::ContentProvider::for_bytes(
            "text/plain;charset=utf-8",
            &glib::Bytes::from_static(b""),
        );
        let _ = clipboard.set_content(Some(&empty));
        glib::timeout_future(SETTLE).await;
        let now = daemon.list().await.unwrap_or_default();
        for id in onboarding::find_test_items(&before, &now) {
            daemon.remove(id).await;
        }
    }
    found
}

fn clear(container: &gtk::Box) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}
