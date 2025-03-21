{ pkgs ? import <nixpkgs> { }, }:
let
  fs = pkgs.lib.fileset;
  stdenv = pkgs.stdenv;
  sourceFile = ./test/benchmark;
  sysbench = pkgs.stdenv.mkDerivation {
    pname = "sysbench";
    version = "1.0.20";
    src = pkgs.fetchzip {
      url = "https://github.com/akopytov/sysbench/archive/1.0.20.tar.gz";
      hash = "sha256-HhcnsdEHIz/JuZFvTC/3r73/SxrcCiOhJs6JogTdVuk";
    };
    nativeBuildInputs =
      [ pkgs.glibc pkgs.m4 pkgs.autoconf pkgs.automake pkgs.libtool ];
    buildCommand = ''
      # FIXME: automake 1.10.x (aclocal) wasn't found, exiting
      sh $src/autogen.sh
      sh $src/configure --without-mysql --prefix=/usr/local/benchmark/sysbench
      make -j
      make install
      # TODO: mv binaries to $out
    '';
  };
in stdenv.mkDerivation {
  name = "benchmark";
  src = fs.toSource {
    root = ./.;
    fileset = sourceFile;
  };
  buildInputs = [ pkgs.glibc.static ];
  buildCommand = ''
    cp -r $src/test/benchmark/* $out

    mkdir -p $out/bin
    cp ${sysbench} $out/bin
  '';
}
