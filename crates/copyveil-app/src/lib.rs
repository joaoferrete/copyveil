//! The Copyveil app: the popup, settings and onboarding.
//!
//! It reaches the History only over D-Bus, through `copyveil-dbus`
//! (`docs/engineering.md`). [`popup`] and [`text`] hold everything that can
//! be decided without GTK and are unit tested; the widget modules only
//! render them.

pub mod daemon;
pub mod mask;
pub mod popup;
pub mod prefs;
pub mod prefs_window;
pub mod row;
pub mod style;
pub mod text;
pub mod window;
