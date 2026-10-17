// The wsdbus Desktop1 contract for the GNOME provider: names, fact rules,
// command checks, the app.id rule and the sandbox rule. No GNOME Shell
// imports; spec/README.md is normative and wsdbus-contract is the Rust twin.

export const BUS_NAME = 'io.github.kkkingqz.WsDbus';
export const OBJECT_PATH = '/io/github/kkkingqz/WsDbus';
export const INTERFACE = 'io.github.kkkingqz.WsDbus.Desktop1';
export const VERSION = 1;
export const CAPABILITY_WINDOWS = 'windows';
export const MAX_SHORTCUT_KEYS = 8;

export const FACT_KINDS = Object.freeze({
    'focus.app': 'text',
    'focus.class': 'text',
    'focus.fullscreen': 'bool',
    'session.locked': 'bool',
    'appearance.scheme': 'text',
    'appearance.accent': 'text',
    'appearance.font': 'text',
    'workspaces': 'workspaces',
    'workspace.active': 'text',
    'display.brightness': 'number',
    'keyboard.layout': 'text',
});

export const METHODS = Object.freeze([
    'Shortcut', 'LaunchApp', 'ActivateWorkspace', 'WindowAction',
    'SetWorkspaceMode', 'Key', 'SetBrightness',
]);

export const WINDOW_ACTIONS = Object.freeze([
    'activate', 'close', 'minimize', 'restore', 'toggle-maximize',
]);

export const WORKSPACE_MODES = Object.freeze(['tiling', 'floating']);

export const ERRORS = Object.freeze({
    unsupported: 'io.github.kkkingqz.WsDbus.Error.Unsupported',
    stale: 'io.github.kkkingqz.WsDbus.Error.StaleWindow',
    locked: 'io.github.kkkingqz.WsDbus.Error.Locked',
    denied: 'io.github.kkkingqz.WsDbus.Error.Denied',
    'invalid-args': 'org.freedesktop.DBus.Error.InvalidArgs',
});

/** A rejected command; `kind` is a key of ERRORS. */
export class Rejection extends Error {
    constructor(kind, message) {
        super(message);
        this.kind = kind;
    }

    get dbusName() {
        return ERRORS[this.kind];
    }
}

const WINDOW_TEXT = ['id', 'app', 'class', 'title', 'workspace', 'monitor'];
const WINDOW_FLAGS = ['focused', 'minimized', 'maximized', 'floating', 'fullscreen'];
export const WINDOW_KEYS = Object.freeze([...WINDOW_TEXT, ...WINDOW_FLAGS]);

function isHexColor(text) {
    return /^#[0-9a-f]{6}$/.test(text);
}

/** Returns null when the value is valid, otherwise the reason. */
export function factProblem(name, value) {
    const kind = FACT_KINDS[name];
    if (!kind)
        return 'unknown fact';
    switch (kind) {
    case 'bool':
        return typeof value === 'boolean' ? null : 'wrong value type';
    case 'number':
        if (typeof value !== 'number')
            return 'wrong value type';
        return Number.isFinite(value) && value >= 0 && value <= 1
            ? null : 'expected a number from 0.0 to 1.0';
    case 'workspaces': {
        if (!Array.isArray(value))
            return 'wrong value type';
        const ids = new Set();
        for (const record of value) {
            if (typeof record.id !== 'string' || !record.id)
                return 'empty workspace id';
            if (ids.has(record.id))
                return 'duplicate workspace id';
            ids.add(record.id);
            if (record.mode !== undefined && !WORKSPACE_MODES.includes(record.mode))
                return 'unknown workspace mode';
        }
        return null;
    }
    default:
        if (typeof value !== 'string')
            return 'wrong value type';
        if (!value)
            return 'empty text';
        if (name === 'appearance.scheme' && value !== 'light' && value !== 'dark')
            return 'expected light or dark';
        if (name === 'appearance.accent' && !isHexColor(value))
            return 'expected #rrggbb in lower case';
        return null;
    }
}

export function windowProblem(window) {
    for (const key of WINDOW_TEXT) {
        if (typeof window[key] !== 'string')
            return `${key} must be text`;
    }
    for (const key of WINDOW_FLAGS) {
        if (typeof window[key] !== 'boolean')
            return `${key} must be a boolean`;
    }
    if (!window.id)
        return 'empty id';
    if (!window.app)
        return 'empty app';
    return null;
}

/** The capabilities of the GNOME provider: everything but workspace modes. */
export function gnomeCapabilities() {
    return [
        ...Object.keys(FACT_KINDS),
        CAPABILITY_WINDOWS,
        ...METHODS.filter(method => method !== 'SetWorkspaceMode'),
    ];
}

