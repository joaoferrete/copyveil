# Contributing to Copyveil

Thanks for helping. This guide covers how work flows from an idea to `main`.

By taking part you agree to the [Code of Conduct](CODE_OF_CONDUCT.md). Report security problems privately, as described in [SECURITY.md](SECURITY.md).

## Before you start

- **Bugs and small fixes**: open a PR directly, or an issue first if you're unsure.
- **Features and behaviour changes**: open an issue first and agree on the approach before writing code. Anything touching how secrets are handled needs that discussion.
- **Detector false positives or negatives**: use the "Detector got it wrong" issue form. Never paste a real secret anywhere in the project.

## Setup

You need the Rust toolchain pinned in `rust-toolchain.toml` (rustup picks it up automatically), Meson and Ninja, Node.js 22 for the GNOME extension, Python 3 for the design-token tool, and [`just`](https://github.com/casey/just).

Optional tools that CI also runs: `cargo install cargo-llvm-cov cargo-mutants cargo-deny`.

`just --list` shows every task. `just ci` runs the full gate that CI runs.

## Vocabulary

[GLOSSARY.md](GLOSSARY.md) defines the project's terms: Item, History, Sensitivity, Secret, Suspected, Plain, Mask, Reveal, Pin, Expiry, Override. Use them in code, UI strings, docs and discussions. If a term is missing or wrong, change the glossary in the same PR that needs it.

## Commits and pull requests

### Conventional Commits

PR titles follow [Conventional Commits](https://www.conventionalcommits.org/). CI checks the title, because every PR is **squash-merged** and its title becomes the commit on `main` and the CHANGELOG line.

```
<type>(<scope>): <subject>
```

- **Types**: `feat`, `fix`, `perf`, `refactor`, `test`, `docs`, `build`, `ci`, `chore`, `revert`.
- **Scopes** (optional): `core`, `detect`, `daemon`, `keyring`, `app`, `capture`, `gnome-ext`, `design`, `packaging`, `deps`, `repo`.
- **Subject**: imperative, lowercase, no final period. Example: `feat(detect): flag Slack bot tokens`.
- **Breaking changes**: add `!` after the scope (`feat(core)!: …`) and explain the change in the PR body.

Commits inside your branch can be anything; only the PR title lands on `main`.

### Pull requests

- Keep each PR to one concern. Smaller PRs get reviewed faster.
- Fill in the template. The **Security impact** section is required when the PR gets the `security-sensitive` label, which is applied automatically to changes in `crates/copyveil-core`, `crates/copyveil-detect`, `crates/copyveil-daemon`, `crates/copyveil-keyring`, capture sources, the GNOME extension and the build and lint configuration.
- Changes to security-sensitive paths need a code owner's review (`.github/CODEOWNERS`).
- For UI changes, include screenshots in light and dark theme.
- All CI checks must pass. Nobody can merge with a red check, maintainers included.

### Releases

[release-please](https://github.com/googleapis/release-please) opens a release PR from the commits on `main`, with the version bump and CHANGELOG. Merging it tags the release and publishes binaries, SBOMs and provenance attestations. Versioning follows [SemVer](https://semver.org/); before 1.0, `feat` bumps the minor version.

## Testing

The bar is strict, and highest for the code that handles secrets. Read [docs/testing.md](docs/testing.md) before changing `copyveil-core` or `copyveil-detect`. In short: at least 90% line coverage, no surviving mutants, property tests for crypto and Sensitivity rules, and a corpus case for every detector change.

## Documentation

Documentation changes ship in the same PR as the behaviour they describe.

- **Public Rust API**: rustdoc on every public item (`missing_docs` is on).
- **Behaviour and architecture**: [`docs/`](docs/README.md).
- **UI components and tokens**: [`docs/design/`](docs/design/README.md). A new component needs its entry there, with light and dark screenshots.
- **Decisions**: an ADR in [`docs/adr/`](docs/adr/README.md) when a decision is hard to reverse, surprising without context, and the result of a real trade-off.
- **Language**: everything in the repository is in English. The app itself is translated with gettext (English and Brazilian Portuguese to start).

## Code style

`cargo fmt` and `cargo clippy` (pedantic, warnings as errors) are the style guide. The workspace forbids `unsafe` and rejects printing macros. Item content is logged by id and Sensitivity only, never by value.

## AI-assisted contributions

You may use AI tools. [AGENTS.md](AGENTS.md) holds the instructions for coding agents. You are responsible for every line you submit: understand it, test it, and be ready to explain it in review.

## Licence

Copyveil is licensed under [GPL-3.0-or-later](LICENSE). Contributions are accepted under the same licence; there is no CLA.
