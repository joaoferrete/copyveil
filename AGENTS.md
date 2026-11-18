# AGENTS.md

Copyveil is a Linux clipboard manager whose identity is handling secrets with care. A Rust core owns the History, encryption and Sensitivity, and a daemon serves it over D-Bus. Capture sources (a thin GNOME Shell extension, and `ext-data-control` later) feed it, and a GTK4/libadwaita app shows it.

## Vocabulary

Name things with the terms in `GLOSSARY.md`: Item, History, Sensitivity (Plain, Suspected, Secret), Mask, Reveal, Pin, Expiry, Override. These are the names in code identifiers, UI strings, docs and commit messages too. When you need a term the glossary lacks, or the code contradicts it, raise it with the user and update `GLOSSARY.md` in the same change.

## Item content is radioactive

Item content flows only through `copyveil-core`'s content types, which implement neither `Debug` nor `Display`; give any new type holding content the same treatment. Logs, errors, panics, D-Bus signals, test names and assertion messages carry the Item id and Sensitivity, and nothing else. This is the guardrail the whole project exists for, so it outranks convenience every time.

The hard floors in `docs/adr/0002-*` are not settings: Secret and Suspected Items live in memory only, Plain Items are encrypted, and the key lives only in the keyring. Read that ADR before touching storage, expiry or the keyring.

## How to work

1. **Orient.** Read the code you will change and its module docs, plus the ADRs and docs it points to. When writing or reviewing code, follow `docs/engineering.md`: layers, module design, names, errors, time, async, comments.
2. **Place the change.** Domain rules go in `copyveil-core`, D-Bus and process wiring in `copyveil-daemon`, and desktop specifics in a capture source. `python3 tools/layers.py` checks the dependency side of this.
3. **Test first for the security crates.** In `copyveil-core` and `copyveil-detect`, write the failing test, then the code (`docs/testing.md`).
4. **Keep the PR to one concern.** A refactor that a feature needs goes in its own commit or PR, ahead of the feature.
5. **Fix the docs the change made stale**, in the same PR: README, `AGENTS.md`, `CONTRIBUTING.md`, `SECURITY.md`, `docs/` and `GLOSSARY.md`. Search the Markdown for the crates, commands and terms you touched.
6. **Finish green.** Run `just ci`, or the subset for what you touched (`lint`, `test`, `cov`, `mutants`, `tokens`, `ext-lint`), and fix what fails before reporting the work as done.

## Gates move only with the user

The gates are the project's promise, so an agent strengthens them freely and asks the user before loosening any of them:

- **Thresholds and checks**: coverage, mutation testing, the lints in `Cargo.toml` and `clippy.toml`, `deny.toml`, the layer rules, the CI jobs and the branch rules.
- **Survivors and drops**: a surviving mutant or a coverage drop means a test is missing. Write it, and leave the threshold and `.cargo/mutants.toml` alone.
- **Lint suppressions**: every one is `#[expect(lint, reason = "…")]` scoped to the smallest item. When the honest reason is "it's convenient", fix the code instead.
- **Test samples**: always synthetic, with an obvious marker (`COPYVEIL-MARKER-…`, `ghp_TEST…`). Never a real credential, not even a revoked one.

## Where things go

- Changing `copyveil-core` or `copyveil-detect`: read `docs/testing.md` first. Every detector change adds corpus cases.
- Changing the D-Bus API (`crates/copyveil-daemon/src/bus.rs`): update `docs/dbus.md` in the same change. A breaking change adds a new interface version (`History2`), never edits `History1`.
- Adding or changing a UI component, colour or animation: follow `docs/design/README.md`, and change colours only in `design/tokens.json`, then run `just tokens-gen`.
- Making a decision that is hard to reverse, surprising, and a real trade-off: propose an ADR using `docs/adr/README.md`.
- Writing a commit message or PR title: Conventional Commits with the types and scopes in `CONTRIBUTING.md`.
- Adding a dependency: justify it in the PR's security impact section. `cargo deny check` must pass, and a dependency of a security crate goes on its allowlist in `tools/layers.py`.
