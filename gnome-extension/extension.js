// SPDX-License-Identifier: GPL-3.0-or-later
//
// Copyveil capture source for GNOME Shell.
//
// The extension stays thin: it forwards clipboard changes (content plus the
// offered MIME types, including `x-kde-passwordManagerHint`) to the daemon
// over D-Bus, draws the panel icon, binds the global shortcut and injects
// Ctrl+V on paste. It holds no History and buffers nothing: when the daemon
// is unreachable, the change is dropped. See docs/adr/0001-*.

import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

export default class CopyveilExtension extends Extension {
    enable() {
        // Wiring lands with the GNOME capture source milestone (docs/roadmap.md).
    }

    disable() {
        // Every signal, keybinding and panel item created in enable() is released here.
    }
}
