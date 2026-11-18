//! The app's stylesheets: the token colours, light or dark following the
//! system, and the component styles that use them.

use gtk::gdk;

const STYLE_LIGHT: &str = include_str!("../../../data/ui/style.css");
const STYLE_DARK: &str = include_str!("../../../data/ui/style-dark.css");
const COMPONENTS: &str = include_str!("../../../data/ui/components.css");

/// Loads the stylesheets for the default display and follows dark mode.
pub fn load() {
    let Some(display) = gdk::Display::default() else {
        return;
    };
    let tokens = gtk::CssProvider::new();
    let components = gtk::CssProvider::new();
    components.load_from_string(COMPONENTS);
    let priority = gtk::STYLE_PROVIDER_PRIORITY_APPLICATION;
    gtk::style_context_add_provider_for_display(&display, &tokens, priority);
    gtk::style_context_add_provider_for_display(&display, &components, priority);

    let style = adw::StyleManager::default();
    let apply = move |style: &adw::StyleManager| {
        tokens.load_from_string(if style.is_dark() {
            STYLE_DARK
        } else {
            STYLE_LIGHT
        });
    };
    apply(&style);
    style.connect_dark_notify(apply);
}
