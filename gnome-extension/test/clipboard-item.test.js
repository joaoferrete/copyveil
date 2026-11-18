// SPDX-License-Identifier: GPL-3.0-or-later
import {test} from 'node:test';
import assert from 'node:assert/strict';
import {ClipboardItem} from '../lib/clipboard-item.js';

test('an expiry clears the clipboard only for the Item on it', () => {
    const item = new ClipboardItem();
    assert.equal(item.expires([1n]), false);
    item.captured(7n);
    assert.equal(item.expires([3n, 7n]), true);
    assert.equal(item.expires([3n]), false);
    item.captured(8);
    assert.equal(item.expires([8n]), true);
    item.captured(null);
    assert.equal(item.expires([8n]), false);
});

test('changes the extension makes itself are not captured again', () => {
    const item = new ClipboardItem();
    assert.equal(item.shouldCapture(), true);
    item.settingOwn(4n);
    assert.equal(item.id, 4n);
    assert.equal(item.shouldCapture(), false);
    assert.equal(item.shouldCapture(), true);

    item.settingOwn(null);
    item.settingOwn(5n);
    assert.equal(item.shouldCapture(), false);
    assert.equal(item.shouldCapture(), false);
    assert.equal(item.shouldCapture(), true);
});
