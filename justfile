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

# Lint the GNOME extension and run the tests of its pure logic (lib/).
ext-lint:
    cd gnome-extension && npm ci && npm run lint && npm test

data-check:
    desktop-file-validate --no-hints data/io.github.joaoferrete.Copyveil.desktop.in || true
    meson setup _build --wipe -Dprofile=debug >/dev/null && meson test -C _build

# Build and install from source (see README).
install prefix="/usr/local":
    meson setup _build --prefix={{prefix}} -Dprofile=release --wipe
    meson compile -C _build
    meson install -C _build

# Install the GNOME Shell extension for the current user.
# Build the extension zip (as extensions.gnome.org takes it) into _build/.
# Staged from copies, so the zip holds real files and no dev tooling.
ext-pack:
    rm -rf _build/extension && mkdir -p _build/extension/icons
    cd gnome-extension && cp -r metadata.json extension.js clipboard.js daemon.js \
        indicator.js keyboard.js lib schemas ../_build/extension/
    cp data/icons/hicolor/symbolic/apps/io.github.joaoferrete.Copyveil-symbolic.svg \
        _build/extension/icons/copyveil-symbolic.svg
    # GNOME Shell loads extension settings from schemas/gschemas.compiled.
    glib-compile-schemas --strict _build/extension/schemas
    cd _build/extension && gnome-extensions pack --force --out-dir=.. \
        --extra-source=clipboard.js --extra-source=daemon.js \
        --extra-source=indicator.js --extra-source=keyboard.js \
        --extra-source=lib --extra-source=icons \
        --schema=schemas/org.gnome.shell.extensions.copyveil.gschema.xml
    # `gnome-extensions pack` leaves the compiled schema out; add it.
    python3 -c 'import sys, zipfile; z = zipfile.ZipFile(sys.argv[1], "a"); z.write(sys.argv[2], "schemas/gschemas.compiled"); z.close()' \
        _build/copyveil@joaoferrete.github.io.shell-extension.zip _build/extension/schemas/gschemas.compiled

# Light and dark screenshots of the popup, from an isolated headless GNOME
# Shell with synthetic Items (tools/app-screenshot/run.sh). For UI PRs.
app-screenshot out="docs/design/screenshots":
    cargo build --locked -p copyveil-daemon
    cargo build --locked -p copyveil-app --example screenshot
    tools/app-screenshot/run.sh {{out}}

# Run the extension against the real daemon in an isolated, headless GNOME
# Shell (tools/ext-smoke/run.sh). Touches nothing in your session or keyring.
ext-smoke: ext-pack
    cargo build --locked -p copyveil-daemon
    cargo build --locked -p copyveil-app --example live_test
    tools/ext-smoke/run.sh

# Install the extension for this user; log out and back in to load it.
ext-install: ext-pack
    gnome-extensions install --force _build/copyveil@joaoferrete.github.io.shell-extension.zip

# Fuzz one target (detect, records or sealed). Needs nightly and cargo-fuzz.
fuzz target="records" seconds="60":
    cd fuzz && cargo +nightly fuzz run {{target}} -- -max_total_time={{seconds}}
