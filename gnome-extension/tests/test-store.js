import {Store} from '../lib/store.js';
import {assert, equal} from './assert.js';

function window(id, title = 'a') {
    return {
        id, app: 'app', class: 'App', title, workspace: '1', monitor: 'eDP-1',
        focused: false, minimized: false, maximized: false, floating: true, fullscreen: false,
    };
}

export const tests = {
    'no-op updates keep the revision'() {
        const store = new Store();
        const locked = new Map([['session.locked', false]]);
        equal(store.updateFacts(locked).revision, 1);
        equal(store.updateFacts(locked, ['focus.app']), null);
        equal(store.updateWindows([window('1')]).revision, 2);
        equal(store.updateWindows([window('1')], ['2']), null);
        equal(store.revision, 2);
    },
    'invalid updates change nothing'() {
        const store = new Store();
        let failed = false;
        try {
            store.updateFacts(new Map([['appearance.scheme', 'sepia']]));
        } catch (_) {
            failed = true;
        }
        assert(failed);
        equal(store.revision, 0);
    },
    'upsert wins over remove'() {
        const store = new Store();
        store.updateWindows([window('1')]);
        const change = store.updateWindows([window('1', 'b')], ['1']);
        equal(change.upserted.map(item => item.title), ['b']);
        equal(change.removed, []);
    },
    'sync replaces the state'() {
        const store = new Store();
        store.syncFacts(new Map([['focus.app', 'a'], ['session.locked', false]]));
        const {change, rejected} = store.syncFacts(new Map([
            ['session.locked', true], ['appearance.scheme', 'sepia'],
        ]));
        equal(rejected, ['appearance.scheme']);
        equal([...change.changed], [['session.locked', true]]);
        equal(change.removed, ['focus.app']);
        store.syncWindows([window('2'), window('1')]);
        const windows = store.syncWindows([window('2', 'new')]);
        equal(windows.change.upserted.map(item => item.id), ['2']);
        equal(windows.change.removed, ['1']);
        equal(store.revision, 4);
    },
    'workspace records compare by value'() {
        const store = new Store();
        const records = () => [{id: '1', name: '1', monitor: 'eDP-1', active: true}];
        store.updateFacts(new Map([['workspaces', records()]]));
        equal(store.updateFacts(new Map([['workspaces', records()]])), null);
    },
};
