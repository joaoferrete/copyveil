//! Runs the onboarding's live test against the daemon on the session bus and
//! prints `live-test: pass` or `live-test: fail`. Used by
//! `tools/ext-smoke/run.sh`; needs the GNOME Shell extension running, since
//! that's what it proves.
//!
//! It then keeps running, like the app does, until it's killed: when a
//! clipboard owner exits, Mutter puts back the last non-empty content it
//! held, which would bring the test token back.

use adw::prelude::*;
use copyveil_app::daemon::Daemon;
use copyveil_app::onboarding;
use copyveil_app::onboarding_window::live_test;
use gtk::glib;

fn main() -> glib::ExitCode {
    let app = adw::Application::builder()
        .application_id("io.github.joaoferrete.Copyveil.LiveTest")
        .build();
    app.connect_activate(|app| {
        // A window, so this Wayland client may set the clipboard.
        let window = adw::ApplicationWindow::builder().application(app).build();
        window.present();
        let app = app.clone();
        glib::spawn_future_local(async move {
            // Wayland lets a client set the clipboard only once its window
            // has focus, as it has when someone clicks the onboarding's button.
            while !window.is_active() {
                glib::timeout_future(std::time::Duration::from_millis(50)).await;
            }
            let token = onboarding::test_token(glib::random_int);
            let passed = live_test(&Daemon::connect(), &token)
                .await
                .is_some_and(|item| item.sensitivity == "suspected" && item.preview.is_empty());
            let line = if passed {
                "live-test: pass"
            } else {
                "live-test: fail"
            };
            std::io::Write::write_all(&mut std::io::stdout(), format!("{line}\n").as_bytes()).ok();
            std::io::Write::flush(&mut std::io::stdout()).ok();
            let _keep = app;
        });
    });
    app.run_with_args::<&str>(&[])
}
