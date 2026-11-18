// SPDX-License-Identifier: GPL-3.0-or-later
//
// Copyveil capture source for GNOME Shell.
//
// The extension stays thin: it forwards clipboard changes to the daemon over
// D-Bus (docs/dbus.md), pastes when the daemon asks, clears the clipboard
// when the Item on it reaches its Expiry, draws the panel icon and binds the
// shortcuts. It holds no History and buffers nothing: when the daemon is
// unreachable, the change is dropped. It keeps one thing: the id of the Item
// on the clipboard. See docs/adr/0001-* and docs/engineering.md.

import Gio from 'gi://Gio';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

import {Clipboard} from './clipboard.js';
import {DaemonClient} from './daemon.js';
import {Indicator} from './indicator.js';
import {Keyboard} from './keyboard.js';
import {ClipboardItem} from './lib/clipboard-item.js';
import {iconState} from './lib/icon-state.js';
import {pasteType} from './lib/mime.js';
import {withSuperV, withoutSuperV} from './lib/shortcuts.js';

const PIN_SLOTS = [1, 2, 3, 4, 5, 6, 7, 8, 9];
const SHELL_KEYBINDINGS = 'org.gnome.shell.keybindings';
const MESSAGE_TRAY = 'toggle-message-tray';

export default class CopyveilExtension extends Extension {
    enable() {
        this._settings = this.getSettings();
        this._shellKeys = new Gio.Settings({schema_id: SHELL_KEYBINDINGS});
        this._daemon = new DaemonClient();
        this._clipboard = new Clipboard();
        this._keyboard = new Keyboard();
        this._onClipboard = new ClipboardItem();

        this._clipboard.watch(read => this._capture(read).catch(() => {}));
        this._daemon.watch(
            ids => this._expired(ids),
            id => this._paste(id).catch(() => {}));

        this._indicator = new Indicator(this.dir.get_child('icons'), this._daemon, this._settings);
        this._daemon.watchHistory(() => this._refreshIcon().catch(() => {}));
        this._pausedChanged = this._settings.connect('changed::capture-paused', () => this._refreshIcon().catch(() => {}));
        this._refreshIcon().catch(() => {});
        Main.panel.addToStatusArea(this.uuid, this._indicator);

        this._applySuperV();
        this._superVChanged = this._settings.connect('changed::take-super-v', () => this._applySuperV());
        this._bindShortcuts();
    }

    disable() {
        this._settings.disconnect(this._pausedChanged);
        this._unbindShortcuts();
        this._settings.disconnect(this._superVChanged);
        this._giveSuperVBack();

        this._indicator.destroy();
        this._keyboard.cancel();
        this._daemon.unwatch();
        this._clipboard.unwatch();

        this._indicator = null;
        this._keyboard = null;
        this._daemon = null;
        this._clipboard = null;
        this._onClipboard = null;
        this._shellKeys = null;
        this._settings = null;
    }

    async _refreshIcon() {
        const sensitivities = await this._daemon?.sensitivities() ?? [];
        this._indicator?.setState(iconState({
            paused: this._settings?.get_boolean('capture-paused') ?? false,
            sensitivities,
        }));
    }

    async _capture(read) {
        if (!this._onClipboard.shouldCapture())
            return;
        // Paused: copies aren't read or sent. The id on the clipboard is no
        // longer known, so an Expiry won't clear what was copied meanwhile.
        if (this._settings.get_boolean('capture-paused')) {
            this._onClipboard.captured(null);
            return;
        }
        const offer = await read();
        if (offer === null) {
            this._onClipboard.captured(null);
            return;
        }
        const id = await this._daemon?.capture(offer.mime, offer.content, offer.fallback, offer.hint);
        this._onClipboard?.captured(id ?? null);
    }

    _expired(ids) {
        if (!this._onClipboard.expires(ids))
            return;
        this._onClipboard.settingOwn(null);
        this._clipboard.clear();
    }

    /**
     * Puts an Item on the clipboard and presses Ctrl+V. The content goes from
     * the daemon to the clipboard without being kept.
     *
     * @param {bigint} id the Item to paste
     */
    async _paste(id) {
        const item = await this._daemon?.itemForPaste(id);
        if (!item || this._clipboard === null)
            return;
        const bytes = item.summary.kind === 'rich-text' && item.fallback.length > 0
            ? item.fallback
            : item.content;
        this._onClipboard.settingOwn(id);
        this._clipboard.set(pasteType(item.summary), bytes);
        this._keyboard.pasteSoon();
    }

    async _pastePinSlot(slot) {
        const id = await this._daemon?.pinSlot(slot);
        if (id !== null && id !== undefined)
            await this._paste(id);
    }

    _bindShortcuts() {
        const mode = Shell.ActionMode.NORMAL | Shell.ActionMode.OVERVIEW;
        const bind = (name, handler) =>
            Main.wm.addKeybinding(name, this._settings, Meta.KeyBindingFlags.IGNORE_AUTOREPEAT, mode, handler);
        bind('popup-shortcut', () => this._daemon.activateApp('popup'));
        for (const slot of PIN_SLOTS)
            bind(`paste-pin-${slot}`, () => this._pastePinSlot(slot).catch(() => {}));
    }

    _unbindShortcuts() {
        Main.wm.removeKeybinding('popup-shortcut');
        for (const slot of PIN_SLOTS)
            Main.wm.removeKeybinding(`paste-pin-${slot}`);
    }

    /** Takes Super+V from the notification list once the user agreed in onboarding. */
    _applySuperV() {
        if (!this._settings.get_boolean('take-super-v')) {
            this._giveSuperVBack();
            return;
        }
        const {bindings, removed} = withoutSuperV(this._shellKeys.get_strv(MESSAGE_TRAY));
        if (removed) {
            this._shellKeys.set_strv(MESSAGE_TRAY, bindings);
            this._settings.set_boolean('took-super-v', true);
        }
    }

    /** Returns Super+V to the notification list if this extension took it. */
    _giveSuperVBack() {
        if (!this._settings.get_boolean('took-super-v'))
            return;
        this._shellKeys.set_strv(MESSAGE_TRAY, withSuperV(this._shellKeys.get_strv(MESSAGE_TRAY)));
        this._settings.set_boolean('took-super-v', false);
    }
}
