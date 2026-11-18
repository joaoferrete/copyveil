// SPDX-License-Identifier: GPL-3.0-or-later
//
// The panel icon and its menu.

import Gio from 'gi://Gio';
import GObject from 'gi://GObject';
import St from 'gi://St';

import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';
import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

import {ICON_FILES} from './lib/icon-state.js';

/** What each storage notice from the daemon tells the user (docs/dbus.md). */
const NOTICES = {
    '': null,
    'keyring-unavailable': () => _('No keyring: History is kept in memory only'),
    'store-unavailable': () => _('History can’t be saved: kept in memory only'),
    'store-wiped': () => _('The saved History couldn’t be opened and was erased'),
    'items-unreadable': () => _('Some saved Items couldn’t be read and were erased'),
    'sync-failed': () => _('Saving the History failed; retrying on the next copy'),
};

export const Indicator = GObject.registerClass(
class CopyveilIndicator extends PanelMenu.Button {
    /**
     * @param {Gio.File} iconsDir the extension's `icons/` folder
     * @param {import('./daemon.js').DaemonClient} daemon
     * @param {Gio.Settings} settings the extension's settings
     */
    _init(iconsDir, daemon, settings) {
        super._init(0.5, 'Copyveil');
        this._daemon = daemon;
        this._icons = {};
        for (const [state, name] of Object.entries(ICON_FILES)) {
            const file = iconsDir.get_child(name);
            this._icons[state] = file.query_exists(null)
                ? new Gio.FileIcon({file})
                : Gio.ThemedIcon.new('edit-paste-symbolic');
        }
        this._icon = new St.Icon({gicon: this._icons.normal, style_class: 'system-status-icon'});
        this.add_child(this._icon);

        this.menu.addAction(_('Open History'), () => this._daemon.activateApp('popup'));
        this.menu.addAction(_('Settings'), () => this._daemon.activateApp('preferences'));
        const pause = new PopupMenu.PopupSwitchMenuItem(_('Pause Capture'), settings.get_boolean('capture-paused'));
        pause.connect('toggled', (_item, on) => settings.set_boolean('capture-paused', on));
        this._pauseChanged = settings.connect('changed::capture-paused', () =>
            pause.setToggleState(settings.get_boolean('capture-paused')));
        this._settings = settings;
        this.menu.addMenuItem(pause);
        this.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
        this._status = new PopupMenu.PopupMenuItem('', {reactive: false});
        this._status.visible = false;
        this.menu.addMenuItem(this._status);

        this.menu.connect('open-state-changed', (_menu, open) => {
            if (open)
                this._refreshStatus().catch(() => {});
        });
    }

    /**
     * Shows the icon for `state` (see lib/icon-state.js).
     *
     * @param {'normal'|'paused'|'secret'} state
     */
    setState(state) {
        this._icon.gicon = this._icons[state] ?? this._icons.normal;
        this.accessible_name = {
            normal: 'Copyveil',
            paused: _('Copyveil, capture paused'),
            secret: _('Copyveil, a Secret is in memory'),
        }[state] ?? 'Copyveil';
    }

    destroy() {
        this._settings?.disconnect(this._pauseChanged);
        super.destroy();
    }

    async _refreshStatus() {
        const notice = await this._daemon.storageNotice();
        const describe = notice === null
            ? () => _('Copyveil isn’t running')
            : NOTICES[notice] ?? null;
        this._status.visible = describe !== null;
        if (describe !== null)
            this._status.label.text = describe();
    }
});
