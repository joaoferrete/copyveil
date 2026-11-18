//! The popup window: search, the list, the keyboard and the daemon.
//!
//! It renders a [`Popup`] and turns key presses into calls on it and on the
//! daemon. Typing always goes to the search; arrows, Enter, Ctrl+number,
//! Ctrl+P, Delete, Alt and Escape are handled before the search sees them.

use crate::daemon::Daemon;
use crate::mask::REVEAL_MAX_MS;
use crate::popup::{Popup, Tab, row_number};
use crate::row::ItemRow;
use crate::text;
use adw::prelude::*;
use gtk::gdk;
use gtk::glib::{self, Propagation};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

struct State {
    popup: Popup,
    rows: Vec<ItemRow>,
    /// The row being revealed and the timer that ends the Reveal.
    revealed: Option<(u64, glib::SourceId)>,
    ticker: Option<glib::SourceId>,
}

/// The popup and everything it needs.
#[derive(Clone)]
pub struct PopupWindow {
    window: adw::ApplicationWindow,
    title: adw::WindowTitle,
    banner: adw::Banner,
    search: gtk::SearchEntry,
    list: gtk::ListBox,
    scroller: gtk::ScrolledWindow,
    stack: gtk::Stack,
    empty: adw::StatusPage,
    toasts: adw::ToastOverlay,
    all: gtk::ToggleButton,
    pinned: gtk::ToggleButton,
    daemon: Daemon,
    state: Rc<RefCell<State>>,
}

impl PopupWindow {
    /// Builds the (hidden) popup for `app`.
    #[must_use]
    pub fn new(app: &adw::Application, daemon: Daemon) -> Self {
        let title = adw::WindowTitle::new("Copyveil", "");
        let all = gtk::ToggleButton::with_label("All");
        let pinned = gtk::ToggleButton::with_label("Pinned");
        pinned.set_group(Some(&all));
        all.set_active(true);
        let tabs = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        tabs.add_css_class("linked");
        tabs.append(&all);
        tabs.append(&pinned);

        let header = adw::HeaderBar::new();
        header.set_title_widget(Some(&title));
        // A popup closes; it doesn't minimize or maximize.
        header.set_decoration_layout(Some(":close"));
        let banner = adw::Banner::new("");
        header.pack_start(&tabs);
        let settings = gtk::Button::from_icon_name("emblem-system-symbolic");
        settings.set_tooltip_text(Some("Settings (Ctrl+,)"));
        settings.set_action_name(Some("app.preferences"));
        header.pack_end(&settings);

        let search = gtk::SearchEntry::new();
        search.set_placeholder_text(Some("Search the History"));
        search.set_hexpand(true);
        search.add_css_class("popup-search");

        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::Single);
        list.add_css_class("item-list");
        list.add_css_class("navigation-sidebar");
        let scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .child(&list)
            .build();

        let empty = adw::StatusPage::builder()
            .icon_name("edit-paste-symbolic")
            .title("Nothing copied yet")
            .description(
                "Copy something and it appears here. Secrets are hidden and erased on their own.",
            )
            .build();
        let stack = gtk::Stack::new();
        stack.add_named(&scroller, Some("list"));
        stack.add_named(&empty, Some("empty"));

        let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
        content.set_margin_top(8);
        content.set_margin_start(12);
        content.set_margin_end(12);
        content.append(&search);
        content.append(&stack);

