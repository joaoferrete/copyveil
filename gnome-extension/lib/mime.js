// SPDX-License-Identifier: GPL-3.0-or-later
//
// Choosing what to read from the clipboard. Pure functions, no GNOME
// imports, so they are tested with `node --test`.

/** The MIME type password managers use to flag a secret (KDE convention). */
export const PASSWORD_MANAGER_HINT = 'x-kde-passwordManagerHint';

/** Text types, best first. `text/plain;charset=utf-8` is what GTK offers. */
const TEXT_TYPES = ['text/plain;charset=utf-8', 'UTF8_STRING', 'text/plain', 'STRING', 'TEXT'];

/**
 * Which types to read from a clipboard offer.
 *
 * Images win over text, since a copied image usually also offers a file
 * name as text. Rich text is read as `text/html` with a plain-text
 * fallback. Returns `null` when nothing on offer is something Copyveil
 * keeps (files, for example).
 *
 * @param {string[]} offered the MIME types the clipboard offers
 * @returns {{mime: string, fallback: string|null, hint: boolean}|null}
 */
export function chooseTypes(offered) {
    const hint = offered.includes(PASSWORD_MANAGER_HINT);
    const text = TEXT_TYPES.find(type => offered.includes(type)) ?? null;

    const images = offered.filter(type => type.startsWith('image/') && type.length > 'image/'.length);
    if (images.length > 0) {
        const mime = images.includes('image/png') ? 'image/png' : images[0];
        return {mime, fallback: null, hint};
    }
    if (offered.includes('text/html'))
        return {mime: 'text/html', fallback: text, hint};
    if (text !== null)
        return {mime: text, fallback: null, hint};
    return null;
}

/**
 * Whether a password-manager hint makes the content a Secret.
 *
 * The convention is the value `secret`. A hint whose value can't be read,
 * or reads as empty (Mutter's clipboard can hand it over that way), still
 * counts: a password manager offered the hint, so failing safe means
 * treating the content as a Secret. Only a readable, different value
 * doesn't count.
 *
 * @param {Uint8Array|null} value the bytes offered as the hint, or `null` if unreadable
 * @returns {boolean}
 */
export function hintSaysSecret(value) {
    if (value === null)
        return true;
    const text = new TextDecoder().decode(value).trim();
    return text === '' || text === 'secret';
}

/**
 * The MIME type to put an Item back on the clipboard with.
 *
 * Rich text goes back as its plain-text fallback, since a single-type
 * clipboard source can't offer both and every app accepts plain text.
 *
 * @param {{kind: string, mime: string}} summary the Item's summary from `History1.List`
 * @returns {string}
 */
export function pasteType(summary) {
    return summary.kind === 'image' ? summary.mime : 'text/plain;charset=utf-8';
}
