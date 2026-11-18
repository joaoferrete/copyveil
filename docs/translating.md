# Translating

Copyveil is translated with gettext. English is the source language, and Brazilian Portuguese ships with v0.1.

There are two catalogs, because the app and the GNOME Shell extension are installed separately:

| Part | Domain | Catalog |
|---|---|---|
| The app (popup, settings, onboarding), its desktop file and metainfo | `copyveil` | `po/` |
| The GNOME Shell extension (panel menu, notices) | `copyveil-extension` | `gnome-extension/po/` |

## Translating into a new language

1. Add the language code (for example `es` or `de`) to `po/LINGUAS` and `gnome-extension/po/LINGUAS`.
2. Create the catalogs from the templates:

   ```sh
   meson setup _build && meson compile -C _build copyveil-pot
   msginit -i po/copyveil.pot -o po/es.po -l es
   tools/extension-pot.sh
   msginit -i gnome-extension/po/copyveil-extension.pot -o gnome-extension/po/es.po -l es
   ```

3. Translate every `msgstr`. Keep the glossary terms consistent (below), and keep each string about as long as the English, since rows and buttons have little room.
4. Check them with `just i18n-check`: it fails when any catalog is invalid, or has a string untranslated or fuzzy against the current English. CI runs it on every PR.

## Updating catalogs after the English changes

```sh
meson compile -C _build copyveil-pot copyveil-update-po
tools/extension-pot.sh --update
```

Commit the updated `.pot` and `.po` files with the change that altered the strings.

## Seeing a translation

```sh
meson setup _build && meson compile -C _build        # compiles po/*.po
COPYVEIL_LOCALEDIR=$PWD/_build/po cargo build -p copyveil-app --example screenshot
cargo build -p copyveil-daemon
LANG=pt_BR.UTF-8 LANGUAGE=pt_BR tools/app-screenshot/run.sh /tmp/shots
```

This renders the popup, settings and onboarding in that language, from an isolated headless GNOME Shell. Strings too long for their row show up at once.

## Glossary terms

Glossary terms are product vocabulary: translate them once, then use the same word everywhere. The Brazilian Portuguese choices:

| English | pt-BR |
|---|---|
| Item | Item |
| History | Histórico |
| Pin / Pin slot | Fixado / posição fixa |
| Expiry | Expiração |
| Sensitivity | Sensibilidade |
| Secret | Segredo |
| Suspected | Suspeito |
| Plain | Comum |
| Mask | Máscara |
| Reveal | Revelar |
| Override | Reclassificar |

## For developers: marking strings

- **App (Rust)**: wrap user-facing strings in `gettext("…")` where they're shown. For a string stored in a constant first, mark it with `N_!("…")` (imported with `use crate::N_;`, since `xgettext` doesn't match `crate::N_!`) and translate it with `i18n::tr` when it's shown. Reasons from the daemon ("Looks like a GitHub token") are English on the bus and translated by the app through `text::reason`, whose table a test keeps in sync with the detectors.
- **Extension (GJS)**: use `_()` from `extension.js`; the extension's `gettext-domain` is set in `metadata.json`.
- Tests compare against the English strings: without a loaded catalog, gettext returns the original.
- Never put Item content in a translatable string or its arguments.
