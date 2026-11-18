// SPDX-License-Identifier: GPL-3.0-or-later
//
// Reading and writing the clipboard through Mutter's selection. Content is
// read into memory only long enough to send it to the daemon.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';

import {PASSWORD_MANAGER_HINT, chooseTypes, hintSaysSecret} from './lib/mime.js';

/** Larger offers are dropped rather than read: a History is not a file store. */
export const MAX_CAPTURE_BYTES = 32 * 1024 * 1024;

const CLIPBOARD = Meta.SelectionType.SELECTION_CLIPBOARD;

Gio._promisify(Meta.Selection.prototype, 'transfer_async');

export class Clipboard {
    #selection = global.display.get_selection();
    #handler = null;

    /**
     * Calls `onChange(read)` after every clipboard change, where `read()`
     * returns the offer to capture, or `null` when there is nothing to keep.
     *
     * @param {(read: () => Promise<object|null>) => void} onChange
     */
    watch(onChange) {
        this.#handler = this.#selection.connect('owner-changed', (_selection, type) => {
            if (type === CLIPBOARD)
                onChange(() => this.#read());
        });
    }

    /** Stops watching. */
    unwatch() {
        if (this.#handler !== null)
            this.#selection.disconnect(this.#handler);
        this.#handler = null;
    }

    /**
     * Puts `bytes` on the clipboard as `mime`.
     *
     * @param {string} mime
     * @param {Uint8Array} bytes
     */
    set(mime, bytes) {
        const source = Meta.SelectionSourceMemory.new(mime, new GLib.Bytes(bytes));
        this.#selection.set_owner(CLIPBOARD, source);
    }

    /** Replaces the clipboard with empty text, after a Secret's Expiry. */
    clear() {
        this.set('text/plain;charset=utf-8', new Uint8Array(0));
    }

    async #read() {
        const types = chooseTypes(this.#selection.get_mimetypes(CLIPBOARD));
        if (types === null)
            return null;
        const content = await this.#transfer(types.mime);
        if (content === null || content.length === 0)
            return null;
        const fallback = types.fallback === null ? new Uint8Array(0) : await this.#transfer(types.fallback);
        const hint = types.hint && hintSaysSecret(await this.#transfer(PASSWORD_MANAGER_HINT));
        return {
            mime: types.mime,
            content,
            fallback: fallback ?? new Uint8Array(0),
            hint,
        };
    }

    async #transfer(mime) {
        const stream = Gio.MemoryOutputStream.new_resizable();
        try {
            await this.#selection.transfer_async(CLIPBOARD, mime, MAX_CAPTURE_BYTES + 1, stream, null);
            stream.close(null);
        } catch {
            return null;
        }
        const bytes = stream.steal_as_bytes().toArray();
        return bytes.length > MAX_CAPTURE_BYTES ? null : bytes;
    }
}
