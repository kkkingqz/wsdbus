# wsdbus GNOME Shell extension

The GNOME provider of the `Desktop1` contract (`spec/`). UUID
`wsdbus@kkkingqz.github.io`, GNOME Shell 50, session modes `user` and
`unlock-dialog` (the facts stay available while the screen is locked).

| File | What |
|---|---|
| `extension.js` | enable/disable |
| `lib/shell.js` | GNOME Shell sources, D-Bus export, commands |
| `lib/contract.js` | names, fact rules, command checks, `app.id` and sandbox rules |
| `lib/store.js` | state with one revision |
| `lib/wire.js` | GLib.Variant encoding |
| `lib/keys.js` | keys held through `Key`, released when the caller leaves or the screen locks |
| `lib/probe.js` | sandbox evidence of a caller from `/proc` |
| `io.github.kkkingqz.WsDbus.Desktop1.xml` | copy of the spec XML; a test keeps them equal |

## Sources

| Contract | GNOME |
|---|---|
| `focus.*` | `global.display.focus_window`, `Shell.WindowTracker` |
| `session.locked` | `Main.sessionMode.isLocked` |
| `appearance.*` | `org.gnome.desktop.interface` `color-scheme`, `accent-color` (libadwaita colors), `font-name` |
| `workspaces`, `workspace.active` | `global.workspace_manager`; ids are numbers from 1, names from `org.gnome.desktop.wm.preferences workspace-names`, monitor is the primary one |
| `display.brightness`, `SetBrightness` | `Main.brightnessManager.globalScale`; absent and `Unsupported` without a backlight |
| `keyboard.layout` | input source manager, id of the current source |
| windows | normal and dialog windows outside `skip-taskbar`; id from `get_stable_sequence()`; `floating` is always true |
| `Shortcut`, `Key` | Clutter virtual keyboard; key names resolve through `Clutter.KEY_*`, `XF86Foo` as `KEY_Foo` |
| `LaunchApp` | `Shell.AppSystem` lookup of `ID.desktop`, then `activate()` |
| `SetWorkspaceMode` | not offered (W18) |

Changes from Shell signals are collected and published once per idle cycle.

## Tests

```sh
gjs -m gnome-extension/tests/run-tests.js
gnome-extension/tests/headless.sh target/debug/wsdbus-ctl
```

The first runs the modules without GNOME Shell, including the shared cases
of `spec/`. The second starts a headless GNOME Shell 50 with its own session
bus and temporary XDG directories, enables the extension there and drives it
with `wsdbus-ctl`: facts, a real window, commands and errors, the lock
screen. It does not touch the running session. The live checklist is
`docs/testing.md`.
