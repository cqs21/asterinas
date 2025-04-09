{ config, lib, pkgs, ... }:
pkgs.stdenv.mkDerivation {
  pname = "sysbench";
  version = "1.0.20";
  src = pkgs.fetchzip {
    url = "https://github.com/akopytov/sysbench/archive/1.0.20.tar.gz";
    hash = "sha256-HhcnsdEHIz/JuZFvTC/3r73/SxrcCiOhJs6JogTdVuk";
  };
  nativeBuildInputs = with pkgs.buildPackages; [
    automake
    autoconf
    libtool
    gnum4
    makeWrapper
    pkg-config
  ];
  configurePhase = ''
    libtoolize --copy --force
    aclocal -I m4
    autoheader
    automake -c --foreign --add-missing
    autoconf

    # FIXME: this is a workaround to add pkg-config.
    ln -s ${pkgs.buildPackages.pkg-config}/bin/*pkg-config ./pkg-config
    export PATH=./:$PATH

    ./configure --without-mysql \
      --host ${pkgs.hostPlatform.system} \
      --prefix=/usr/local/benchmark/sysbench
  '';
  makeFlags = [ "CC=${pkgs.stdenv.cc.targetPrefix}cc" ];
  installPhase = ''
    #error "No support for this architecture (yet)"
    exit 1
  '';
}