        let toasts = adw::ToastOverlay::new();
        toasts.set_child(Some(&content));

        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&header);
        toolbar.add_top_bar(&banner);
        toolbar.set_content(Some(&toasts));
        toolbar.add_bottom_bar(&hints());

        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("Copyveil")
            .default_width(520)
            .default_height(600)
            .hide_on_close(true)
            .content(&toolbar)
            .build();
        window.add_css_class("copyveil-popup");

        let popup = Self {
            window,
            title,
            banner,
            search,
            list,
            scroller: scroller.clone(),
            stack,
            empty,
            toasts,
            all,
            pinned,
            daemon,
            state: Rc::new(RefCell::new(State {
                popup: Popup::default(),
                rows: Vec::new(),
                revealed: None,
                ticker: None,
            })),
        };
        popup.connect();
        popup
    }

    /// The window, for tools that render it (`examples/screenshot.rs`).
    #[must_use]
    pub fn window(&self) -> &adw::ApplicationWindow {
        &self.window
    }

    /// Closes the popup and opens the settings in their own window.
    #[must_use]
    pub fn open_settings(&self) -> adw::PreferencesDialog {
        if self.window.is_visible() {
            self.hide();
        }
        crate::prefs_window::present(self.daemon.clone())
    }

    /// Shows the popup, or hides it if it's showing (the shortcut toggles).
    pub fn toggle(&self) {
        if self.window.is_visible() {
            self.hide();
        } else {
            self.show();
        }
    }

    fn show(&self) {
        self.search.set_text("");
        self.all.set_active(true);
        self.state.borrow_mut().popup.set_tab(Tab::All);
        self.window.present();
        self.search.grab_focus();
        self.refresh();
        let this = self.clone();
        let ticker = glib::timeout_add_seconds_local(1, move || {
            let now = Instant::now();
            for row in &this.state.borrow().rows {
                row.tick(now);
            }
            glib::ControlFlow::Continue
        });
        if let Some(old) = self.state.borrow_mut().ticker.replace(ticker) {
            old.remove();
        }
    }

    fn hide(&self) {
        self.conceal();
        if let Some(ticker) = self.state.borrow_mut().ticker.take() {
            ticker.remove();
        }
        self.window.set_visible(false);
    }

    fn connect(&self) {
        let this = self.clone();
        self.search.connect_search_changed(move |entry| {
            let query = entry.text().to_string();
            let this = this.clone();
            glib::spawn_future_local(async move {
                let matches = if query.is_empty() {
                    None
                } else {
                    Some(this.daemon.search(query).await.unwrap_or_default())
                };
                this.state.borrow_mut().popup.set_matches(matches);
                this.render();
            });
        });

        for (button, tab) in [(&self.all, Tab::All), (&self.pinned, Tab::Pinned)] {
            let this = self.clone();
            button.connect_toggled(move |button| {
                if button.is_active() {
                    this.state.borrow_mut().popup.set_tab(tab);
                    this.render();
                }
            });
        }

        let this = self.clone();
        self.list.connect_row_selected(move |_, row| {
            let Some(row) = row else { return };
            let id = this.state.borrow().rows.get(row_index(row)).map(|r| r.id);
            if let Some(id) = id {
                this.state.borrow_mut().popup.select(id);
            }
        });
        let this = self.clone();
        self.list.connect_row_activated(move |_, row| {
            let id = this.state.borrow().rows.get(row_index(row)).map(|r| r.id);
            if let Some(id) = id {
                this.paste(id);
            }
        });

        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let this = self.clone();
        keys.connect_key_pressed(move |_, key, _, modifiers| this.key_pressed(key, modifiers));
        let this = self.clone();
        keys.connect_key_released(move |_, key, _, _| {
            if matches!(key, gdk::Key::Alt_L | gdk::Key::Alt_R) {
                this.conceal();
            }
        });
        self.window.add_controller(keys);

        let this = self.clone();
        self.window.connect_is_active_notify(move |window| {
            if !window.is_active() && window.is_visible() {
                this.hide();
            }
        });

        let this = self.clone();
        let changes = self.daemon.changes();
        glib::spawn_future_local(async move {
            while changes.recv().await.is_ok() {
                if this.window.is_visible() {
                    this.refresh();
                }
            }
        });
    }

    fn key_pressed(&self, key: gdk::Key, modifiers: gdk::ModifierType) -> Propagation {
        let ctrl = modifiers.contains(gdk::ModifierType::CONTROL_MASK);
        match key {
            gdk::Key::Escape => self.hide(),
            gdk::Key::Up => self.move_selection(-1),
            gdk::Key::Down => self.move_selection(1),
            gdk::Key::Page_Up => self.move_selection(-8),
            gdk::Key::Page_Down => self.move_selection(8),
            gdk::Key::Return | gdk::Key::KP_Enter => {
                let id = self.state.borrow().popup.selected().map(|item| item.id);
                if let Some(id) = id {
                    self.paste(id);
                }
            }
            gdk::Key::Alt_L | gdk::Key::Alt_R => self.reveal_selected(),
            gdk::Key::Delete if self.search.text().is_empty() => self.remove_selected(),
            gdk::Key::Tab if ctrl => {
                let target = if self.all.is_active() {
                    &self.pinned
                } else {
                    &self.all
                };
                target.set_active(true);
            }
            gdk::Key::p | gdk::Key::P if ctrl => self.toggle_pin(),
            gdk::Key::n | gdk::Key::N if ctrl => self.unflag_selected(),
            key if ctrl && number_key(key).is_some() => {
                let id = number_key(key)
                    .and_then(|n| self.state.borrow().popup.numbered(n).map(|item| item.id));
                if let Some(id) = id {
                    self.paste(id);
                }
            }
            _ => return Propagation::Proceed,
        }
        Propagation::Stop
    }

    /// Lists the History again and updates the storage line.
    fn refresh(&self) {
        let this = self.clone();
        glib::spawn_future_local(async move {
            let items = this.daemon.list().await;
            let storage = this.daemon.storage().await;
            let (on_disk, notice) = match storage {
                Some((on_disk, notice)) => (Some(on_disk), notice),
                None => (None, String::new()),
            };
            this.title.set_subtitle(text::storage_title(on_disk));
            match text::storage_notice(&notice) {
                Some(message) => {
                    this.banner.set_title(message);
                    this.banner.set_revealed(true);
                }
                None => this.banner.set_revealed(false),
            }
            let query = this.search.text().to_string();
            let matches = if query.is_empty() {
                None
            } else {
                this.daemon.search(query).await
            };
            {
                let mut state = this.state.borrow_mut();
                state.popup.set_items(items.unwrap_or_default());
                state.popup.set_matches_keeping_selection(matches);
            }
            this.render();
        });
    }

    /// Rebuilds the rows from the popup state.
    fn render(&self) {
        self.conceal();
        self.list.remove_all();
        let now = Instant::now();
        let mut state = self.state.borrow_mut();
        let rows: Vec<ItemRow> = state
            .popup
            .visible()
            .iter()
            .enumerate()
            .map(|(index, item)| ItemRow::new(item, row_number(index), now))
            .collect();
        for row in &rows {
            self.list.append(&row.widget);
            if let Some(button) = &row.unflag {
                let (this, id) = (self.clone(), row.id);
                button.connect_clicked(move |_| this.unflag(id));
            }
        }
        let selected = state.popup.selected_index();
        state.rows = rows;
        let searching = !self.search.text().is_empty();
        drop(state);

        if let Some(row) = selected.and_then(|index| self.list.row_at_index(index_i32(index))) {
            self.list.select_row(Some(&row));
        }
        let is_empty = self.list.row_at_index(0).is_none();
        if is_empty {
            let (title, description) = match (searching, self.pinned.is_active()) {
                (true, _) => (
                    "No matches",
                    "Secret and Suspected Items match only on their description.",
                ),
                (false, true) => (
                    "No Pins yet",
                    "Pin a Plain Item with Ctrl+P to keep it here and paste it with Super+Alt+number.",
                ),
                (false, false) => (
                    "Nothing copied yet",
                    "Copy something and it appears here. Secrets are hidden and erased on their own.",
                ),
            };
            self.empty.set_title(title);
            self.empty.set_description(Some(description));
        }
        self.stack
            .set_visible_child_name(if is_empty { "empty" } else { "list" });
    }

    fn move_selection(&self, delta: isize) {
        self.conceal();
        let index = {
            let mut state = self.state.borrow_mut();
            state.popup.move_selection(delta);
            state.popup.selected_index()
        };
        if let Some(row) = index.and_then(|index| self.list.row_at_index(index_i32(index))) {
            self.list.select_row(Some(&row));
            self.scroll_to(&row);
        }
    }

    /// Scrolls the list so `row` is visible, leaving focus in the search.
    fn scroll_to(&self, row: &gtk::ListBoxRow) {
        let Some(bounds) = row.compute_bounds(&self.list) else {
            return;
        };
        let adjustment = self.scroller.vadjustment();
        let (top, bottom) = (
            f64::from(bounds.y()),
            f64::from(bounds.y() + bounds.height()),
        );
        if top < adjustment.value() {
            adjustment.set_value(top);
        } else if bottom > adjustment.value() + adjustment.page_size() {
            adjustment.set_value(bottom - adjustment.page_size());
        }
    }

    fn paste(&self, id: u64) {
        self.hide();
        let daemon = self.daemon.clone();
        glib::spawn_future_local(async move {
            daemon.paste(id).await;
        });
    }

    fn toggle_pin(&self) {
        let Some(item) = self.state.borrow().popup.selected().cloned() else {
            return;
        };
        if let Err(refusal) = Popup::may_pin(&item) {
            self.toasts.add_toast(adw::Toast::new(refusal.message()));
            return;
        }
        let this = self.clone();
        glib::spawn_future_local(async move {
            let pinned = item.pin_slot == 0;
            if this.daemon.set_pinned(item.id, pinned).await.is_none() {
                this.toasts.add_toast(adw::Toast::new(
                    "Couldn’t change the Pin. Is Copyveil running?",
                ));
            }
        });
    }

    /// "Not a secret": Overrides a Suspected Item to Plain, then offers to
    /// stop flagging text like it (turning its detector off).
    fn unflag_selected(&self) {
        let selected = self.state.borrow().popup.selected().cloned();
        if let Some(item) = selected.filter(Popup::may_unflag) {
            self.unflag(item.id);
        }
    }

    fn unflag(&self, id: u64) {
        let Some(item) = self
            .state
            .borrow()
            .popup
            .visible()
            .into_iter()
            .find(|item| item.id == id)
            .cloned()
        else {
            return;
        };
        let this = self.clone();
        glib::spawn_future_local(async move {
            if this
                .daemon
                .override_sensitivity(item.id, copyveil_dbus::sensitivity::PLAIN)
                .await
                .is_none()
            {
                this.toasts
                    .add_toast(adw::Toast::new("Couldn’t change it. Is Copyveil running?"));
                return;
            }
            let toast = adw::Toast::new("Marked as not a secret");
            let detectors = this.daemon.detectors().await.unwrap_or_default();
            if let Some((name, title)) = crate::popup::detector_for(&item.reason, &detectors) {
                toast.set_button_label(Some(&format!("Stop flagging {}", title.to_lowercase())));
                let (daemon, toasts, name) =
                    (this.daemon.clone(), this.toasts.clone(), name.to_owned());
                toast.connect_button_clicked(move |_| {
                    let (daemon, toasts, name) = (daemon.clone(), toasts.clone(), name.clone());
                    glib::spawn_future_local(async move {
                        let mut disabled = daemon
                            .settings()
                            .await
                            .map(|values| values.disabled_detectors)
                            .unwrap_or_default();
                        if !disabled.contains(&name) {
                            disabled.push(name);
                        }
                        let message = match daemon
                            .change(crate::daemon::SettingChange::DisabledDetectors(disabled))
                            .await
                        {
                            Ok(()) => {
                                "Turned off. Change it back in Settings › Detection.".to_owned()
                            }
                            Err(message) => message,
                        };
                        toasts.add_toast(adw::Toast::new(&message));
                    });
                });
            }
            this.toasts.add_toast(toast);
        });
    }

    fn remove_selected(&self) {
        let Some(id) = self.state.borrow().popup.selected().map(|item| item.id) else {
            return;
        };
        let daemon = self.daemon.clone();
        glib::spawn_future_local(async move {
            daemon.remove(id).await;
        });
    }

    /// Reveals the selected masked Item while Alt is held, for at most 5 s.
    fn reveal_selected(&self) {
        let Some(id) = self.state.borrow().popup.selected().map(|item| item.id) else {
            return;
        };
        let masked = self
            .state
            .borrow()
            .rows
            .iter()
            .any(|row| row.id == id && row.masked);
        if !masked || self.state.borrow().revealed.is_some() {
            return;
        }
        let this = self.clone();
        glib::spawn_future_local(async move {
            let Some(value) = this.daemon.reveal(id).await else {
                return;
            };
            let this_timer = this.clone();
            let timer =
                glib::timeout_add_local_once(Duration::from_millis(REVEAL_MAX_MS), move || {
                    this_timer.state.borrow_mut().revealed = None;
                    this_timer.conceal();
                });
            let state = this.state.borrow();
            if let Some(row) = state.rows.iter().find(|row| row.id == id) {
                row.reveal(&value);
            }
            drop(state);
            this.state.borrow_mut().revealed = Some((id, timer));
        });
    }

    fn conceal(&self) {
        let revealed = self.state.borrow_mut().revealed.take();
        if let Some((_, timer)) = revealed {
            timer.remove();
        }
        for row in &self.state.borrow().rows {
            row.conceal();
        }
    }
}

