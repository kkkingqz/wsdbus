// Keys held through `Key`, per caller, so the provider can release what a
// caller left pressed when it leaves the bus or the screen locks.

export class HeldKeys {
    constructor() {
        this._held = new Map();
    }

    /** Records a press; false when this caller already holds the key. */
    press(sender, key) {
        let keys = this._held.get(sender);
        if (!keys) {
            keys = [];
            this._held.set(sender, keys);
        }
        if (keys.includes(key))
            return false;
        keys.push(key);
        return true;
    }

    /** Records a release; false when this caller did not hold the key. */
    release(sender, key) {
        const keys = this._held.get(sender);
        const index = keys ? keys.indexOf(key) : -1;
        if (index < 0)
            return false;
        keys.splice(index, 1);
        if (keys.length === 0)
            this._held.delete(sender);
        return true;
    }

    holds(sender) {
        return this._held.has(sender);
    }

    /** Forgets a caller; returns its keys, last pressed first. */
    releaseSender(sender) {
        const keys = this._held.get(sender) ?? [];
        this._held.delete(sender);
        return keys.reverse();
    }

    /** Forgets every caller; returns [sender, key] pairs to release. */
    releaseAll() {
        const pairs = [];
        for (const sender of [...this._held.keys()]) {
            for (const key of this.releaseSender(sender))
                pairs.push([sender, key]);
        }
        return pairs;
    }
}
