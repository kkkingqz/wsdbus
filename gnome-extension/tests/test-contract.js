import {
    ACCENT_COLORS, admit, commandProblem, factProblem, keyvalForName,
    schemeFromColorScheme, gnomeCapabilities,
} from '../lib/contract.js';
import {assert, equal} from './assert.js';

const launch = {method: 'LaunchApp', id: 'org.gnome.Nautilus'};

function kind(rejection) {
    return rejection ? rejection.kind : null;
}

export const tests = {
    'order of checks'() {
        const all = gnomeCapabilities();
        equal(kind(admit({method: 'LaunchApp', id: ''}, 'sandboxed', [], true)), 'invalid-args');
        equal(kind(admit(launch, 'sandboxed', [], true)), 'denied');
        equal(kind(admit(launch, 'unverified', all, false)), 'denied');
        equal(kind(admit(launch, 'trusted', [], true)), 'unsupported');
        equal(kind(admit(launch, 'trusted', all, true)), 'locked');
        equal(kind(admit(launch, 'trusted', all, false)), null);
        const mode = {method: 'SetWorkspaceMode', id: '1', mode: 'floating'};
        equal(kind(admit(mode, 'trusted', all, false)), 'unsupported');
    },
    'lock blocks presses only'() {
        const all = gnomeCapabilities();
        const press = {method: 'Key', key: 'Escape', pressed: true};
        const release = {method: 'Key', key: 'Escape', pressed: false};
        equal(kind(admit(press, 'trusted', all, true)), 'locked');
        equal(kind(admit(release, 'trusted', all, true)), null);
        const close = {method: 'WindowAction', id: '3', action: 'close'};
        equal(kind(admit(close, 'trusted', all, true)), null);
        equal(kind(admit({method: 'SetBrightness', level: 0.5}, 'trusted', all, true)), null);
    },
    'argument checks'() {
        assert(commandProblem({method: 'Shortcut', keys: [], app: undefined}));
        assert(commandProblem({method: 'Shortcut', keys: Array(9).fill('a'), app: undefined}));
        assert(commandProblem({method: 'Shortcut', keys: ['Control L'], app: undefined}));
        assert(commandProblem({method: 'Shortcut', keys: ['t'], app: ''}));
        equal(commandProblem({method: 'Shortcut', keys: ['Control_L', 't'], app: 'x'}), null);
        assert(commandProblem({method: 'Key', key: '', pressed: true}));
        assert(commandProblem({method: 'SetBrightness', level: 1.5}));
        assert(commandProblem({method: 'SetBrightness', level: NaN}));
        equal(commandProblem({method: 'SetBrightness', level: 0}), null);
        assert(commandProblem({method: 'WindowAction', id: '1', action: 'explode'}));
        assert(commandProblem({method: 'SetWorkspaceMode', id: '1', mode: 'stacked'}));
        assert(commandProblem({method: 'Frobnicate'}));
    },
    'fact rules'() {
        equal(factProblem('appearance.scheme', 'dark'), null);
        assert(factProblem('appearance.scheme', 'default'));
        equal(factProblem('appearance.accent', '#3584e4'), null);
        assert(factProblem('appearance.accent', '#3584E4'));
        assert(factProblem('display.brightness', 1.5));
        assert(factProblem('focus.app', ''));
        assert(factProblem('session.locked', 'no'));
        assert(factProblem('app.id', 'x'));
        assert(factProblem('workspaces', [{id: '1'}, {id: '1'}]));
        equal(factProblem('workspaces', [{id: '1', name: '1', monitor: 'eDP-1', active: true}]), null);
    },
    'accent colors are valid facts'() {
        for (const color of Object.values(ACCENT_COLORS))
            equal(factProblem('appearance.accent', color), null, color);
    },
    'color scheme'() {
        equal(schemeFromColorScheme('prefer-dark'), 'dark');
        equal(schemeFromColorScheme('prefer-light'), 'light');
        equal(schemeFromColorScheme('default'), 'light');
    },
    'keysym names'() {
        const table = {KEY_Escape: 0xff1b, KEY_AudioPlay: 0x1008ff14, KEY_t: 0x74};
        equal(keyvalForName('Escape', table), 0xff1b);
        equal(keyvalForName('XF86AudioPlay', table), 0x1008ff14);
        equal(keyvalForName('t', table), 0x74);
        equal(keyvalForName('Nope', table), null);
        equal(keyvalForName('XF86Nope', table), null);
    },
};
