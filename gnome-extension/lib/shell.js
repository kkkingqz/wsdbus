// The GNOME provider: reads facts and windows from GNOME Shell, serves
// Desktop1 and runs commands. Rules live in contract.js, store.js, wire.js
// and keys.js; this file only connects them to the Shell.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {getInputSourceManager} from 'resource:///org/gnome/shell/ui/status/keyboard.js';

import {
    ACCENT_COLORS, BUS_NAME, OBJECT_PATH, Rejection, VERSION, admit,
    gnomeCapabilities, keyvalForName, normalizeAppId, schemeFromColorScheme,
} from './contract.js';
import {keyFromToken, keyToken, resolveKey} from './keycodes.js';
import {HeldKeys} from './keys.js';
import {callerVerdict} from './probe.js';
import {Store} from './store.js';
import {changeSignal, stateBody, windowsBody} from './wire.js';

const WINDOW_TYPES = [
    Meta.WindowType.NORMAL,
    Meta.WindowType.DIALOG,
    Meta.WindowType.MODAL_DIALOG,
];

const WINDOW_SIGNALS = [
    'notify::title', 'notify::wm-class', 'notify::minimized',
    'notify::maximized-horizontally', 'notify::maximized-vertically',
    'notify::fullscreen', 'notify::appears-focused', 'notify::skip-taskbar',
    'notify::on-all-workspaces', 'notify::main-monitor', 'workspace-changed',
];

const INTERFACE_KEYS = ['color-scheme', 'accent-color', 'font-name'];

function now() {
    return global.get_current_time();
}

export class Provider {
    constructor(interfaceXml) {
        this._xml = interfaceXml;
    }

    enable() {
        this._store = new Store();
        this._held = new HeldKeys();
        this._senderWatches = new Map();
        this._signals = [];
        this._windows = new Map();
        this._capabilities = gnomeCapabilities();
        this._tracker = Shell.WindowTracker.get_default();
        this._interfaceSettings = new Gio.Settings({schema_id: 'org.gnome.desktop.interface'});
        this._wmSettings = new Gio.Settings({schema_id: 'org.gnome.desktop.wm.preferences'});
        this._brightnessScale = null;
        this._brightnessId = 0;
        this._syncId = 0;
        this._locked = Main.sessionMode.isLocked;

        const sync = () => this._schedule();
        this._connect(global.display, 'window-created', (_display, window) => {
            this._track(window);
            this._schedule();
        });
        this._connect(global.display, 'notify::focus-window', sync);
        this._connect(this._tracker, 'notify::focus-app', sync);
        this._connect(this._tracker, 'tracked-windows-changed', sync);
        const workspaces = global.workspace_manager;
        for (const name of ['workspace-added', 'workspace-removed', 'active-workspace-changed', 'workspaces-reordered'])
            this._connect(workspaces, name, sync);
        this._connect(Main.layoutManager, 'monitors-changed', sync);
        this._connect(Main.sessionMode, 'updated', () => this._sessionChanged());
        for (const key of INTERFACE_KEYS)
            this._connect(this._interfaceSettings, `changed::${key}`, sync);
        this._connect(this._wmSettings, 'changed::workspace-names', sync);
        this._connect(getInputSourceManager(), 'current-source-changed', sync);
        if (Main.brightnessManager) {
            this._connect(Main.brightnessManager, 'changed', () => this._watchBrightness());
            this._watchBrightness();
        }
        for (const actor of global.get_window_actors())
            this._track(actor.meta_window);

        this._exported = Gio.DBusExportedObject.wrapJSObject(this._xml, this._service());
        this._exported.export(Gio.DBus.session, OBJECT_PATH);
        this._sync();
        this._nameId = Gio.bus_own_name_on_connection(Gio.DBus.session, BUS_NAME,
            Gio.BusNameOwnerFlags.NONE, null, null);
    }

