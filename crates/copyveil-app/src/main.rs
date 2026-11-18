//! `copyveil`: the app. See the library docs.

use adw::prelude::*;
use copyveil_app::daemon::Daemon;
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
        action.connect_activate(move |_, _| popup_for(&app, &popup).toggle());
    }
    app.add_action(&action);

    let quit = gio::SimpleAction::new("quit", None);
    {
        let app = app.clone();
        quit.connect_activate(move |_, _| app.quit());
    }
    app.add_action(&quit);

    {
        let popup = Rc::clone(&popup);
        app.connect_activate(move |app| popup_for(app, &popup).toggle());
    }
    app.run()
}

fn popup_for<'a>(app: &adw::Application, popup: &'a OnceCell<PopupWindow>) -> &'a PopupWindow {
    popup.get_or_init(|| PopupWindow::new(app, Daemon::connect()))
}
