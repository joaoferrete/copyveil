// SPDX-License-Identifier: GPL-3.0-or-later
//
// The daemon's D-Bus API (docs/dbus.md), as the extension uses it. Calls
// start the daemon through D-Bus activation. Any failure resolves to
// `null`: the extension drops what it was doing and never retries with
// content in hand.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

const BUS_NAME = 'io.github.joaoferrete.Copyveil.Daemon';
const OBJECT_PATH = '/io/github/joaoferrete/Copyveil/Daemon';
const CAPTURE = 'io.github.joaoferrete.Copyveil.Capture1';
const HISTORY = 'io.github.joaoferrete.Copyveil.History1';

const APP_ID = 'io.github.joaoferrete.Copyveil';
const APP_PATH = '/io/github/joaoferrete/Copyveil';

Gio._promisify(Gio.DBusConnection.prototype, 'call');

export class DaemonClient {
    #bus = Gio.DBus.session;
    #subscriptions = [];

    /**
     * Calls `fn(ids)` when Items reach their Expiry, and `onPaste(id)` when
     * the user asks to paste one. Both receive ids only.
     *
     * @param {(ids: bigint[]) => void} onExpired
     * @param {(id: bigint) => void} onPaste
     */
    watch(onExpired, onPaste) {
        const subscribe = (member, handler) => this.#bus.signal_subscribe(
            BUS_NAME, CAPTURE, member, OBJECT_PATH, null, Gio.DBusSignalFlags.NONE,
            (_conn, _sender, _path, _iface, _member, params) => handler(params));
        this.#subscriptions.push(
            subscribe('Expired', params => onExpired(params.get_child_value(0).deepUnpack().map(BigInt))),
            subscribe('PasteRequested', params => onPaste(BigInt(params.get_child_value(0).deepUnpack()))));
    }

    /** Stops delivering signals. */
    unwatch() {
        for (const id of this.#subscriptions)
            this.#bus.signal_unsubscribe(id);
        this.#subscriptions = [];
    }

    /**
     * Reports a clipboard change. Returns the Item's id, or `null`.
     *
     * @param {string} mime
     * @param {Uint8Array} content
     * @param {Uint8Array} fallback empty when there is none
     * @param {boolean} hint
     * @returns {Promise<bigint|null>}
     */
    async capture(mime, content, fallback, hint) {
        const reply = await this.#call(CAPTURE, 'Capture',
            new GLib.Variant('(sayayb)', [mime, content, fallback, hint]), '(t)');
        return reply === null ? null : BigInt(reply.get_child_value(0).deepUnpack());
    }

    /**
     * The content of one Item and its summary, for a paste. `null` if it's gone.
     *
     * @param {bigint} id
     * @returns {Promise<{content: Uint8Array, fallback: Uint8Array, summary: object}|null>}
     */
    async itemForPaste(id) {
        const list = await this.#call(HISTORY, 'List', null, '(a(tssssuttts))');
        if (list === null)
            return null;
        const row = list.get_child_value(0).deepUnpack()
            .find(item => BigInt(item[0]) === id);
        if (!row)
            return null;
        const reply = await this.#call(HISTORY, 'Content', new GLib.Variant('(t)', [id]), '(ayay)');
        if (reply === null)
            return null;
        return {
            content: reply.get_child_value(0).deepUnpack(),
            fallback: reply.get_child_value(1).deepUnpack(),
            summary: {kind: row[1], mime: row[2]},
        };
    }

    /**
     * The id of the Item in Pin slot `slot`, or `null` if the slot is empty.
     *
     * @param {number} slot counting from 1
     * @returns {Promise<bigint|null>}
     */
    async pinSlot(slot) {
        const reply = await this.#call(HISTORY, 'PinSlot', new GLib.Variant('(u)', [slot]), '(t)');
        return reply === null ? null : BigInt(reply.get_child_value(0).deepUnpack());
    }

    /**
     * The storage notice (`docs/dbus.md`), or `null` if the daemon is unreachable.
     *
     * @returns {Promise<string|null>}
     */
    async storageNotice() {
        const reply = await this.#call('org.freedesktop.DBus.Properties', 'Get',
            new GLib.Variant('(ss)', [HISTORY, 'StorageNotice']), '(v)');
        return reply === null ? null : reply.get_child_value(0).deepUnpack().deepUnpack();
    }

    /**
     * Activates an action of the Copyveil app (`popup`, `preferences`),
     * starting it if needed. Resolves to whether the app answered.
     *
     * @param {string} action
     * @returns {Promise<boolean>}
     */
    async activateApp(action) {
        try {
            await this.#bus.call(APP_ID, APP_PATH, 'org.freedesktop.Application', 'ActivateAction',
                new GLib.Variant('(sava{sv})', [action, [], {}]), null,
                Gio.DBusCallFlags.NONE, -1, null);
            return true;
        } catch {
            return false;
        }
    }

    async #call(iface, method, params, replyType) {
        try {
            return await this.#bus.call(BUS_NAME, OBJECT_PATH, iface, method, params,
                new GLib.VariantType(replyType), Gio.DBusCallFlags.NONE, -1, null);
        } catch {
            // Unreachable daemon, unknown Item, refused capture: the caller drops it.
            return null;
        }
    }
}
