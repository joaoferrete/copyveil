// SPDX-License-Identifier: GPL-3.0-or-later
//
// The Super+V hand-over. GNOME binds Super+V (and Super+M) to the
// notification list. Copyveil takes Super+V only when the user agreed in
// onboarding, leaves Super+M alone, and gives Super+V back on disable.

export const SUPER_V = '<Super>v';

/**
 * The notification list's bindings without Super+V, and whether it was there.
 *
 * @param {string[]} bindings `org.gnome.shell.keybindings toggle-message-tray`
 * @returns {{bindings: string[], removed: boolean}}
 */
export function withoutSuperV(bindings) {
    const kept = bindings.filter(binding => binding.toLowerCase() !== SUPER_V.toLowerCase());
    return {bindings: kept, removed: kept.length !== bindings.length};
}

/**
 * The notification list's bindings with Super+V given back, first, as GNOME
 * ships it.
 *
 * @param {string[]} bindings the current bindings
 * @returns {string[]}
 */
export function withSuperV(bindings) {
    if (bindings.some(binding => binding.toLowerCase() === SUPER_V.toLowerCase()))
        return bindings;
    return [SUPER_V, ...bindings];
}
