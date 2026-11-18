//! `harden` in a process of its own, since it changes the whole process.

use rustix::process::{DumpableBehavior, dumpable_behavior};

#[test]
fn hardening_makes_the_process_non_dumpable() {
    copyveil_daemon::harden().unwrap();
    assert_eq!(dumpable_behavior().unwrap(), DumpableBehavior::NotDumpable);
}
