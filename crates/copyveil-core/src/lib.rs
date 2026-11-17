//! Copyveil core: the History, encryption at rest and Sensitivity rules.
//!
//! This crate is desktop-agnostic. Capture sources hand Items to it; it never
//! talks to a compositor directly (see `docs/adr/0001-*`).

mod sensitivity;

pub use sensitivity::Sensitivity;
