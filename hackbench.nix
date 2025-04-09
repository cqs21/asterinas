{ stdenv, fetchgit, }:
stdenv.mkDerivation rec {
  pname = "hackbench";
  version = "0.0.1";
  src = fetchgit {
    url =
      "https://git.kernel.org/pub/scm/linux/kernel/git/clrkwllms/rt-tests.git";
    tag = "v0.92";
    hash = "sha256-gvg+2jyKc5zw9BK25BVMr7T8iTUgg0dviLSlPyn8IqM";
  };
  buildPhase = ''
    cd src/hackbench
    make hackbench
  '';
  installPhase = ''
    mkdir -p $out
    cp hackbench $out/
  '';
}
