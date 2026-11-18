//! Renders the popup to a PNG, for the light and dark screenshots UI PRs
//! need (`docs/design/README.md`).
//!
//!     screenshot OUT.png [light|dark] [popup|search|settings|onboarding|veil]
//!
//! `veil` captures a new Secret while the popup is open and shoots mid
//! veil-draw, to show the animation is there.
//!
//! It talks to whatever daemon is on the session bus, so run it inside
//! `tools/app-screenshot/run.sh`, which starts an isolated one with
//! synthetic Items.

use adw::prelude::*;
use copyveil_app::daemon::Daemon;
use copyveil_app::window::PopupWindow;
use gtk::glib;
use std::time::Duration;

fn main() -> glib::ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(out) = args.next() else {
        return glib::ExitCode::FAILURE;
    };
    let dark = args.next().as_deref() == Some("dark");
    let view = args.next().unwrap_or_default();

    let app = adw::Application::builder()
        .application_id("io.github.joaoferrete.Copyveil.Screenshot")
        .build();
    app.connect_activate(move |app| {
        let style = adw::StyleManager::default();
        style.set_color_scheme(if dark {
            adw::ColorScheme::ForceDark
        } else {
            adw::ColorScheme::ForceLight
        });
        copyveil_app::style::load();
        let popup = PopupWindow::new(app, Daemon::connect());
        let shown: gtk::Widget = match view.as_str() {
            "settings" => popup.open_settings().upcast(),
            "search" => {
                popup.show_with_query("co");
                popup.window().clone().upcast()
            }
            "onboarding" => {
                copyveil_app::onboarding_window::present(app, &Daemon::connect(), || {}).upcast()
            }
            _ => {
                popup.toggle();
                popup.window().clone().upcast()
            }
        };
        let (app, out) = (app.clone(), out.clone());
        let delay = if view == "veil" {
            // Capture a Secret once the popup has listed, then shoot mid-draw.
            glib::timeout_add_local_once(Duration::from_secs(2), || {
                let _ = std::process::Command::new("busctl")
                    .args([
                        "--user",
                        "call",
                        "io.github.joaoferrete.Copyveil.Daemon",
                        "/io/github/joaoferrete/Copyveil/Daemon",
                        "io.github.joaoferrete.Copyveil.Capture1",
                        "Capture",
                        "sayayb",
                        "text/plain",
                        "4",
                        "120",
                        "121",
                        "122",
                        "119",
                        "0",
                        "true",
                    ])
                    .status();
            });
            Duration::from_millis(2120)
        } else {
            Duration::from_millis(2500)
        };
        glib::timeout_add_local_once(delay, move || {
            let Some(window) = shown.root().and_downcast::<gtk::Window>() else {
                std::process::exit(1);
            };
            let paintable = gtk::WidgetPaintable::new(Some(&window));
            let (width, height) = (window.width(), window.height());
            let snapshot = gtk::Snapshot::new();
            paintable.snapshot(&snapshot, f64::from(width), f64::from(height));
            let saved = snapshot.to_node().and_then(|node| {
                let renderer = window.native()?.renderer()?;
                let texture = renderer.render_texture(node, None);
                texture.save_to_png(&out).ok()
            });
            app.quit();
            if saved.is_none() {
                std::process::exit(1);
            }
        });
    });
    app.run_with_args::<&str>(&[])
}
