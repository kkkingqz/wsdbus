# Home Manager module: wsdbus-ctl and the GNOME provider. Enabling the
# extension (org.gnome.shell enabled-extensions) is left to the owner of
# the extension list: extensionUuid names it.
{ config, lib, pkgs, ... }:
let
  cfg = config.programs.wsdbus;
  packages = import ./packages.nix { inherit pkgs; };
in
{
  options.programs.wsdbus = {
    enable = lib.mkEnableOption "wsdbus-ctl and the wsdbus GNOME Shell provider";
    gnome = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Install the GNOME Shell provider extension.";
    };
    ctl = lib.mkOption {
      type = lib.types.str;
      readOnly = true;
      default = lib.getExe packages.ctl;
      description = "Absolute path of wsdbus-ctl, for services and scripts without the profile in PATH.";
    };
    extensionUuid = lib.mkOption {
      type = lib.types.str;
      readOnly = true;
      default = packages.uuid;
      description = "UUID of the GNOME provider extension to enable.";
    };
  };

  config = lib.mkIf cfg.enable {
    home.packages = [ packages.ctl ];

    # force: replaces a copy installed by hand.
    home.file.".local/share/gnome-shell/extensions/${packages.uuid}" = lib.mkIf cfg.gnome {
      source = "${packages.gnome-extension}/share/gnome-shell/extensions/${packages.uuid}";
      recursive = true;
      force = true;
    };
  };
}
