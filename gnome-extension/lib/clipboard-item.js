// SPDX-License-Identifier: GPL-3.0-or-later
//
// Which Item the clipboard holds, so an Expiry can clear it. The extension
// keeps this one id and nothing else: no content, no History.

export class ClipboardItem {
    #id = null;
    #ownChanges = 0;

    /** The id of the Item on the clipboard, or `null` when unknown. */
    get id() {
        return this.#id;
    }

    /**
     * Records the Item the daemon returned for a capture.
     *
     * @param {bigint|number|null} id the Item id, or `null` if the capture was refused
     */
    captured(id) {
        this.#id = id;
    }

    /**
     * Announces that the extension itself is about to change the clipboard
     * (a paste or a clear), so the change isn't captured again.
     *
     * @param {bigint|number|null} id the Item now on the clipboard, or `null` after a clear
     */
    settingOwn(id) {
        this.#id = id;
        this.#ownChanges += 1;
    }

    /**
     * Called for every clipboard change. Returns whether to capture it:
     * false for a change the extension made itself.
     *
     * @returns {boolean}
     */
    shouldCapture() {
        if (this.#ownChanges > 0) {
            this.#ownChanges -= 1;
            return false;
        }
        return true;
    }

    /**
     * Whether an Expiry of `ids` means the clipboard must be cleared.
     *
     * @param {Array<bigint|number>} ids the Items that reached their Expiry
     * @returns {boolean}
     */
    expires(ids) {
        return this.#id !== null && ids.some(id => BigInt(id) === BigInt(this.#id));
    }
}
