{ pkgs ? import <nixpkgs> { }, }:
let
  fs = pkgs.lib.fileset;
  stdenv = pkgs.stdenv;
  sourceFile = ./test/apps;
in stdenv.mkDerivation {
  name = "test";
  src = fs.toSource {
    root = ./.;
    fileset = sourceFile;
  };
  buildInputs = [ pkgs.glibc.static ];
  buildCommand = ''
    BUILD_DIR=$(mktemp -d)
    mkdir -p $BUILD_DIR/build
    cp -r $src/test/apps $BUILD_DIR/

    pushd $BUILD_DIR
    make --no-print-directory -C apps
    popd

    mkdir -p $out/test
    mv $BUILD_DIR/build/initramfs/test $out/
  '';
}
