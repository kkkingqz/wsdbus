import GLib from 'gi://GLib';

export function assert(condition, message = 'assertion failed') {
    if (!condition)
        throw new Error(message);
}

function canonical(value) {
    if (Array.isArray(value))
        return value.map(canonical);
    if (value && typeof value === 'object') {
        return Object.fromEntries(Object.keys(value).sort()
            .map(key => [key, canonical(value[key])]));
    }
    return value;
}

/** Deep equality; object key order does not matter, array order does. */
export function equal(actual, expected, message = '') {
    const a = JSON.stringify(canonical(actual));
    const b = JSON.stringify(canonical(expected));
    if (a !== b)
        throw new Error(`${message} expected ${b}, got ${a}`);
}

/** A file of the repository, relative to its root. */
export function repoFile(path) {
    const tests = GLib.path_get_dirname(GLib.filename_from_uri(import.meta.url)[0]);
    const root = GLib.path_get_dirname(GLib.path_get_dirname(tests));
    const [, bytes] = GLib.file_get_contents(GLib.build_filenamev([root, path]));
    return new TextDecoder().decode(bytes);
}
