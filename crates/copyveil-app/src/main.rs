//! `copyveil`: the app. See the library docs.

use adw::prelude::*;
use copyveil_app::daemon::Daemon;
use copyveil_app::onboarding_window;
use copyveil_app::window::PopupWindow;
use gtk::{gio, glib};
use std::cell::OnceCell;
use std::rc::Rc;

const APP_ID: &str = "io.github.joaoferrete.Copyveil";

fn main() -> glib::ExitCode {
    let app = adw::Application::builder().application_id(APP_ID).build();
    let popup: Rc<OnceCell<PopupWindow>> = Rc::new(OnceCell::new());

    app.connect_startup(|app| {
        copyveil_app::style::load();
        // Stay running after the popup closes, so the next Super+V is instant.
        std::mem::forget(app.hold());
    });

    let action = gio::SimpleAction::new("popup", None);
    {
        let (app, popup) = (app.clone(), Rc::clone(&popup));
        action.connect_activate(move |_, _| open(&app, &popup));
    }
    app.add_action(&action);

    let preferences = gio::SimpleAction::new("preferences", None);
    {
        let (app, popup) = (app.clone(), Rc::clone(&popup));
        preferences.connect_activate(move |_, _| {
            let _settings = popup_for(&app, &popup).open_settings();
        });
    }
    app.add_action(&preferences);
    app.set_accels_for_action("app.preferences", &["<Control>comma"]);

    let quit = gio::SimpleAction::new("quit", None);
    {
        let app = app.clone();
        quit.connect_activate(move |_, _| app.quit());
    }
    app.add_action(&quit);

    {
        let popup = Rc::clone(&popup);
        app.connect_activate(move |app| open(app, &popup));
    }
    app.run()
}

/// Opens the popup, or the onboarding until it has been completed once.
fn open(app: &adw::Application, popup: &Rc<OnceCell<PopupWindow>>) {
    if onboarding_window::is_done() {
        popup_for(app, popup).toggle();
        return;
    }
    let (app_for_done, popup_for_done) = (app.clone(), Rc::clone(popup));
    let _window = onboarding_window::present(app, &Daemon::connect(), move || {
        popup_for(&app_for_done, &popup_for_done).toggle();
    });
}

fn popup_for<'a>(app: &adw::Application, popup: &'a OnceCell<PopupWindow>) -> &'a PopupWindow {
    popup.get_or_init(|| PopupWindow::new(app, Daemon::connect()))
}
