{ config, lib, pkgs, ... }:

{
  environment.systemPackages = with pkgs; [
    fish
    zsh
    fastfetch
    htop
    lsof
    ncdu
    procps
    coreutils
    diffutils
    findutils
    gnugrep
    hostname
    less
    man-pages
    texinfoInteractive
    util-linux
    which
  ];
}
