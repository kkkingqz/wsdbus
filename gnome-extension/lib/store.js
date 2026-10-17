// Provider state with one revision counter for facts and windows; the twin of
// `Store` in wsdbus-contract. Every effective update bumps the revision by one
// and returns one change to emit; updates without a difference return null.

import {factProblem, windowProblem} from './contract.js';

export function same(a, b) {
    if (a === b)
        return true;
    if (typeof a !== 'object' || typeof b !== 'object' || a === null || b === null)
        return false;
    if (Array.isArray(a) !== Array.isArray(b))
        return false;
    const keys = Object.keys(a);
    if (keys.length !== Object.keys(b).length)
        return false;
    return keys.every(key => Object.hasOwn(b, key) && same(a[key], b[key]));
}

export class Store {
    constructor() {
        this.revision = 0;
        this.facts = new Map();
        this.windows = new Map();
    }

    /**
     * Sets `changed` (a Map of name to value) and removes `removed`. A name in
     * both is set. Throws on an invalid fact without changing anything.
     */
    updateFacts(changed, removed = []) {
        for (const [name, value] of changed) {
            const problem = factProblem(name, value);
            if (problem)
                throw new Error(`invalid fact ${name}: ${problem}`);
        }
        const effective = new Map(
            [...changed].filter(([name, value]) => !same(this.facts.get(name), value)));
        const gone = [...new Set(removed)]
            .filter(name => this.facts.has(name) && !changed.has(name))
            .sort();
        if (effective.size === 0 && gone.length === 0)
            return null;
        for (const name of gone)
            this.facts.delete(name);
        for (const [name, value] of effective)
            this.facts.set(name, value);
        this.revision++;
        return {kind: 'facts', revision: this.revision, changed: effective, removed: gone};
    }

    /** Inserts or replaces windows and removes ids; an id in both is kept. */
    updateWindows(upserted, removed = []) {
        for (const window of upserted) {
            const problem = windowProblem(window);
            if (problem)
                throw new Error(`invalid window ${window.id}: ${problem}`);
        }
        const changed = new Map();
        for (const window of upserted)
            changed.set(window.id, window);
        for (const [id, window] of [...changed]) {
            if (same(this.windows.get(id), window))
                changed.delete(id);
        }
        const kept = new Set(upserted.map(window => window.id));
        const gone = [...new Set(removed)]
            .filter(id => this.windows.has(id) && !kept.has(id))
            .sort();
        if (changed.size === 0 && gone.length === 0)
            return null;
        for (const id of gone)
            this.windows.delete(id);
        for (const [id, window] of changed)
            this.windows.set(id, window);
        this.revision++;
        const ids = [...changed.keys()].sort();
        return {
            kind: 'windows',
            revision: this.revision,
            upserted: ids.map(id => changed.get(id)),
            removed: gone,
        };
    }

    /**
     * Makes the facts equal to `all`: invalid entries are dropped and their
     * names returned in `rejected`.
     */
    syncFacts(all) {
        const rejected = [];
        const valid = new Map();
        for (const [name, value] of all) {
            if (factProblem(name, value))
                rejected.push(name);
            else
                valid.set(name, value);
        }
        const removed = [...this.facts.keys()].filter(name => !valid.has(name));
        return {change: this.updateFacts(valid, removed), rejected};
    }

    /** Makes the windows equal to `all`, dropping invalid ones. */
    syncWindows(all) {
        const rejected = [];
        const valid = [];
        for (const window of all) {
            if (windowProblem(window))
                rejected.push(String(window.id));
            else
                valid.push(window);
        }
        const ids = new Set(valid.map(window => window.id));
        const removed = [...this.windows.keys()].filter(id => !ids.has(id));
        return {change: this.updateWindows(valid, removed), rejected};
    }
}
