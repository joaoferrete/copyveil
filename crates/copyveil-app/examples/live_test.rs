//! Runs the onboarding's live test against the daemon on the session bus and
//! exits 0 if it passes. Used by `tools/ext-smoke/run.sh`; needs the GNOME
//! Shell extension running, since that's what it proves.

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
            let token = onboarding::test_token(glib::random_int);
            let passed = live_test(&Daemon::connect(), &token)
                .await
                .is_some_and(|item| item.sensitivity == "suspected" && item.preview.is_empty());
            app.quit();
            // 0 when the test passed, 1 otherwise.
            std::process::exit(i32::from(!passed));
        });
    });
    app.run_with_args::<&str>(&[])
}
