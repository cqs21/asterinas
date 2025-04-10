{ config, lib, pkgs, ... }:
let
  hackbench = pkgs.callPackage ./hackbench.nix { };
  iozone = pkgs.callPackage ./iozone.nix { };
  lmbench = pkgs.callPackage ./lmbench.nix { };
  membench = pkgs.callPackage ./membench.nix { };
  # sysbench = pkgs.callPackage ./sysbench.nix { };
  schbench = pkgs.callPackage ./schbench.nix { };
  syscall_test = pkgs.callPackage ./syscall_test.nix { };
  test = pkgs.callPackage ./test.nix { };
  packages = [ hackbench iozone lmbench membench schbench test ];
in {
  options = {
    asterinas.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Whether to enable asterinas package.";
    };
    asterinas.package = lib.mkOption {
      type = lib.types.package;
      description = "The asterinas package to use.";
      default = pkgs.stdenv.mkDerivation {
        name = "asterinas";
        version = "0.14.1";
        buildCommand = ''
          mkdir -p $out
          for pkg in ${
            lib.concatStringsSep " " (map (pkg: pkg.outPath) packages)
          }; do
            cp -rp $pkg/* $out/
          done
        '';
      };
    };
  };

  config = lib.mkIf config.asterinas.enable {
    system.systemBuilderCommands = ''
      ln -s ${config.asterinas.package} $out/asterinas
    '';
  };
}