    disable() {
        this._releaseAllKeys();
        Gio.bus_unown_name(this._nameId);
        this._exported.unexport();
        this._exported = null;
        for (const id of this._senderWatches.values())
            Gio.bus_unwatch_name(id);
        this._senderWatches.clear();
        if (this._syncId)
            GLib.source_remove(this._syncId);
        this._syncId = 0;
        this._unwatchBrightness();
        for (const [object, id] of this._signals)
            object.disconnect(id);
        this._signals = [];
        for (const window of [...this._windows.keys()])
            this._untrack(window);
        this._virtualKeyboard = null;
        this._interfaceSettings = null;
        this._wmSettings = null;
        this._store = null;
    }

    _connect(object, signal, handler) {
        this._signals.push([object, object.connect(signal, handler)]);
    }

    // State

    _track(window) {
        if (!window || this._windows.has(window))
            return;
        const ids = [];
        for (const signal of WINDOW_SIGNALS)
            ids.push(window.connect(signal, () => this._schedule()));
        ids.push(window.connect('unmanaged', () => {
            this._untrack(window);
            this._schedule();
        }));
        this._windows.set(window, ids);
    }

    _untrack(window) {
        for (const id of this._windows.get(window) ?? [])
            window.disconnect(id);
        this._windows.delete(window);
    }

    _watchBrightness() {
        const scale = Main.brightnessManager.globalScale;
        if (scale === this._brightnessScale)
            return;
        this._unwatchBrightness();
        this._brightnessScale = scale;
        if (scale)
            this._brightnessId = scale.connect('notify::value', () => this._schedule());
        this._schedule();
    }

    _unwatchBrightness() {
        if (this._brightnessScale && this._brightnessId)
            this._brightnessScale.disconnect(this._brightnessId);
        this._brightnessScale = null;
        this._brightnessId = 0;
    }

    _sessionChanged() {
        const locked = Main.sessionMode.isLocked;
        if (locked && !this._locked)
            this._releaseAllKeys();
        this._locked = locked;
        this._schedule();
    }

    /** Collapses bursts of Shell signals into one sync. */
    _schedule() {
        if (this._syncId || !this._store)
            return;
        this._syncId = GLib.idle_add(GLib.PRIORITY_DEFAULT_IDLE, () => {
            this._syncId = 0;
            this._sync();
            return GLib.SOURCE_REMOVE;
        });
    }

    _sync() {
        const facts = this._store.syncFacts(this._facts());
        const windows = this._store.syncWindows(this._windowRecords());
        for (const name of [...facts.rejected, ...windows.rejected])
            console.warn(`wsdbus: dropped invalid ${name}`);
        for (const change of [facts.change, windows.change]) {
            if (change)
                this._exported.emit_signal(...changeSignal(change));
        }
    }

    _connector(index) {
        try {
            const manager = global.backend.get_monitor_manager();
            for (const logical of manager.get_logical_monitors()) {
                const [monitor] = logical.get_monitors();
                if (logical.get_number() === index && monitor)
                    return monitor.get_connector();
            }
        } catch (error) {
            console.debug(`wsdbus: no monitor connector: ${error.message}`);
        }
        return index >= 0 ? String(index) : '';
    }

    _appId(window) {
        const app = this._tracker.get_window_app(window);
        return normalizeAppId(app?.get_id() ?? null, window.get_wm_class());
    }

    _facts() {
        const facts = new Map();
        const focus = global.display.focus_window;
        if (focus) {
            facts.set('focus.app', this._appId(focus));
            const wmClass = focus.get_wm_class();
            if (wmClass)
                facts.set('focus.class', wmClass);
            facts.set('focus.fullscreen', focus.fullscreen);
        }
        facts.set('session.locked', Main.sessionMode.isLocked);
        const settings = this._interfaceSettings;
        facts.set('appearance.scheme', schemeFromColorScheme(settings.get_string('color-scheme')));
        const accent = ACCENT_COLORS[settings.get_string('accent-color')];
        if (accent)
            facts.set('appearance.accent', accent);
        const font = settings.get_string('font-name');
        if (font)
            facts.set('appearance.font', font);

        const manager = global.workspace_manager;
        const active = manager.get_active_workspace_index();
        const names = this._wmSettings.get_strv('workspace-names');
        const monitor = this._connector(global.display.get_primary_monitor());
        const records = [];
        for (let index = 0; index < manager.n_workspaces; index++) {
            records.push({
                id: String(index + 1),
                name: names[index] || String(index + 1),
                monitor,
                active: index === active,
            });
        }
        facts.set('workspaces', records);
        facts.set('workspace.active', String(active + 1));

        const level = this._brightnessScale?.value;
        if (typeof level === 'number')
            facts.set('display.brightness', Math.min(1, Math.max(0, level)));
        const source = getInputSourceManager().currentSource;
        if (source?.id)
            facts.set('keyboard.layout', source.id);
        return facts;
    }

