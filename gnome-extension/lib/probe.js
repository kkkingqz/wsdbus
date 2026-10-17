// Collects sandbox evidence about a D-Bus caller (W20); classifyCaller in
// contract.js judges it.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

import {classifyCaller} from './contract.js';

function fileState(path) {
    try {
        Gio.File.new_for_path(path).query_info('standard::type',
            Gio.FileQueryInfoFlags.NOFOLLOW_SYMLINKS, null);
        return 'present';
    } catch (error) {
        return error.matches(Gio.IOErrorEnum, Gio.IOErrorEnum.NOT_FOUND) ? 'absent' : 'failed';
    }
}

function readText(path) {
    try {
        const [, bytes] = GLib.file_get_contents(path);
        return {probe: 'present', text: new TextDecoder().decode(bytes)};
    } catch (error) {
        return {probe: error.matches(GLib.FileError, GLib.FileError.NOENT) ? 'absent' : 'failed'};
    }
}

/** Evidence for a PID read from /proc. */
export function evidenceForPid(pid) {
    return {
        pid: true,
        flatpakInfo: fileState(`/proc/${pid}/root/.flatpak-info`),
        cgroup: readText(`/proc/${pid}/cgroup`),
    };
}

function callerPid(connection, sender) {
    return new Promise(resolve => {
        connection.call('org.freedesktop.DBus', '/org/freedesktop/DBus',
            'org.freedesktop.DBus', 'GetConnectionUnixProcessID',
            new GLib.Variant('(s)', [sender]), new GLib.VariantType('(u)'),
            Gio.DBusCallFlags.NONE, -1, null, (source, result) => {
                try {
                    resolve(source.call_finish(result).deepUnpack()[0]);
                } catch (_) {
                    resolve(null);
                }
            });
    });
}

/** 'trusted', 'sandboxed' or 'unverified' for a bus sender. */
export async function callerVerdict(connection, sender) {
    const pid = sender ? await callerPid(connection, sender) : null;
    if (pid === null)
        return classifyCaller({pid: false, flatpakInfo: 'failed', cgroup: {probe: 'failed'}});
    return classifyCaller(evidenceForPid(pid));
}
