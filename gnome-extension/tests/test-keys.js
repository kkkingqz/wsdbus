import {HeldKeys} from '../lib/keys.js';
import {assert, equal} from './assert.js';

export const tests = {
    'repeated press and stray release'() {
        const held = new HeldKeys();
        assert(held.press(':1.5', 1));
        assert(!held.press(':1.5', 1));
        assert(!held.release(':1.6', 1));
        assert(held.release(':1.5', 1));
        assert(!held.holds(':1.5'));
    },
    'leaving releases in reverse order'() {
        const held = new HeldKeys();
        held.press(':1.5', 1);
        held.press(':1.5', 2);
        held.press(':1.6', 3);
        equal(held.releaseSender(':1.5'), [2, 1]);
        equal(held.releaseAll(), [[':1.6', 3]]);
        equal(held.releaseAll(), []);
    },
};
