// GLib.Variant encoding of facts (a{sv}) and windows (aa{sv}), as in
// `wire.rs` of wsdbus-contract.

import GLib from 'gi://GLib';

import {FACT_KINDS, WINDOW_KEYS} from './contract.js';

function workspaceRecord(workspace) {
    const record = {
        id: new GLib.Variant('s', workspace.id),
        name: new GLib.Variant('s', workspace.name),
        monitor: new GLib.Variant('s', workspace.monitor),
        active: new GLib.Variant('b', workspace.active),
    };
    if (workspace.mode !== undefined)
        record.mode = new GLib.Variant('s', workspace.mode);
    return record;
}

export function factVariant(name, value) {
    switch (FACT_KINDS[name]) {
    case 'bool':
        return new GLib.Variant('b', value);
    case 'number':
        return new GLib.Variant('d', value);
    case 'workspaces':
        return new GLib.Variant('aa{sv}', value.map(workspaceRecord));
    default:
        return new GLib.Variant('s', value);
    }
}

/** A Map of facts as an object of variants, the JS form of a{sv}. */
export function factsDict(facts) {
    const dict = {};
    for (const [name, value] of facts)
        dict[name] = factVariant(name, value);
    return dict;
}

export function windowDict(window) {
    const dict = {};
    for (const key of WINDOW_KEYS) {
        const value = window[key];
        dict[key] = new GLib.Variant(typeof value === 'boolean' ? 'b' : 's', value);
    }
    return dict;
}

/** Signal name and body of a store change. */
export function changeSignal(change) {
    if (change.kind === 'facts') {
        return ['StateChanged', new GLib.Variant('(ta{sv}as)',
            [change.revision, factsDict(change.changed), change.removed])];
    }
    return ['WindowsChanged', new GLib.Variant('(taa{sv}as)',
        [change.revision, change.upserted.map(windowDict), change.removed])];
}

export function stateBody(store) {
    return new GLib.Variant('(ta{sv})', [store.revision, factsDict(store.facts)]);
}

export function windowsBody(store) {
    const ids = [...store.windows.keys()].sort();
    return new GLib.Variant('(taa{sv})',
        [store.revision, ids.map(id => windowDict(store.windows.get(id)))]);
}
