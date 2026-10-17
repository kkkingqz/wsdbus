import {Store} from '../lib/store.js';
import {changeSignal, factVariant, stateBody, windowsBody} from '../lib/wire.js';
import {equal} from './assert.js';

function window(id) {
    return {
        id, app: 'app', class: 'App', title: 't', workspace: '1', monitor: 'eDP-1',
        focused: true, minimized: false, maximized: false, floating: true, fullscreen: false,
    };
}

export const tests = {
    'fact value types'() {
        equal(factVariant('focus.app', 'x').get_type_string(), 's');
        equal(factVariant('session.locked', true).get_type_string(), 'b');
        equal(factVariant('display.brightness', 1).get_type_string(), 'd');
        const records = factVariant('workspaces', [
            {id: '1', name: 'One', monitor: 'eDP-1', active: true},
            {id: '2', name: 'Two', monitor: 'eDP-1', active: false, mode: 'floating'},
        ]);
        equal(records.get_type_string(), 'aa{sv}');
        equal(records.recursiveUnpack(), [
            {active: true, id: '1', monitor: 'eDP-1', name: 'One'},
            {active: false, id: '2', mode: 'floating', monitor: 'eDP-1', name: 'Two'},
        ]);
    },
    'snapshot bodies'() {
        const store = new Store();
        store.updateFacts(new Map([['session.locked', false], ['display.brightness', 0.5]]));
        store.updateWindows([window('9'), window('10')]);
        const state = stateBody(store);
        equal(state.get_type_string(), '(ta{sv})');
        equal(state.recursiveUnpack(), [2, {'display.brightness': 0.5, 'session.locked': false}]);
        const windows = windowsBody(store);
        equal(windows.get_type_string(), '(taa{sv})');
        equal(windows.recursiveUnpack()[1].map(item => item.id), ['10', '9']);
    },
    'signals'() {
        const store = new Store();
        const [name, body] = changeSignal(store.updateFacts(new Map([['focus.app', 'a']])));
        equal(name, 'StateChanged');
        equal(body.get_type_string(), '(ta{sv}as)');
        equal(body.recursiveUnpack(), [1, {'focus.app': 'a'}, []]);
        const [windowsName, windowsBody] = changeSignal(store.updateWindows([window('1')]));
        equal(windowsName, 'WindowsChanged');
        equal(windowsBody.get_type_string(), '(taa{sv}as)');
        equal(Object.keys(windowsBody.recursiveUnpack()[1][0]).length, 11);
    },
};
