# wsdbus

A shared desktop API for the workstation projects: one D-Bus contract of
desktop facts and commands, a provider for each desktop, and a window-policy
daemon for Hyprland.

- consumers: [wstouchbar](https://github.com/kkkingqz/wstouchbar),
  [wswidgets](https://github.com/kkkingqz/wswidgets), `ws-window` of
  [wsconfig](https://github.com/kkkingqz/wsconfig), the minimized-windows
  and window-switcher UI;
- providers: a GNOME Shell extension and a Rust daemon for Hyprland
  (`provider` always on, `policy` optional: floating workspaces,
  minimize/restore, maximize, snapping).

Status: plan 1 stages 1 (contract) and 2 (GNOME provider) are
implemented; integration with the consumers is next. Plans, decisions and the contract (in Russian):

- [contract `Desktop1`](spec/README.md) and its
  [D-Bus interface](spec/io.github.kkkingqz.WsDbus.Desktop1.xml);

- [plan 1: contract and GNOME](docs/plans/2026-10-17-core.md), no Hyprland;
- [plan 2: the Hyprland stack](docs/plans/2026-10-17-hyprland.md);
- [decision log](docs/decisions.md).

## Layout

| Path | What |
|---|---|
| `spec/` | contract description, D-Bus XML, shared `app.id` and sandbox cases |
| `crates/wsdbus-contract` | types, rules, D-Bus and JSON encoding, provider store, consumer mirror; no IO |
| `crates/wsdbus-client` | zbus client: commands, `Watcher` with snapshot, gaps and reconnects; provider helpers |
| `crates/wsdbus-fake` | fake provider and a private `dbus-daemon` for consumer tests |
| `bin/wsdbus-ctl` | command-line client; `watch --json` for QML consumers |
| `gnome-extension/` | GNOME Shell 50 provider, see its [README](gnome-extension/README.md) |

## Checks

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

The tests start their own `dbus-daemon` and never use the session bus;
`dbus-daemon` must be installed. The extension has GJS tests and a headless
GNOME Shell test, see [docs/testing.md](docs/testing.md) (in Russian).

## License

[MIT](LICENSE).
