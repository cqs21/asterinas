{ pkgs ? import <nixpkgs> { } }:
let
  logo = builtins.path {
    name = "logo";
    path = ./logo.png;
  };
in pkgs.stdenv.mkDerivation {
  name = "splash";
  src = pkgs.fetchzip {
    url = "https://github.com/NixOS/nixos-artwork/releases/download/bootloader-18.09-pre/grub2-installer.tar.bz2";
    sha256 = "0rhh061m1hpgadm7587inw3fxfacnd53xjc53w3vzghlck56djq5";
  };
  buildCommand = ''
    mkdir -p $out
    find $src -mindepth 1 -maxdepth 1 ! -name "logo.png" -exec cp -r {} $out \;
    cp ${logo} $out/logo.png
  '';
}