    _interesting(window) {
        return !window.skip_taskbar && WINDOW_TYPES.includes(window.get_window_type());
    }

    _windowRecords() {
        const records = [];
        for (const window of this._windows.keys()) {
            if (!this._interesting(window))
                continue;
            const workspace = window.get_workspace();
            records.push({
                id: String(window.get_stable_sequence()),
                app: this._appId(window),
                class: window.get_wm_class() ?? '',
                title: window.get_title() ?? '',
                workspace: window.is_on_all_workspaces() || !workspace ? '' : String(workspace.index() + 1),
                monitor: this._connector(window.get_monitor()),
                focused: window.has_focus(),
                minimized: window.minimized,
                maximized: window.is_maximized(),
                floating: true,
                fullscreen: window.fullscreen,
            });
        }
        return records;
    }

    _windowById(id) {
        for (const window of this._windows.keys()) {
            if (String(window.get_stable_sequence()) === id && this._interesting(window))
                return window;
        }
        return null;
    }

    // D-Bus

    _service() {
        const provider = this;
        const run = (invocation, command) => {
            provider._run(invocation, command).catch(error => {
                logError(error, 'wsdbus: command failed');
                invocation.return_dbus_error('org.freedesktop.DBus.Error.Failed', error.message);
            });
        };
        return {
            get Version() {
                return VERSION;
            },
            get Provider() {
                return 'gnome';
            },
            get Capabilities() {
                return provider._capabilities;
            },
            GetStateAsync(_params, invocation) {
                invocation.return_value(stateBody(provider._store));
            },
            GetWindowsAsync(_params, invocation) {
                invocation.return_value(windowsBody(provider._store));
            },
            ShortcutAsync([keys, app], invocation) {
                run(invocation, {method: 'Shortcut', keys, app: app === '' ? undefined : app});
            },
            LaunchAppAsync([id], invocation) {
                run(invocation, {method: 'LaunchApp', id});
            },
            ActivateWorkspaceAsync([id], invocation) {
                run(invocation, {method: 'ActivateWorkspace', id});
            },
            WindowActionAsync([id, action], invocation) {
                run(invocation, {method: 'WindowAction', id, action});
            },
            SetWorkspaceModeAsync([id, mode], invocation) {
                run(invocation, {method: 'SetWorkspaceMode', id, mode});
            },
            KeyAsync([key, pressed], invocation) {
                run(invocation, {method: 'Key', key, pressed});
            },
            SetBrightnessAsync([level], invocation) {
                run(invocation, {method: 'SetBrightness', level});
            },
        };
    }

    async _run(invocation, command) {
        const sender = invocation.get_sender();
        const verdict = await callerVerdict(invocation.get_connection(), sender);
        if (!this._store)
            throw new Error('the provider is disabled');
        try {
            const rejection = admit(command, verdict, this._capabilities, Main.sessionMode.isLocked);
            if (rejection)
                throw rejection;
            this._perform(command, sender);
            invocation.return_value(null);
        } catch (error) {
            if (!(error instanceof Rejection))
                throw error;
            invocation.return_dbus_error(error.dbusName, error.message);
        }
    }

