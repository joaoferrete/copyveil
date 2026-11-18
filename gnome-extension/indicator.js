// SPDX-License-Identifier: GPL-3.0-or-later
//
// The panel icon and its menu.

import Gio from 'gi://Gio';
import GObject from 'gi://GObject';
import St from 'gi://St';

import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';
import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

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
     * @param {Gio.File} iconFile the symbolic icon shipped with the extension
     * @param {import('./daemon.js').DaemonClient} daemon
     */
    _init(iconFile, daemon) {
        super._init(0.5, 'Copyveil');
        this._daemon = daemon;

        const gicon = iconFile.query_exists(null)
            ? new Gio.FileIcon({file: iconFile})
            : Gio.ThemedIcon.new('edit-paste-symbolic');
        this.add_child(new St.Icon({gicon, style_class: 'system-status-icon'}));

        this.menu.addAction(_('Open History'), () => this._daemon.activateApp('popup'));
        this.menu.addAction(_('Settings'), () => this._daemon.activateApp('preferences'));
        this.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
        this._status = new PopupMenu.PopupMenuItem('', {reactive: false});
        this._status.visible = false;
        this.menu.addMenuItem(this._status);

        this.menu.connect('open-state-changed', (_menu, open) => {
            if (open)
                this._refreshStatus().catch(() => {});
        });
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
