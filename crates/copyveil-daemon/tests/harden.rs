//! `harden` in a process of its own, since it changes the whole process.

#![allow(clippy::unwrap_used, reason = "a failing test should panic")]

use rustix::process::{DumpableBehavior, dumpable_behavior};

#[test]
fn hardening_makes_the_process_non_dumpable() {
    copyveil_daemon::harden().unwrap();
    assert_eq!(dumpable_behavior().unwrap(), DumpableBehavior::NotDumpable);
}
