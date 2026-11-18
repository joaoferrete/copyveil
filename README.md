<p align="center">
  <img src="data/icons/hicolor/scalable/apps/io.github.joaoferrete.Copyveil.svg" width="112" alt="">
</p>

<h1 align="center">Copyveil</h1>

<p align="center"><b>Remembers what you copy. Forgets your secrets.</b></p>

<p align="center">
  <a href="https://github.com/joaoferrete/copyveil/actions/workflows/ci.yml"><img src="https://github.com/joaoferrete/copyveil/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://scorecard.dev/viewer/?uri=github.com/joaoferrete/copyveil"><img src="https://api.scorecard.dev/projects/github.com/joaoferrete/copyveil/badge" alt="OpenSSF Scorecard"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-GPL--3.0--or--later-34465F" alt="License: GPL-3.0-or-later"></a>
</p>

Copyveil is a clipboard history manager for Linux, built for people who copy API keys, tokens and passwords all day. Press <kbd>Super</kbd>+<kbd>V</kbd>, find what you copied, paste it again. Secrets are masked, kept in memory only, and erased after a short time. Everything else is encrypted on disk.

> **Status: pre-alpha.** The daemon and the GNOME Shell extension work: copies are captured, secrets are detected and expire, the clipboard is cleared after a Secret's Expiry, and the rest is encrypted on disk. The app (popup, settings, onboarding) comes next, so there's nothing to click yet. See the [roadmap](docs/roadmap.md).

## How it treats what you copy

Every Item in the History has a **Sensitivity**:

| | How Copyveil knows | Stored | Shown | Erased |
|---|---|---|---|---|
| **Secret** | A password manager marked it (`x-kde-passwordManagerHint`) | Memory only | Solid mask, fixed width | After 30 s, and the clipboard is cleared |
| **Suspected** | It looks like a token or key (GitHub, AWS, AI providers, JWT, PEM, high entropy) | Memory only | Hatched mask, ends visible | After 15 min |
| **Plain** | Nothing suggests a secret | Encrypted on disk | As is | By history limit |

You can change an Item's Sensitivity by hand. Search never looks inside Secret or Suspected Items, so nobody can recover one by typing guesses. The full threat model, including what Copyveil cannot protect against, is in [SECURITY.md](SECURITY.md).

## Desktop support

| Desktop | Capture | Paste with <kbd>Enter</kbd> | Status |
|---|---|---|---|
| GNOME (Wayland) | Copyveil Shell extension | Yes | v0.1 |
| KDE Plasma, COSMIC, wlroots (Sway, Hyprland) | `ext-data-control-v1` | Copies, you press <kbd>Ctrl</kbd>+<kbd>V</kbd> | v0.2 |

GNOME doesn't let regular apps watch the clipboard, so a small Shell extension passes each copy to Copyveil and stores nothing itself. See [ADR 0001](docs/adr/0001-desktop-agnostic-core-with-pluggable-capture-sources.md).

## Install from source

Requirements: Rust (the version in `rust-toolchain.toml`), Meson ≥ 1.1, Ninja, a Secret Service keyring (GNOME Keyring or KWallet) to keep the History on disk, and GTK 4 / libadwaita development files once the app lands.

```sh
git clone https://github.com/joaoferrete/copyveil.git
cd copyveil
meson setup _build --prefix=/usr/local
meson compile -C _build
sudo meson install -C _build
systemctl --user enable --now copyveil-daemon.service
```

The daemon keeps the History. It starts with your session, and capture sources and the app can also start it through D-Bus.

GNOME also needs the Shell extension. Until it's published on extensions.gnome.org, install it from the repository with `just ext-install`, then log out and back in.

Flatpak (Flathub) is planned for v0.2.

## Contributing

Contributions are welcome. [CONTRIBUTING.md](CONTRIBUTING.md) covers setup, commit and PR conventions, and the testing bar. Documentation lives in [`docs/`](docs/README.md), and the project's vocabulary is defined in [GLOSSARY.md](GLOSSARY.md).

## License

[GPL-3.0-or-later](LICENSE).
