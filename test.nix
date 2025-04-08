{ pkgs ? import <nixpkgs> { }, }:
let
  fs = pkgs.lib.fileset;
  stdenv = pkgs.stdenv;
  platform = pkgs.hostPlatform.system;
in stdenv.mkDerivation {
  name = "test";
  src = fs.toSource {
    root = ./.;
    fileset = ./test/apps;
  };
  buildInputs = [ pkgs.glibc.static ];
  buildCommand = ''
    if [ ${platform} == "riscv64-linux" ]; then
      ARCH_PRE="riscv64-unknown-linux-gnu-"
    fi

    BUILD_DIR=$(mktemp -d)
    mkdir -p $BUILD_DIR/build
    cp -r $src/test/apps $BUILD_DIR/

    pushd $BUILD_DIR
    make HOST_PLATFORM=${platform} CC="''${ARCH_PRE}gcc" --no-print-directory -C apps
    popd

    mkdir -p $out/test
    mv $BUILD_DIR/build/initramfs/test $out/
  '';
}
