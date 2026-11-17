# AGENTS.md

Copyveil is a Linux clipboard manager whose identity is handling secrets with care. A Rust core owns History, encryption and Sensitivity. Capture sources (a thin GNOME Shell extension; `ext-data-control` later) feed it, and a GTK4/libadwaita app shows it.

## Vocabulary

Name things with the terms in `GLOSSARY.md`: Item, History, Sensitivity (Plain, Suspected, Secret), Mask, Reveal, Pin, Expiry, Override. These are the names in code identifiers, UI strings, docs and commit messages too. When you need a term the glossary lacks, or the code contradicts it, raise it with the user and update `GLOSSARY.md` in the same change.

## Item content is radioactive

Item content flows only through `copyveil-core`'s content types, which implement neither `Debug` nor `Display`; give any new type holding content the same treatment. Logs, errors, panics, D-Bus signals, test names and assertion messages carry the Item id and Sensitivity, and nothing else. This is the guardrail the whole project exists for, so it outranks convenience every time.

The hard floors in `docs/adr/0002-*` are not settings: Secret and Suspected Items live in memory only, Plain Items are encrypted, and the key lives only in the keyring. Read that ADR before touching storage, expiry or the keyring.

## Done means `just ci` is green

`just --list` shows the tasks. Before reporting work as done, run `just ci` (or the subset for what you touched: `lint`, `test`, `cov`, `mutants`, `tokens`, `ext-lint`) and fix what fails. A surviving mutant or a coverage drop in `copyveil-core` or `copyveil-detect` is a failing test suite. Write the missing test; leave the threshold where it is.

## Where things go

- Changing `copyveil-core` or `copyveil-detect`: read `docs/testing.md` first. Every detector change adds corpus cases.
- Adding or changing a UI component, colour or animation: follow `docs/design/README.md`, and change colours only in `design/tokens.json`, then run `just tokens-gen`.
- Making a decision that is hard to reverse, surprising, and a real trade-off: propose an ADR using `docs/adr/README.md`.
- Writing a commit message or PR title: Conventional Commits with the types and scopes in `CONTRIBUTING.md`.
- Adding a dependency: justify it in the PR's security impact section; `cargo deny check` must pass.
