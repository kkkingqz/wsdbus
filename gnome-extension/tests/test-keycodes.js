import {keyvalForName} from '../lib/contract.js';
import {EVDEV_CODES, keyFromToken, keyToken, resolveKey} from '../lib/keycodes.js';
import {equal} from './assert.js';

const table = {KEY_t: 0x74, KEY_Escape: 0xff1b, KEY_AudioPlay: 0x1008ff14};

export const tests = {
    'main block keys go by position'() {
        equal(resolveKey('t', table, keyvalForName), {code: 20});
        equal(resolveKey('T', table, keyvalForName), {code: 20});
        equal(resolveKey('Control_L', table, keyvalForName), {code: 29});
        equal(resolveKey('Shift_L', table, keyvalForName), {code: 42});
        equal(resolveKey('Super_L', table, keyvalForName), {code: 125});
        equal(resolveKey('F5', table, keyvalForName), {code: 63});
        equal(resolveKey('F12', table, keyvalForName), {code: 88});
        equal(resolveKey('Return', table, keyvalForName), {code: 28});
    },
    'other keys go by keyval'() {
        equal(resolveKey('XF86AudioPlay', table, keyvalForName), {keyval: 0x1008ff14});
        equal(resolveKey('NoSuchKey', table, keyvalForName), null);
    },
    'US positions'() {
        equal(['q', 'a', 'z', 'm', '1', '0', 'space'].map(key => EVDEV_CODES[key]),
            [16, 30, 44, 50, 2, 11, 57]);
    },
    'tokens round trip'() {
        for (const key of [{code: 20}, {keyval: 0x1008ff14}])
            equal(keyFromToken(keyToken(key)), key);
    },
};
