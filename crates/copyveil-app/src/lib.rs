//! The Copyveil app: the popup, settings and onboarding.
//!
//! It reaches the History only over D-Bus, through `copyveil-dbus`
//! (`docs/engineering.md`). The modules here hold everything that can be
//! decided without GTK, so it is unit tested; the widgets only render it.

pub mod popup;
pub mod text;