/// The keyboard hints along the bottom of the popup.
fn hints() -> gtk::Box {
    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    bar.add_css_class("popup-hints");
    bar.set_halign(gtk::Align::Center);
    bar.set_margin_top(6);
    bar.set_margin_bottom(6);
    for (keys, action) in [
        ("↵", "Paste"),
        ("Alt", "Hold to reveal"),
        ("Ctrl P", "Pin"),
        ("Del", "Remove"),
    ] {
        let hint = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let key = gtk::Label::new(Some(keys));
        key.add_css_class("keycap");
        let label = gtk::Label::new(Some(action));
        label.add_css_class("dim-label");
        label.add_css_class("caption");
        hint.append(&key);
        hint.append(&label);
        bar.append(&hint);
    }
    bar
}

fn number_key(key: gdk::Key) -> Option<usize> {
    let keys = [
        gdk::Key::_1,
        gdk::Key::_2,
        gdk::Key::_3,
        gdk::Key::_4,
        gdk::Key::_5,
        gdk::Key::_6,
        gdk::Key::_7,
        gdk::Key::_8,
        gdk::Key::_9,
    ];
    keys.iter().position(|k| *k == key).map(|index| index + 1)
}

fn row_index(row: &gtk::ListBoxRow) -> usize {
    usize::try_from(row.index()).unwrap_or(usize::MAX)
}

fn index_i32(index: usize) -> i32 {
    i32::try_from(index).unwrap_or(i32::MAX)
}
