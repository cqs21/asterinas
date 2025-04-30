{ target, }:
let
  crossSystem.config = if target == "x86_64" then
    "x86_64-unknown-linux-gnu"
  else if target == "riscv64" then
    "riscv64-unknown-linux-gnu"
  else
    throw "Target arch ${target} not yet supported.";
  pkgs = import <nixpkgs> { inherit crossSystem; };

  benchmark = pkgs.callPackage ./benchmark.nix { inherit pkgs; };
  busybox = pkgs.busybox.override { enableStatic = true; };
  test = pkgs.callPackage ./test.nix { inherit pkgs; };
  syscall_test = pkgs.callPackage ./syscall_test.nix { inherit pkgs; };
  vdso = pkgs.fetchFromGitHub {
    owner = "asterinas";
    repo = "linux_vdso";
    rev = "be255018febf8b9e2d36f356f6aeb15896521618";
    hash = "sha256-F5RPtu/Hh2hDnjm6/0mc0wGqhQtfMNvPP+6/Id9Hcpk";
  };
in pkgs.stdenv.mkDerivation {
  name = "initramfs";
  nativeBuildInputs = [ pkgs.buildPackages.cpio ];
  buildCommand = ''
    pushd $(mktemp -d)
    mkdir -m 0755 ./{dev,etc,usr,ext2,exfat}
    mkdir -m 0555 ./{proc,sys}
    mkdir -m 1777 ./tmp

    mkdir -p ./usr/{bin,sbin,lib,lib64,local}
    ln -sfn usr/bin bin
    ln -sfn usr/sbin sbin
    ln -sfn usr/lib lib
    ln -sfn usr/lib64 lib64
    cp -r ${busybox}/bin/* ./bin/

    mkdir -m 0755 ./test
    cp -r ${test}/* ./

    mkdir -m 0755 -p ./opt/ltp
    cp -r ${syscall_test}/* ./

    mkdir -m 0755 -p ./benchmark
    cp -r ${benchmark}/* ./

    mkdir -p ./usr/lib/x86_64-linux-gnu
    if [ ${target} == "x86_64" ]; then
      cp -r ${vdso}/vdso64.so ./usr/lib/x86_64-linux-gnu/vdso64.so
    elif [ ${target} == "riscv64" ]; then
      cp -r ${vdso}/riscv64-vdso.so ./usr/lib/x86_64-linux-gnu/vdso64.so
    fi

    mkdir -p ./${pkgs.glibc}/
    cp -r ${pkgs.glibc}/lib ./${pkgs.glibc}/

    mkdir -p ./${pkgs.gcc-unwrapped.lib}/lib
    cp -L ${pkgs.gcc-unwrapped.lib}/lib/libgcc_s.so.1 ./${pkgs.gcc-unwrapped.lib}/lib/
    cp -L ${pkgs.gcc-unwrapped.lib}/lib/libstdc++.so.6 ./${pkgs.gcc-unwrapped.lib}/lib/

    mkdir -p ./${pkgs.zlib}/
    cp -r ${pkgs.zlib}/lib ./${pkgs.zlib}/

    mkdir -p ./${pkgs.libaio}/
    cp -r ${pkgs.libaio}/lib ./${pkgs.libaio}/

    mkdir -p ./${pkgs.iperf}/
    cp -r ${pkgs.iperf}/lib ./${pkgs.iperf}/

    mkdir -p ./${pkgs.openssl.out}/
    cp -r ${pkgs.openssl.out}/lib ./${pkgs.openssl.out}/

    mkdir -p ./${pkgs.lksctp-tools}/
    cp -r ${pkgs.lksctp-tools}/lib ./${pkgs.lksctp-tools}/

    mkdir -p ./${pkgs.libtirpc}/
    cp -r ${pkgs.libtirpc}/lib ./${pkgs.libtirpc}/

    mkdir -p ./${pkgs.krb5.lib}/
    cp -r ${pkgs.krb5.lib}/lib ./${pkgs.krb5.lib}/

    mkdir -p ./${pkgs.keyutils.lib}/
    cp -r ${pkgs.keyutils.lib}/lib ./${pkgs.keyutils.lib}/

    mkdir -p ./${pkgs.libmemcached}/
    cp -r ${pkgs.libmemcached}/lib ./${pkgs.libmemcached}/

    mkdir -p ./${pkgs.cyrus_sasl.out}/
    cp -r ${pkgs.cyrus_sasl.out}/lib ./${pkgs.cyrus_sasl.out}/

    find . -print0 | cpio -o -H newc --null | cat > $out
    popd
  '';
}
