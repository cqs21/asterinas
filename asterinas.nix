{ config, lib, pkgs, ... }:
let
  benchmark = lib.fileset.toSource {
    root = ./.;
    fileset = ./test/benchmark;
  };
  hackbench = pkgs.callPackage ./hackbench.nix { };
  iozone = pkgs.callPackage ./iozone.nix { };
  lmbench = pkgs.callPackage ./lmbench.nix { };
  ltp = if pkgs.hostPlatform.system == "x86_64-linux" then
    pkgs.callPackage ./ltp.nix { }
  else
    null;
  membench = pkgs.callPackage ./membench.nix { };
  schbench = pkgs.callPackage ./schbench.nix { };
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
          mkdir -p $out/bin
          for pkg in ${
            lib.concatStringsSep " " (map (pkg: pkg.outPath) packages)
          }; do
            cp -rp $pkg/* $out/bin/
          done

          mkdir $out/bin/fio
          cp -rp ${pkgs.fio}/* $out/bin/fio/

          mkdir $out/bin/iperf
          cp -rp ${pkgs.iperf}/* $out/bin/iperf/

          mkdir $out/bin/libmemcached
          cp -rp ${pkgs.libmemcached}/* $out/bin/libmemcached/

          mkdir $out/bin/unixbench
          cp -rp ${pkgs.unixbench}/* $out/bin/unixbench/

          if [ ${pkgs.hostPlatform.system} == "x86_64-linux" ]; then
            mkdir $out/bin/sysbench
            cp -rp ${pkgs.sysbench}/* $out/bin/sysbench/

            mkdir $out/ltp
            cp -rp ${ltp}/* $out/ltp/
          fi

          cp -rp ${benchmark}/test/benchmark/* $out/
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
