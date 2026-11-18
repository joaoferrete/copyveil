# Engineering guide

How Copyveil's code is shaped, so it stays easy to read, change and trust. These rules describe the code as it is: when the code and this guide disagree, fix one of them in the same PR.

Lints, formatting and the test bar are enforced by tools and aren't repeated here: see `Cargo.toml` (`[workspace.lints]`), `clippy.toml`, `rustfmt.toml`, `deny.toml` and [`testing.md`](testing.md).

## Architecture

Code is layered. Each crate depends only on the layers below it, and `tools/layers.py` checks this in CI.

| Layer | Crate | Owns | Never |
|---|---|---|---|
| Detection | `copyveil-detect` | Deciding whether text looks like a secret | I/O, clocks, state |
| Domain | `copyveil-core` | History, Sensitivity rules, Expiry, encryption, the store | D-Bus, async, desktop APIs, global state |
| Keyring | `copyveil-keyring` | Reading and creating the History key | Anything else |
| Service | `copyveil-daemon` | D-Bus, the async runtime, process setup, file paths | Domain rules |
| Clients | `copyveil-app`, capture sources, `gnome-extension/` | Showing and capturing | Holding History; they reach it over D-Bus only |

- **Rules live in the domain.** A rule about Items (what may persist, what expires, what search may read) goes in `copyveil-core` with its tests, even when only the daemon calls it. The daemon translates and wires; if a daemon function needs a comment explaining a rule, the rule is in the wrong crate.
- **The security crates have an allowlist of dependencies** in `tools/layers.py`. A new dependency there is a new line in that file and a justification in the PR.
- **Desktop-specific code is a capture source.** The core never learns which desktop it runs on ([ADR 0001](adr/0001-desktop-agnostic-core-with-pluggable-capture-sources.md)).
- **The D-Bus API is a contract.** [`dbus.md`](dbus.md) documents it. A breaking change adds a new interface version and leaves the old one alone.

## Modules and types

- **Deep modules, small surfaces.** A module hides a decision behind a few functions. `Store` exposes `open`, `load`, `sync` and `destroy`, and keeps SQLite, sealing and record layout private. Prefer one more private helper to one more public function.
- **Visibility starts at private.** Use `pub(crate)` for crate-internal APIs, and `pub` only for what another crate calls. A `pub` item gets rustdoc.
- **One concept per file.** When a file passes about 400 lines of non-test code, look for the second concept hiding in it. Large test suites move to a sibling `tests.rs` (`history/tests.rs`, `store/tests.rs`).
- **Make illegal states unrepresentable.** Use an enum, not a bool plus an `Option`. Use a newtype, not a bare integer (`ItemId`, not `u64`). Parse at the boundary (`kind_for_mime`, `parse_sensitivity`) and pass typed values inward.
- **No boolean arguments whose meaning isn't obvious at the call site.** `capture(c, true)` says nothing; a named field (`password_manager_hint: true`) or an enum does.
- **Item content lives in content types.** `Content` (and `Key`) implement neither `Debug` nor `Display`, and a `static_assertions` check fails the test build if one starts to. A new type that holds content gets the same treatment and the same check.

## Names

- Use the [glossary](../GLOSSARY.md): Item, History, Pin, Expiry, Sensitivity, Mask, Reveal, Override. Don't use their listed synonyms (no `clip`, `entry`, `ttl`, `favorite`).
- Use whole words (`expires_at`, not `exp`). Short names are fine in short scopes (`id`, `now`, `err`).
- Functions that act are verbs (`capture`, `seal`). Functions that answer are nouns or questions (`next_expiry`, `is_pinned`, `may_persist`).
- Constructors say where the value comes from (`Key::generate`, `Key::from_bytes`, `Capture::secret_text`).

## Errors

- **Each module that fails has its own error enum** (`HistoryError`, `CryptoError`, `StoreError`), with a `Display` written for the user who reads the log. Libraries never return `String` or `Box<dyn Error>`. Only `main` and `run` may.
- **Errors never carry content.** They carry ids, counts and kinds. A wrapped third-party error is fine when its message can't contain content (SQLite and `io` errors can't).
- **Convert at the boundary.** The daemon maps `HistoryError` to D-Bus errors in one `From` impl. Inner layers don't know the outer error types.
- **Failing is explicit.** Don't use `unwrap`, `expect` or `panic!` outside tests; the lints enforce it. When something can't fail, restructure until the type system agrees, or return an error anyway.
- **Degrade instead of dying, and say so.** When the keyring or the store fails, the daemon keeps running in memory only and reports a notice. When a security floor can't be met (`harden`), it refuses to start.

## Time, randomness and I/O

- **The domain takes `now` as an argument.** `History` and `Store` never read the clock. Tests pass exact `Instant`s, so expiry is tested to the nanosecond without sleeping. Only the daemon calls `Instant::now()` and `SystemTime::now()`.
- **Randomness comes from the OS** through `chacha20poly1305`'s `getrandom` (keys, nonces). There's no custom RNG and no seeded RNG outside tests.
- **I/O stays at the edges.** `copyveil-detect` does none. `copyveil-core` touches one file, through `Store`. Everything else is the daemon.

## Async and concurrency

- `copyveil-core` and `copyveil-detect` are synchronous. Async lives in the daemon and the keyring client.
- The daemon's state sits behind one `tokio::sync::Mutex`. Hold it only for the change itself, and release it before awaiting anything else, such as emitting a signal or making a D-Bus call. `HistoryService::change` shows the pattern.
- Background work (the Expiry task) waits on a `Notify` or a timer, never in a polling loop.

## Comments and documentation

- **Rustdoc says what and why for the caller.** It covers what a function promises, what it refuses, and its errors in a `# Errors` section. Public items without docs fail the build.
- **Comments explain why, never what.** A comment that restates the code gets deleted. A comment that points to an ADR or explains a non-obvious choice stays ("Items a newer detector now flags leave the disk now").
- **No commented-out code and no `TODO` without an issue link.** Git keeps history; issues keep plans.
- **Module docs open each file** with one paragraph on what the module is for and what it guarantees.

## Readability

- Return early on errors and edge cases (`let … else`, `?`), so the main path reads straight down without nesting.
- Keep functions short enough to read without scrolling. When a function needs section comments, those sections are functions.
- Prefer iterator chains for transforms and `for` loops when there are side effects or early exits. Choose whichever reads more plainly; clever is a cost.
- Avoid copying content. Pass `&Content`, and use `expose()` only where the bytes are needed: each call is a place where content leaves the vault, and reviewers look for them.
- Prefer the standard library. A new dependency must save real work and pass `cargo deny`.

## The GNOME extension

- It's a capture source: it captures, pastes, clears and binds keys. It stores nothing but the id of the Item it last put on the clipboard.
- It never logs content. `console`, `log` and `logError` are banned by ESLint; nothing goes to the journal.
- Decisions that need no GNOME API (which MIME type to read, when to clear, which bindings to change) live in `gnome-extension/lib/`, with no `gi://` imports, and are unit tested with `node --test`. The rest is thin glue over Shell and D-Bus.
- Logic that grows beyond calling D-Bus and Shell APIs belongs in the daemon, where it can be tested.

## Before you open a PR

- [ ] The change sits in the right layer, and `python3 tools/layers.py` passes.
- [ ] New types holding content implement neither `Debug` nor `Display`, with a `static_assertions` check.
- [ ] Errors, logs, signals and test names carry no content.
- [ ] Public items have rustdoc. Comments explain why.
- [ ] Tests follow [`testing.md`](testing.md), and `just ci` is green.
- [ ] Docs made stale by the change are fixed in the same PR.
