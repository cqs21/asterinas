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

  # dontPatchShebangs = true;
  enableParallelBuilding = true;
  nativeBuildInputs = with buildPackages; [
    automake
    autoconf
    libtool
    gnum4
    makeWrapper
    pkg-config
  ];
  configurePhase = ''
    make autotools
    ./configure --host ${hostPlatform.system} --prefix=$out
  '';
}
