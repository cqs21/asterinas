{ stdenv, fetchgit, }:
stdenv.mkDerivation {
  pname = "schbench";
  version = "1.0.0";
  src = fetchgit {
    url = "https://git.kernel.org/pub/scm/linux/kernel/git/mason/schbench.git";
    tag = "v1.0";
    hash = "sha256-BSGp2TpNh29OsqwDEwaRC1W8T6QFec7AhgVgNEslHZY";
  };
  patchPhase = ''
    substituteInPlace schbench.c \
      --replace "defined(__powerpc64__)" "defined(__powerpc64__) || defined(__riscv)"
  '';
  makeFlags = [ "CC=${stdenv.cc.targetPrefix}cc" ];
  installPhase = ''
    mkdir -p $out
    cp schbench $out/
  '';
}
