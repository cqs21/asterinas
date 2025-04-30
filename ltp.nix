{ pkgs ? import <nixpkgs> { }, }:
with pkgs;
stdenv.mkDerivation rec {
  pname = "ltp";
  version = "20250130";

  src = fetchFromGitHub {
    owner = "linux-test-project";
    repo = "ltp";
    rev = "${version}";
    hash = "sha256-FgTRDAvYvVuhxxhwM6UO4qzq0FUzJqxV08sl1xOuHVU";
  };

  dontPatchShebangs = true;
  enableParallelBuilding = true;
  nativeBuildInputs = with buildPackages; [
    automake
    autoconf
    libtool
    gnum4
    makeWrapper
    pkg-config
  ];
  patchPhase = ''
    # FIXME: Unlinking a file would cause the mapped memory to become inaccessible
    # due to the `PageCache` being dropped, which is a bug of Asterinas.
    substituteInPlace lib/tst_test.c --replace "SAFE_UNLINK(shm_path);" ""

    # FIXME: `write_kmsg` function requires `/dev/kmsg`, which is not available now.
    sed -i "1372,1384d" pan/ltp-pan.c
  '';
  configurePhase = ''
    make autotools
    ./configure --host ${hostPlatform.system} --prefix=$out
  '';
  buildPhase = ''
    make -C testcases/kernel/syscalls
    make -C testcases/lib
    make -C runtest
    make -C pan
  '';
  installPhase = ''
    make -C testcases/kernel/syscalls install
    make -C testcases/lib install
    make -C runtest install
    make -C pan install

    install -m 00755 $src/runltp $out/runltp
    install -m 00444 $src/VERSION $out/Version
    install -m 00755 $src/ver_linux $out/ver_linux
    install -m 00755 $src/IDcheck.sh $out/IDcheck.sh
  '';
}
