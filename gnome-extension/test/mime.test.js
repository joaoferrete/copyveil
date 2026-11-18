// SPDX-License-Identifier: GPL-3.0-or-later
import {test} from 'node:test';
import assert from 'node:assert/strict';
import {PASSWORD_MANAGER_HINT, chooseTypes, hintSaysSecret, pasteType} from '../lib/mime.js';

test('plain text prefers the UTF-8 type GTK offers', () => {
    assert.deepEqual(chooseTypes(['TEXT', 'text/plain;charset=utf-8', 'UTF8_STRING']), {
        mime: 'text/plain;charset=utf-8', fallback: null, hint: false,
    });
    assert.deepEqual(chooseTypes(['STRING', 'UTF8_STRING']), {mime: 'UTF8_STRING', fallback: null, hint: false});
    assert.deepEqual(chooseTypes(['TEXT']), {mime: 'TEXT', fallback: null, hint: false});
});

test('rich text is read as HTML with a plain-text fallback', () => {
    assert.deepEqual(chooseTypes(['text/html', 'text/plain']), {
        mime: 'text/html', fallback: 'text/plain', hint: false,
    });
    assert.deepEqual(chooseTypes(['text/html']), {mime: 'text/html', fallback: null, hint: false});
});

test('images win over text, and PNG over other images', () => {
    assert.deepEqual(chooseTypes(['text/plain', 'image/jpeg', 'image/png']), {
        mime: 'image/png', fallback: null, hint: false,
    });
    assert.equal(chooseTypes(['image/webp', 'text/plain']).mime, 'image/webp');
    assert.equal(chooseTypes(['image/']), null);
});

test('nothing Copyveil keeps yields null', () => {
    assert.equal(chooseTypes(['text/uri-list', 'x-special/gnome-copied-files']), null);
    assert.equal(chooseTypes([]), null);
});

test('the password manager hint is reported', () => {
    assert.equal(chooseTypes(['text/plain', PASSWORD_MANAGER_HINT]).hint, true);
    const encoder = new TextEncoder();
    assert.equal(hintSaysSecret(encoder.encode('secret')), true);
    assert.equal(hintSaysSecret(encoder.encode('secret\n')), true);
    assert.equal(hintSaysSecret(encoder.encode('public')), false);
    // Fail safe: an unreadable or empty hint still means a Secret.
    assert.equal(hintSaysSecret(new Uint8Array()), true);
    assert.equal(hintSaysSecret(null), true);
});

test('items go back as plain text, images as themselves', () => {
    assert.equal(pasteType({kind: 'text', mime: ''}), 'text/plain;charset=utf-8');
    assert.equal(pasteType({kind: 'rich-text', mime: ''}), 'text/plain;charset=utf-8');
    assert.equal(pasteType({kind: 'image', mime: 'image/png'}), 'image/png');
});
