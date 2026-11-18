// SPDX-License-Identifier: GPL-3.0-or-later
//
// Pasting into the focused window by injecting Ctrl+V through a virtual
// keyboard. The extension has this power because it runs inside the
// compositor; nothing else in Copyveil can type.

import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';

/** Time for the popup to close and focus to return before typing. */
const FOCUS_DELAY_MS = 60;

export class Keyboard {
    #device = Clutter.get_default_backend()
        .get_default_seat()
        .create_virtual_device(Clutter.InputDeviceType.KEYBOARD_DEVICE);

    #timeout = 0;

    /** Presses Ctrl+V in the focused window, after focus settles. */
    pasteSoon() {
        this.cancel();
        this.#timeout = GLib.timeout_add(GLib.PRIORITY_DEFAULT, FOCUS_DELAY_MS, () => {
            this.#timeout = 0;
            const time = GLib.get_monotonic_time();
            this.#device.notify_keyval(time, Clutter.KEY_Control_L, Clutter.KeyState.PRESSED);
            this.#device.notify_keyval(time, Clutter.KEY_v, Clutter.KeyState.PRESSED);
            this.#device.notify_keyval(time, Clutter.KEY_v, Clutter.KeyState.RELEASED);
            this.#device.notify_keyval(time, Clutter.KEY_Control_L, Clutter.KeyState.RELEASED);
            return GLib.SOURCE_REMOVE;
        });
    }

    /** Drops a pending paste. */
    cancel() {
        if (this.#timeout !== 0)
            GLib.source_remove(this.#timeout);
        this.#timeout = 0;
    }
}
