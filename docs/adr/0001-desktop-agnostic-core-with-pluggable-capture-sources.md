# Desktop-agnostic core with pluggable capture sources

On GNOME Wayland, Mutter only delivers clipboard events to the focused window and implements neither `ext-data-control-v1` nor `wlr-data-control`, so an ordinary app cannot monitor the clipboard there. We run a core process (History, encryption, Sensitivity rules) that is independent of any desktop and receives Items from pluggable capture sources: a minimal GNOME Shell extension that only forwards clipboard events to the core, and an `ext-data-control` client for KDE, wlroots and COSMIC. The UI (popup, settings) is a standalone app, not the extension. This keeps sensitive logic out of the compositor process and lets the project run on other distros and desktops.

## Considered Options

- **GNOME Shell extension only**: rejected. It locks us to GNOME, breaks with Shell releases, and puts secrets inside the compositor process.
- **`ext-data-control` only**: rejected. It doesn't work on GNOME, the author's primary desktop.