    _perform(command, sender) {
        switch (command.method) {
        case 'Shortcut':
            this._shortcut(command);
            break;
        case 'Key':
            this._key(command, sender);
            break;
        case 'LaunchApp': {
            const app = Shell.AppSystem.get_default().lookup_app(`${command.id}.desktop`);
            if (!app)
                throw new Rejection('invalid-args', `no app ${command.id}`);
            app.activate();
            break;
        }
        case 'ActivateWorkspace': {
            const index = /^[1-9][0-9]*$/.test(command.id) ? Number(command.id) - 1 : -1;
            const workspace = index >= 0 ? global.workspace_manager.get_workspace_by_index(index) : null;
            if (!workspace)
                throw new Rejection('invalid-args', `no workspace ${command.id}`);
            workspace.activate(now());
            break;
        }
        case 'WindowAction':
            this._windowAction(command);
            break;
        case 'SetBrightness':
            if (!this._brightnessScale)
                throw new Rejection('unsupported', 'no brightness control');
            this._brightnessScale.value = command.level;
            break;
        default:
            throw new Rejection('unsupported', `${command.method} is not supported by this provider`);
        }
    }

    _resolve(names) {
        return names.map(name => {
            const key = resolveKey(name, Clutter, keyvalForName);
            if (!key)
                throw new Rejection('invalid-args', `unknown key ${name}`);
            return key;
        });
    }

    _keyboard() {
        if (!this._virtualKeyboard) {
            const seat = Clutter.get_default_backend().get_default_seat();
            this._virtualKeyboard = seat.create_virtual_device(Clutter.InputDeviceType.KEYBOARD_DEVICE);
        }
        return this._virtualKeyboard;
    }

    _notifyKey(key, pressed) {
        const state = pressed ? Clutter.KeyState.PRESSED : Clutter.KeyState.RELEASED;
        if (key.code !== undefined)
            this._keyboard().notify_key(GLib.get_monotonic_time(), key.code, state);
        else
            this._keyboard().notify_keyval(GLib.get_monotonic_time(), key.keyval, state);
    }

    _shortcut({keys, app}) {
        const resolved = this._resolve(keys);
        const focus = global.display.focus_window;
        if (app !== undefined && (!focus || this._appId(focus) !== app))
            return;
        for (const key of resolved)
            this._notifyKey(key, true);
        for (const key of [...resolved].reverse())
            this._notifyKey(key, false);
    }

    _key({key, pressed}, sender) {
        const [resolved] = this._resolve([key]);
        const token = keyToken(resolved);
        if (pressed) {
            if (this._held.press(sender, token)) {
                this._watchSender(sender);
                this._notifyKey(resolved, true);
            }
        } else if (this._held.release(sender, token)) {
            this._notifyKey(resolved, false);
        }
    }

    /** Releases a caller's keys when it leaves the bus. */
    _watchSender(sender) {
        if (this._senderWatches.has(sender))
            return;
        const id = Gio.bus_watch_name_on_connection(Gio.DBus.session, sender,
            Gio.BusNameWatcherFlags.NONE, null, () => {
                for (const token of this._held.releaseSender(sender))
                    this._notifyKey(keyFromToken(token), false);
                Gio.bus_unwatch_name(id);
                this._senderWatches.delete(sender);
            });
        this._senderWatches.set(sender, id);
    }

    _releaseAllKeys() {
        for (const [, token] of this._held.releaseAll())
            this._notifyKey(keyFromToken(token), false);
    }

    _windowAction({id, action}) {
        const window = this._windowById(id);
        if (!window)
            throw new Rejection('stale', `no window ${id}`);
        switch (action) {
        case 'activate':
            Main.activateWindow(window, now());
            break;
        case 'close':
            window.delete(now());
            break;
        case 'minimize':
            if (!window.can_minimize())
                throw new Rejection('unsupported', 'the window cannot be minimized');
            window.minimize();
            break;
        case 'restore':
            window.unminimize();
            break;
        case 'toggle-maximize':
            if (window.is_maximized())
                window.unmaximize();
            else if (window.can_maximize())
                window.maximize();
            else
                throw new Rejection('unsupported', 'the window cannot be maximized');
            break;
        }
    }
}
