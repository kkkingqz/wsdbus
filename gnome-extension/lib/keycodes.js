// Keys by physical position. Clutter's virtual keyboard looks a keyval up in
// the current layout only, so `t` does nothing while a Cyrillic layout is
// active. Keys of the main block are therefore sent as evdev codes of their
// US layout position, the way GTK matches shortcuts across layouts; keys
// present in every layout (media, brightness) stay keyvals.

const LETTERS = 'qwertyuiop'.split('').map((key, index) => [key, 16 + index])
    .concat('asdfghjkl'.split('').map((key, index) => [key, 30 + index]))
    .concat('zxcvbnm'.split('').map((key, index) => [key, 44 + index]));

const DIGITS = '1234567890'.split('').map((key, index) => [key, 2 + index]);

const NAMED = [
    ['Escape', 1], ['minus', 12], ['equal', 13], ['BackSpace', 14], ['Tab', 15],
    ['ISO_Left_Tab', 15], ['bracketleft', 26], ['bracketright', 27], ['Return', 28],
    ['Control_L', 29], ['semicolon', 39], ['apostrophe', 40], ['grave', 41],
    ['Shift_L', 42], ['backslash', 43], ['comma', 51], ['period', 52], ['slash', 53],
    ['Shift_R', 54], ['KP_Multiply', 55], ['Alt_L', 56], ['space', 57],
    ['Caps_Lock', 58], ['Num_Lock', 69], ['Scroll_Lock', 70],
    ['KP_7', 71], ['KP_8', 72], ['KP_9', 73], ['KP_Subtract', 74], ['KP_4', 75],
    ['KP_5', 76], ['KP_6', 77], ['KP_Add', 78], ['KP_1', 79], ['KP_2', 80],
    ['KP_3', 81], ['KP_0', 82], ['KP_Decimal', 83], ['KP_Enter', 96],
    ['Control_R', 97], ['KP_Divide', 98], ['Print', 99], ['Alt_R', 100],
    ['ISO_Level3_Shift', 100], ['Home', 102], ['Up', 103], ['Prior', 104],
    ['Page_Up', 104], ['Left', 105], ['Right', 106], ['End', 107], ['Down', 108],
    ['Next', 109], ['Page_Down', 109], ['Insert', 110], ['Delete', 111],
    ['Pause', 119], ['Super_L', 125], ['Super_R', 126], ['Menu', 127],
];

const FUNCTION_KEYS = [
    ['F1', 59], ['F2', 60], ['F3', 61], ['F4', 62], ['F5', 63], ['F6', 64],
    ['F7', 65], ['F8', 66], ['F9', 67], ['F10', 68], ['F11', 87], ['F12', 88],
];

/** XKB keysym name to the evdev code of its US layout position. */
export const EVDEV_CODES = Object.freeze(Object.fromEntries([
    ...LETTERS,
    ...LETTERS.map(([key, code]) => [key.toUpperCase(), code]),
    ...DIGITS,
    ...NAMED,
    ...FUNCTION_KEYS,
]));

/**
 * How to press a key: `{code}` for an evdev code, `{keyval}` for a keyval
 * from `table` (Clutter's `KEY_*` constants), or null for an unknown name.
 */
export function resolveKey(name, table, keyvalForName) {
    const code = EVDEV_CODES[name];
    if (code !== undefined)
        return {code};
    const keyval = keyvalForName(name, table);
    return keyval === null ? null : {keyval};
}

/** A stable token of a resolved key, for tracking held keys. */
export function keyToken(key) {
    return key.code !== undefined ? `code:${key.code}` : `keyval:${key.keyval}`;
}

export function keyFromToken(token) {
    const [kind, value] = token.split(':');
    return kind === 'code' ? {code: Number(value)} : {keyval: Number(value)};
}
