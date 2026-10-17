// wsdbus provider for GNOME Shell.

import GLib from 'gi://GLib';

import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

import {INTERFACE} from './lib/contract.js';
import {Provider} from './lib/shell.js';

export default class WsDbusExtension extends Extension {
    enable() {
        const path = GLib.build_filenamev([this.path, `${INTERFACE}.xml`]);
        const [, bytes] = GLib.file_get_contents(path);
        this._provider = new Provider(new TextDecoder().decode(bytes));
        this._provider.enable();
    }

    disable() {
        this._provider?.disable();
        this._provider = null;
    }
}