function validKeyName(key) {
    return typeof key === 'string' && key.length > 0 && !/\s/.test(key);
}

/** Argument check; returns null or the reason for InvalidArgs. */
export function commandProblem(command) {
    switch (command.method) {
    case 'Shortcut':
        if (command.keys.length < 1 || command.keys.length > MAX_SHORTCUT_KEYS)
            return `expected 1 to ${MAX_SHORTCUT_KEYS} keys`;
        if (!command.keys.every(validKeyName))
            return 'empty key name or key name with spaces';
        if (command.app === '')
            return 'empty app; use no app instead';
        return null;
    case 'Key':
        return validKeyName(command.key) ? null : 'empty key name or key name with spaces';
    case 'SetBrightness':
        return Number.isFinite(command.level) && command.level >= 0 && command.level <= 1
            ? null : 'expected a level from 0.0 to 1.0';
    case 'WindowAction':
        if (!command.id)
            return 'empty id';
        return WINDOW_ACTIONS.includes(command.action) ? null : `unknown action ${command.action}`;
    case 'SetWorkspaceMode':
        if (!command.id)
            return 'empty id';
        return WORKSPACE_MODES.includes(command.mode) ? null : `unknown mode ${command.mode}`;
    case 'LaunchApp':
    case 'ActivateWorkspace':
        return command.id ? null : 'empty id';
    default:
        return `unknown method ${command.method}`;
    }
}

export function blockedWhenLocked(command) {
    return command.method === 'Shortcut' || command.method === 'LaunchApp' ||
        (command.method === 'Key' && command.pressed);
}

/**
 * The checks before a command, in contract order: arguments, caller,
 * capability, lock. Returns null or a Rejection.
 */
export function admit(command, verdict, capabilities, locked) {
    const problem = commandProblem(command);
    if (problem)
        return new Rejection('invalid-args', problem);
    if (verdict !== 'trusted')
        return new Rejection('denied', verdict === 'sandboxed'
            ? 'the caller runs in a sandbox' : 'the caller could not be checked');
    if (!capabilities.includes(command.method))
        return new Rejection('unsupported', `${command.method} is not supported by this provider`);
    if (locked && blockedWhenLocked(command))
        return new Rejection('locked', 'the screen is locked');
    return null;
}

/** The app.id rule (W6). */
export function normalizeAppId(desktopId, wmClass) {
    let id = typeof desktopId === 'string' ? desktopId.trim() : '';
    if (id.endsWith('.desktop'))
        id = id.slice(0, -'.desktop'.length);
    if (id && !id.startsWith('window:'))
        return id;
    return `wmclass:${typeof wmClass === 'string' ? wmClass.trim() : ''}`;
}

/**
 * The sandbox rule (W20). `evidence` is {pid: bool, flatpakInfo:
 * 'present'|'absent'|'failed', cgroup: {probe, text}}. Returns 'trusted',
 * 'sandboxed' or 'unverified'.
 */
export function classifyCaller(evidence) {
    if (!evidence.pid)
        return 'unverified';
    if (evidence.flatpakInfo === 'present')
        return 'sandboxed';
    if (evidence.flatpakInfo !== 'absent')
        return 'unverified';
    if (evidence.cgroup.probe !== 'present')
        return 'unverified';
    for (const line of evidence.cgroup.text.split('\n')) {
        const fields = line.split(':');
        if (fields.length < 3)
            continue;
        const path = fields.slice(2).join(':');
        for (const unit of path.split('/')) {
            if (unit.startsWith('app-flatpak-') || unit.startsWith('snap.'))
                return 'sandboxed';
        }
    }
    return 'trusted';
}

/**
 * Keyval of an XKB keysym name, from a table of `KEY_*` constants such as
 * Clutter. `XF86Foo` names map to `KEY_Foo`, as Clutter drops the prefix.
 */
export function keyvalForName(name, table) {
    const direct = table[`KEY_${name}`];
    if (typeof direct === 'number')
        return direct;
    if (name.startsWith('XF86')) {
        const stripped = table[`KEY_${name.slice(4)}`];
        if (typeof stripped === 'number')
            return stripped;
    }
    return null;
}

/** GNOME accent-color names and their libadwaita colors. */
export const ACCENT_COLORS = Object.freeze({
    blue: '#3584e4',
    teal: '#2190a4',
    green: '#3a944a',
    yellow: '#c88800',
    orange: '#ed5b00',
    red: '#e62d42',
    pink: '#d56199',
    purple: '#9141ac',
    slate: '#6f8396',
});

/** `org.gnome.desktop.interface color-scheme` to the contract scheme. */
export function schemeFromColorScheme(value) {
    return value === 'prefer-dark' ? 'dark' : 'light';
}
