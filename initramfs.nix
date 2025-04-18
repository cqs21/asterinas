{ crossSystem, }:
let
  pkgs = import <nixpkgs> { inherit crossSystem; };
  busybox = pkgs.busybox.override { enableStatic = true; };
  test = pkgs.callPackage ./test.nix { inherit pkgs; };
  vdso64 = pkgs.fetchurl {
    url =
      "https://raw.githubusercontent.com/asterinas/linux_vdso/2a6d2db/vdso64.so";
    hash = "sha256-J8179XapL6SKiRwFmI9S+sNbc3TVuWUNawNeR3xdk6M";
  };
in pkgs.stdenv.mkDerivation {
  name = "initramfs";
  nativeBuildInputs = [ pkgs.buildPackages.cpio ];
  buildCommand = ''
    pushd $(mktemp -d)
    mkdir -m 0755 ./{dev,etc,usr,ext2,exfat,test}
    mkdir -m 0555 ./{proc,sys}
    mkdir -m 1777 ./tmp

    mkdir -p ./usr/{bin,sbin,lib,lib64,local}
    ln -sfn usr/bin bin
    ln -sfn usr/sbin sbin
    ln -sfn usr/lib lib
    ln -sfn usr/lib64 lib64
    cp -rp ${busybox}/bin/* ./bin/
    cp -rp ${test}/* ./

    if [ ${crossSystem.config} == "x86_64-unknown-linux-gnu" ]; then
      mkdir -p ./usr/lib/x86_64-linux-gnu
      cp -rp ${vdso64} ./usr/lib/x86_64-linux-gnu/vdso64.so
    fi

    find . -print0 | cpio -o -H newc --null | cat > $out
    popd
  '';
}
