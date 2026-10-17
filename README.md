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

Status: planning. Plans and decisions (in Russian):

- [plan 1: contract and GNOME](docs/plans/2026-10-17-core.md), no Hyprland;
- [plan 2: the Hyprland stack](docs/plans/2026-10-17-hyprland.md);
- [decision log](docs/decisions.md).

## License

[MIT](LICENSE).
