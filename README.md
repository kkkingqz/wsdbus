# wsdbus

A shared desktop API for the workstation projects: one D-Bus contract of
desktop facts and commands, a provider for each desktop, and a window-policy
daemon for Hyprland.

- consumers: [wstouchbar](https://github.com/kkkingqz/wstouchbar),
  [wswidgets](https://github.com/kkkingqz/wswidgets), the minimized-windows
  and window-switcher UI;
- providers: a GNOME Shell extension and a Rust daemon for Hyprland
  (`provider` always on, `policy` optional: floating workspaces,
  minimize/restore, maximize, snapping).

Status: planning. The plan (in Russian):
[docs/plans/2026-10-17-wsdbus.md](docs/plans/2026-10-17-wsdbus.md).

## License

[MIT](LICENSE).
