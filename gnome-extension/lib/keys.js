// Keys held through `Key`, per caller, so the provider can release what a
// caller left pressed when it leaves the bus or the screen locks.

export class HeldKeys {
    constructor() {
        this._held = new Map();
    }

    /** Records a press; false when this caller already holds the key. */
    press(sender, keyval) {
        let keys = this._held.get(sender);
        if (!keys) {
            keys = [];
            this._held.set(sender, keys);
        }
        if (keys.includes(keyval))
            return false;
        keys.push(keyval);
        return true;
    }

    /** Records a release; false when this caller did not hold the key. */
    release(sender, keyval) {
        const keys = this._held.get(sender);
        const index = keys ? keys.indexOf(keyval) : -1;
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

    /** Forgets every caller; returns [sender, keyval] pairs to release. */
    releaseAll() {
        const pairs = [];
        for (const sender of [...this._held.keys()]) {
            for (const keyval of this.releaseSender(sender))
                pairs.push([sender, keyval]);
        }
        return pairs;
    }
}
