{ pkgs ? import <nixpkgs> { }, }:
let
  busybox = pkgs.callPackage ./busybox.nix { };
  test = pkgs.callPackage ./test.nix { };
  e2fsprogs = pkgs.e2fsprogs.bin;
  fakeroot = pkgs.fakeroot;
in pkgs.stdenv.mkDerivation {
  name = "rootfs-image";
  nativeBuildInputs = [ e2fsprogs fakeroot ];
  buildCommand = ''
    set -e

    tmp_dir=$(mktemp -d)
    rootfs=$tmp_dir/rootfs
    img_file=$tmp_dir/rootfs.img

    mkdir -p $rootfs/{dev,etc,proc,sys,tmp,var}
    chmod 1777 $rootfs/tmp

    cp -r ${busybox}/* $rootfs/
    cp -r ${test}/* $rootfs/

    dd if=/dev/zero of=$img_file bs=1M count=1024

    fakeroot mke2fs -t ext2 -d $rootfs $img_file

    resize2fs -M $img_file

    mv $img_file $out
  '';
}
