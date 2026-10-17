{
  description = "wsdbus: shared desktop D-Bus contract, its client and the GNOME provider (MIT)";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";

  outputs = { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
      packages = import ./nix/packages.nix { inherit pkgs; };
    in {
      # wsdbus-ctl and the GNOME provider for Home Manager (nix/module.nix).
      homeManagerModules.default = import ./nix/module.nix;

      packages.${system} = {
        default = packages.ctl;
        inherit (packages) ctl gnome-extension;
      };

      checks.${system} = {
        # The Rust workspace with its tests (a private dbus-daemon each).
        inherit (packages) ctl gnome-extension;
        gjs = pkgs.runCommand "wsdbus-gjs-tests" { nativeBuildInputs = [ pkgs.gjs ]; } ''
          cp -r ${packages.source} source
          cd source
          gjs -m gnome-extension/tests/run-tests.js
          touch "$out"
        '';
      };
    };
}
