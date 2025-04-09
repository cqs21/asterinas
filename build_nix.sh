#!/bin/bash

set -e

BUILD_PLATFORM="x86_64-linux"
HOST_PLATFORM="riscv64-linux"

SYS_ROOT=$(pwd)/sysroot
OUT_LINK=$(pwd)/result
NIXOS_CONFIG=$(pwd)/config.nix
ASTER_NIX=$(pwd)/asterinas.nix

cat > $NIXOS_CONFIG <<EOF
{ config, lib, pkgs, ... }:
{
  nixpkgs.buildPlatform.system = "$BUILD_PLATFORM";
  nixpkgs.hostPlatform.system = "$HOST_PLATFORM";

  nixpkgs.config.allowUnfree = true;

  imports = [ $ASTER_NIX ];

  boot.loader.grub.enable = false;
  boot.loader.systemd-boot.enable = false;
  boot.initrd.enable = false;
  boot.kernel.enable = false;
  environment.defaultPackages = with pkgs; [
    busybox
    python39
    go
    # zulu -> java
    # sysbench
    # membench
    iperf
    # lmbench
    unixbench
    # iozone
    fio
    # hackbench
    # schbench
    libmemcached
  ];

  systemd.enableCgroupAccounting = false;

  system.stateVersion = "24.11";
}
EOF

nix-build --store $SYS_ROOT --out-link $OUT_LINK \
	-I nixos-config=$NIXOS_CONFIG \
	'<nixpkgs/nixos>' -A system

nix-env --store $SYS_ROOT \
	-p $SYS_ROOT/nix/var/nix/profiles/system \
	--set $(readlink -f $OUT_LINK)

install -m 0755 -d "$SYS_ROOT/bin"
install -m 0755 -d "$SYS_ROOT/sbin"
install -m 0755 -d "$SYS_ROOT/usr/bin"
install -m 0755 -d "$SYS_ROOT/usr/sbin"

install -m 0755 -d "$SYS_ROOT/dev"
install -m 0755 -d "$SYS_ROOT/proc"
install -m 0755 -d "$SYS_ROOT/sys"
install -m 0755 -d "$SYS_ROOT/run"
install -m 0755 -d "$SYS_ROOT/var"
install -m 1777 -d "$SYS_ROOT/tmp"

install -m 0755 -d "$SYS_ROOT/etc"
install -m 0755 -d "$SYS_ROOT/lib"
install -m 0755 -d "$SYS_ROOT/lib64"
install -m 0755 -d "$SYS_ROOT/home"
install -m 0700 -d "$SYS_ROOT/root"

SW=$(readlink -f $SYS_ROOT/$(readlink -f $OUT_LINK)/sw)
SH=$(readlink -f $SYS_ROOT/$SW/bin/sh)
ln -sfn $SH $SYS_ROOT/bin/sh
rm -f $OUT_LINK
