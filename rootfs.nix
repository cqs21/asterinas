{ pkgs ? import <nixpkgs> { }, }:
let
  nixos = import <nixpkgs/nixos> { configuration = ./os-config.nix; };
  # e2fsprogs = pkgs.e2fsprogs.bin;
  # fakeroot = pkgs.fakeroot;
in pkgs.stdenv.mkDerivation {
  name = "rootfs";
  # nativeBuildInputs = [ e2fsprogs fakeroot ];
  buildCommand = ''
    mkdir -p $out/benchmark
    cp -r ${nixos.system}/asterinas/* $out/benchmark/

    cp -r ${nixos.system}/sw/* $out/

    # FIXME: add /nix/store/ to rootfs

    # TODO: create ext2 image from rootfs
    # tmp_dir=$(mktemp -d)
    # rootfs=$tmp_dir/rootfs
    # img_file=$tmp_dir/rootfs.img

    # dd if=/dev/zero of=$img_file bs=4M count=1024

    # fakeroot mke2fs -t ext2 -d $rootfs $img_file

    # resize2fs -M $img_file
  '';
}
