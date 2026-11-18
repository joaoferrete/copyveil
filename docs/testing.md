# Testing

Copyveil's promise is that secrets don't leak. The tests are how that promise is kept, so the bar is set per crate, by how much a bug there would cost.

## Security crates: `copyveil-core` and `copyveil-detect`

These gates run in CI on every PR and block merging:

| Gate | Tool | Bar |
|---|---|---|
| Line coverage | `cargo llvm-cov` | ≥ 90% (`just cov`) |
| Mutation testing | `cargo mutants` | No surviving mutants (`just mutants`) |
| Lints | `clippy` pedantic | Warnings are errors |

When a mutant survives, the test suite has a hole: add the test that kills it. A mutant that genuinely can't be caught (for example, an equivalent mutation) is excluded in `.cargo/mutants.toml` with a comment explaining why, and the PR's reviewer must agree.

### Required test suites

- **Sensitivity rules**: one test per rule in `docs/adr/0002-*`. Secret and Suspected never persist, only Plain can be pinned, and Override changes storage as described.
- **Crypto round-trip** (property test, `proptest`): any content encrypts and decrypts to itself, two encryptions of the same content differ, and stored records decode to what was encoded.
- **Leak test** (`crates/copyveil-core/tests/leak.rs`): run the core against a temporary directory, feed it Secret and Suspected Items with known marker values, let them expire, and assert that no byte sequence of any marker appears in any file under that directory, including the database, its journal and temporary files.
- **No-leak formatting**: content types implement neither `Debug` nor `Display`, checked by compile-fail tests (`trybuild`).

### Detector corpus

Each detector in `copyveil-detect` has two corpus files under `crates/copyveil-detect/tests/corpus/`:

- `<detector>.positive.txt`: synthetic values in every format the detector claims to recognise. These are made up, never real secrets.
- `<detector>.negative.txt`: known false positives (UUIDs, commit hashes, base64 images, file paths…).

One sample per line; blank lines and lines starting with `#` are ignored. `tests/corpus.rs` checks that every positive is reported as its own detector and that no negative is flagged by any detector.

Every "Detector got it wrong" issue becomes a corpus line in the PR that fixes it. A detector change without a corpus change won't be merged.

### Fuzzing

Nightly fuzzing with `cargo fuzz` covers the detectors and the store's decoding paths once they exist. Every crash becomes a regression test.

## Daemon

`copyveil-daemon` is glue between the core and D-Bus, so it has no coverage gate. Its behaviour is tested end to end instead:

- `tests/bus.rs` runs the real `History1` and `Capture1` interfaces over a peer-to-peer D-Bus connection: no session bus, no keyring. Every method, signal and error in [`dbus.md`](dbus.md) has a test.
- Masked Items must never appear with content in `List`, and a signal must never carry content. Those assertions stay in place.
- Anything that changes the whole process, such as `harden`, is tested in a test file of its own, so it runs in its own process.
- To try the real binary without touching your keyring, run it on an isolated session bus that has no activatable services, so it falls back to memory only.

## App and capture sources

There's no coverage target here; coverage of GTK glue code says little. Instead:

- Logic that can live outside the widget code (formatting, state machines, view models) does, and it's unit tested.
- The GNOME extension is linted (`just ext-lint`) and kept thin enough to review by eye.
- UI changes ship with light and dark screenshots in the PR.

## Writing tests

- Name tests after the behaviour: `secret_items_never_reach_disk`, not `test_persist_2`.
- Use synthetic secrets with an obvious marker (`ghp_TEST…`, `COPYVEIL-MARKER-…`). Never use a real credential, not even a revoked one.
- Assertion messages and test names follow the same rule as logs: no Item content.
