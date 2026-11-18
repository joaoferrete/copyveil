// SPDX-License-Identifier: GPL-3.0-or-later
//
// Which panel icon to show. Pure, so it's tested with `node --test`.

/**
 * The panel icon's state.
 *
 * A Secret in memory wins over a pause: the dot tells the user a secret is
 * still alive and the clipboard hasn't been cleared yet, which matters
 * even while capture is paused. It goes away at the Secret's Expiry.
 *
 * @param {{paused: boolean, sensitivities: string[]}} state
 *   whether capture is paused, and the Sensitivity of every listed Item
 * @returns {'secret'|'paused'|'normal'}
 */
export function iconState({paused, sensitivities}) {
    if (sensitivities.includes('secret'))
        return 'secret';
    return paused ? 'paused' : 'normal';
}

/** The icon file for each state, inside the extension's `icons/`. */
export const ICON_FILES = {
    normal: 'copyveil-symbolic.svg',
    paused: 'copyveil-paused-symbolic.svg',
    secret: 'copyveil-secret-symbolic.svg',
};
