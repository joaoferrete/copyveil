//! Translations, through gettext (domain `copyveil`).
//!
//! Strings are marked with `gettext("…")` where they're shown, or with
//! [`N_`] where they're stored first and shown later (constants), so
//! `xgettext` finds them all. The translations live in `po/`; see
//! `docs/translating.md`.

use gettextrs::{bind_textdomain_codeset, bindtextdomain, textdomain};

/// The app's gettext domain.
pub const DOMAIN: &str = "copyveil";

/// Where the compiled translations are: set by the Meson build, with the
/// usual system path for plain `cargo` builds.
const LOCALEDIR: &str = match option_env!("COPYVEIL_LOCALEDIR") {
    Some(dir) => dir,
    None => "/usr/local/share/locale",
};

/// Loads the app's translations. The language comes from the locale,
/// which GTK sets up when it starts (`gtk_init` calls `setlocale`), before
/// any widget asks for a string. Without translations, every string stays
/// in English.
pub fn init() {
    // Failing to bind just leaves the English strings, which is a fine fallback.
    let _ = bindtextdomain(DOMAIN, LOCALEDIR);
    let _ = bind_textdomain_codeset(DOMAIN, "UTF-8");
    let _ = textdomain(DOMAIN);
}

/// Translates `msgid`.
#[must_use]
pub fn tr(msgid: &str) -> String {
    gettextrs::gettext(msgid)
}

/// Marks a string for translation without translating it yet: for
/// constants, translated with [`tr`] where they're shown.
#[macro_export]
macro_rules! N_ {
    ($msgid:literal) => {
        $msgid
    };
}

#[cfg(test)]
mod tests {
    use super::tr;

    #[test]
    fn untranslated_strings_stay_in_english() {
        assert_eq!(tr("Settings"), "Settings");
    }
}
