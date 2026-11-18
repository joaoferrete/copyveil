// SPDX-License-Identifier: GPL-3.0-or-later
import {test} from 'node:test';
import assert from 'node:assert/strict';
import {ICON_FILES, iconState} from '../lib/icon-state.js';

test('the icon shows a Secret in memory, even while paused', () => {
    assert.equal(iconState({paused: false, sensitivities: ['plain', 'secret']}), 'secret');
    assert.equal(iconState({paused: true, sensitivities: ['secret']}), 'secret');
});

test('otherwise it shows whether capture is paused', () => {
    assert.equal(iconState({paused: true, sensitivities: ['plain', 'suspected']}), 'paused');
    assert.equal(iconState({paused: false, sensitivities: ['suspected']}), 'normal');
    assert.equal(iconState({paused: false, sensitivities: []}), 'normal');
});

test('every state has an icon file', () => {
    for (const state of ['normal', 'paused', 'secret'])
        assert.match(ICON_FILES[state], /-symbolic\.svg$/);
});
