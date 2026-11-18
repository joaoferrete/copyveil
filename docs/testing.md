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

`fuzz/` holds three `cargo fuzz` targets, run for ten minutes each every night by `.github/workflows/fuzz.yml`:

| Target | Input | Must hold |
|---|---|---|
| `detect` | Any text | No detector panics |
| `records` | Any bytes as a stored record | Decoding never panics, and whatever decodes encodes back to the same bytes |
| `sealed` | Any bytes as a sealed value | Opening never panics, and no forged value opens |

The targets reach `copyveil-core`'s private decoding paths through its `fuzzing` feature, which no shipped binary enables. To run one locally (needs nightly):

```sh
cd fuzz && cargo +nightly fuzz run records -- -max_total_time=60
```

Every crash becomes a regression test in the PR that fixes it.

## Daemon

`copyveil-daemon` is glue between the core and D-Bus, so it has no coverage gate. Its behaviour is tested end to end instead:

- `tests/bus.rs` runs the real `History1` and `Capture1` interfaces over a peer-to-peer D-Bus connection: no session bus, no keyring. Every method, signal and error in [`dbus.md`](dbus.md) has a test.
- Masked Items must never appear with content in `List`, and a signal must never carry content. Those assertions stay in place.
- `tests/settings.rs` covers `Settings1`: the limits, saving, and memory-only erasing the store and forgetting the key. The keyring is replaced by a `StorageOpener` with a fixed key in a temporary directory (`tests/common/mod.rs`).
- Anything that changes the whole process, such as `harden`, is tested in a test file of its own, so it runs in its own process.
- To try the real binary without touching your keyring, run it on an isolated session bus that has no activatable services, so it falls back to memory only.

## App and capture sources

There's no coverage target here; coverage of GTK glue code says little. Instead:

- Logic that can live outside the widget code (formatting, state machines, view models) does, and it's unit tested.
- The GNOME extension is linted and kept thin enough to review by eye. Its pure logic lives in `gnome-extension/lib/`, with no GNOME imports, and is unit tested with `node --test`. `just ext-lint` runs both, and so does CI.
- `just ext-smoke` runs the extension against the real daemon in a headless GNOME Shell (`tools/ext-smoke/run.sh`). It installs the zip from `just ext-pack` exactly as users get it, compiling nothing on the side, so a packaging mistake fails the test. It checks capture, Secret detection from the password-manager hint, clearing the clipboard after a Secret's Expiry, paste, the Super+V hand-over, and the onboarding's live test (`examples/live_test.rs`): it passes, leaves no test Item behind and leaves the clipboard empty. It runs isolated: a private D-Bus session with no activatable services, throwaway XDG directories and a keyfile GSettings backend. It never touches your session or your keyring. Run it after changing the extension; it needs GNOME Shell installed, so CI doesn't run it.
- UI changes ship with light and dark screenshots: run `just app-screenshot`, which updates the popup, settings and onboarding screenshots in `docs/design/screenshots/` from an isolated headless GNOME Shell with synthetic Items.
- The popup's decisions (what's listed, the selection, Ctrl+number, what may be pinned, what offers "Not a secret" and which detector it can turn off, where the search highlights, how full the Expiry ring is, every string a screen reader hears) live in `crates/copyveil-app/src/popup.rs` and `text.rs`, the settings window's (limits, minutes, Pin moves, shortcut labels) in `prefs.rs`, and the onboarding's (steps, the extension and storage states, the test token, finding the test Item) in `onboarding.rs`. A test checks that the test token is flagged by the real detector. None of them touch GTK, and all are unit tested. Mask geometry is checked against `design/tokens.json`.

## The install script

`install.sh` is checked by hand before each release, in a throwaway `HOME` so nothing of yours is touched:

```sh
HOME=$(mktemp -d) RUSTUP_HOME=~/.rustup CARGO_HOME=~/.cargo ./install.sh --no-session -y
```

It must install the two binaries, the desktop file, both D-Bus services, the icons, the metainfo and the systemd unit under that `HOME`, and `--uninstall --purge --no-session` must leave nothing behind.

## Writing tests

- Name tests after the behaviour: `secret_items_never_reach_disk`, not `test_persist_2`.
- Use synthetic secrets with an obvious marker (`ghp_TEST…`, `COPYVEIL-MARKER-…`). Never use a real credential, not even a revoked one.
- Assertion messages and test names follow the same rule as logs: no Item content.
