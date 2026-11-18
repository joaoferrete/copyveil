# Developer shortcuts. CI runs the same commands; `just ci` is the full local gate.

default:
    @just --list

# Format all Rust code.
fmt:
    cargo fmt --all

# Everything CI checks, in the same order.
ci: lint test cov mutants deny tokens ext-lint data-check

# Format check, clippy with warnings as errors, and the architecture layers.
lint:
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets --locked -- -D warnings
    python3 tools/layers.py

test:
    cargo test --workspace --locked

# Line coverage gate for the security crates (docs/testing.md).
cov:
    cargo llvm-cov --locked -p copyveil-core -p copyveil-detect --fail-under-lines 90

# Mutation testing for the security crates. Every surviving mutant fails.
mutants:
    cargo mutants --package copyveil-core --package copyveil-detect --no-shuffle

deny:
    cargo deny check

tokens:
    python3 tools/tokens.py --check

# Regenerate data/ui/*.css from design/tokens.json.
tokens-gen:
    python3 tools/tokens.py

ext-lint:
    cd gnome-extension && npm ci && npm run lint

data-check:
    desktop-file-validate --no-hints data/io.github.joaoferrete.Copyveil.desktop.in || true
    meson setup _build --wipe -Dprofile=debug >/dev/null && meson test -C _build

# Build and install from source (see README).
install prefix="/usr/local":
    meson setup _build --prefix={{prefix}} -Dprofile=release --wipe
    meson compile -C _build
    meson install -C _build

# Install the GNOME Shell extension for the current user.
ext-install:
    mkdir -p ~/.local/share/gnome-shell/extensions/copyveil@joaoferrete.github.io
    cp gnome-extension/metadata.json gnome-extension/extension.js ~/.local/share/gnome-shell/extensions/copyveil@joaoferrete.github.io/

# Fuzz one target (detect, records or sealed). Needs nightly and cargo-fuzz.
fuzz target="records" seconds="60":
    cd fuzz && cargo +nightly fuzz run {{target}} -- -max_total_time={{seconds}}
