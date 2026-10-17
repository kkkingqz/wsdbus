# What the flake and the Home Manager module install, built with the
# consumer's nixpkgs.
{ pkgs }:
let
  inherit (pkgs) lib;
  version = (lib.importTOML ../Cargo.toml).workspace.package.version;
  uuid = "wsdbus@kkkingqz.github.io";
  source = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../Cargo.toml ../Cargo.lock ../crates ../bin ../spec ../gnome-extension
    ];
  };
in
{
  inherit source uuid;

  # wsdbus-ctl; the check runs the tests of the whole workspace.
  ctl = pkgs.rustPlatform.buildRustPackage {
    pname = "wsdbus-ctl";
    inherit version;
    src = lib.fileset.toSource {
      root = ../.;
      fileset = lib.fileset.unions [ ../Cargo.toml ../Cargo.lock ../crates ../bin ../spec ];
    };
    cargoLock.lockFile = ../Cargo.lock;
    cargoBuildFlags = [ "--package" "wsdbus-ctl" ];
    cargoTestFlags = [ "--workspace" ];
    nativeCheckInputs = [ pkgs.dbus ];
    meta = {
      description = "Command-line client of the wsdbus Desktop1 contract";
      license = lib.licenses.mit;
      mainProgram = "wsdbus-ctl";
    };
  };

  # The GNOME Shell provider, without its tests.
  gnome-extension = pkgs.runCommand "wsdbus-gnome-extension-${version}" { } ''
    dir=$out/share/gnome-shell/extensions/${uuid}
    mkdir -p "$dir"
    cp -r ${../gnome-extension}/. "$dir"
    chmod -R u+w "$dir"
    rm -rf "$dir/tests"
  '';
}
