# Roadmap

Each release is small and finished. v0.1 is polished on GNOME before Copyveil reaches other desktops.

## v0.1 (GNOME)

- [x] Core: History with Secret / Suspected / Plain, Expiry, Pin, deduplication, Override.
- [x] Encrypted Plain store with the key in the keyring ([ADR 0005](adr/0005-plain-store-is-sqlite-with-per-item-xchacha20-poly1305.md)).
- [x] Daemon serving the History over D-Bus ([API](dbus.md)), with a systemd user unit and D-Bus activation.
- [x] `copyveil-detect`: GitHub, AWS, AI-provider keys, JWT, PEM private keys, payment cards (Luhn), high entropy, plus the password-manager hint.
- [ ] GNOME Shell extension as capture source: panel icon, <kbd>Super</kbd>+<kbd>V</kbd>, paste with <kbd>Ctrl</kbd>+<kbd>V</kbd> injection.
- [ ] Paste by number: <kbd>Ctrl</kbd>+<kbd>1…9</kbd> in the popup (History position), <kbd>Super</kbd>+<kbd>Alt</kbd>+<kbd>1…9</kbd> globally (Pin slot) ([ADR 0004](adr/0004-blind-paste-targets-pin-slots-not-history-positions.md)).
- [ ] App: popup (search, keyboard navigation, Masks, Reveal, Pin), settings with Pin reordering, four-step onboarding.
- [ ] Text, images, and rich text with a plain-text fallback.
- [ ] Design system v1, i18n (English, Brazilian Portuguese).
- [ ] Install from source with Meson. Full CI and repository rules.

## v0.2 (beyond GNOME)

- `ext-data-control` capture source (KDE Plasma, COSMIC, wlroots).
- Global shortcut through the GlobalShortcuts portal.
- Flatpak on Flathub; extension on extensions.gnome.org.
- Custom detector patterns.

## Later

- Paranoid mode: a separate password to unlock the History.
- Copied files (`text/uri-list`), stored as references.
- Automatic paste outside GNOME where `virtual-keyboard` is available.
- More languages.
