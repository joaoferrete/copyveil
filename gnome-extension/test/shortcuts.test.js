// SPDX-License-Identifier: GPL-3.0-or-later
import {test} from 'node:test';
import assert from 'node:assert/strict';
import {withSuperV, withoutSuperV} from '../lib/shortcuts.js';

test('taking Super+V leaves Super+M to the notification list', () => {
    assert.deepEqual(withoutSuperV(['<Super>v', '<Super>m']), {bindings: ['<Super>m'], removed: true});
    assert.deepEqual(withoutSuperV(['<Super>V']), {bindings: [], removed: true});
    assert.deepEqual(withoutSuperV(['<Super>m']), {bindings: ['<Super>m'], removed: false});
});

test('giving Super+V back puts it first, once', () => {
    assert.deepEqual(withSuperV(['<Super>m']), ['<Super>v', '<Super>m']);
    assert.deepEqual(withSuperV(['<Super>v', '<Super>m']), ['<Super>v', '<Super>m']);
    assert.deepEqual(withSuperV([]), ['<Super>v']);
});
