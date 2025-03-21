{ pkgs ? import <nixpkgs> { }, }:
pkgs.stdenv.mkDerivation {
  pname = "busybox";
  version = "1.35.0";
  src = pkgs.fetchzip {
    url = "https://busybox.net/downloads/busybox-1.35.0.tar.bz2";
    hash = "sha256-TtncnDoY7Eumz6bVpB/gL19ZqoVK+ynwibNODj9t9SM";
  };
  buildInputs = [ pkgs.glibc ];
  configurePhase = ''
    make defconfig
    sed -i "s/# CONFIG_STATIC is not set/CONFIG_STATIC=y/g" .config
    sed -i "s/# CONFIG_FEATURE_SH_STANDALONE is not set/CONFIG_FEATURE_SH_STANDALONE=y/g" .config
    sed -i "s/CONFIG_TC=y/# CONFIG_TC is not set/g" .config
  '';
  buildPhase = ''
    export LDFLAGS="-L${pkgs.glibc.static}/lib"
    make -j
  '';
  installPhase = ''
    make install CONFIG_PREFIX=$out
  '';
}
