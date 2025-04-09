{ config, lib, pkgs, ... }:
with pkgs;
stdenv.mkDerivation {
  pname = "membench";
  version = "0.0.1";
  src = fetchFromGitHub {
    owner = "nicktehrany";
    repo = "membench";
    rev = "91f4e5b142df05e501d8941b555d547ed4958152";
    sha256 = "sha256-5NLgwWbWViNBL1mQTXqoTnpwCNIC0lXoIeslWWnuXcE=";
  };
  enableParallelBuilding = true;
  makeFlags = [ "CC=${stdenv.cc.targetPrefix}cc" ];
  installPhase = ''
    mkdir -p $out/membench
    cp membench $out/membench/
  '';
  meta = {
    description = "Benchmarking Memory and File System Performance";
    mainProgram = "membench";
    longDescription = ''
      Benchmark to evaluate memory bandwidth/latency, page fault latency, and
      latency for mmap calls. Created for my BSc thesis on "Evaluating
      Performance Characteristics of the PMDK Persistent Memory Software Stack".
    '';
    homepage = "https://github.com/nicktehrany/membench";
    license = lib.licenses.mit;
    platforms = lib.platforms.unix;
  };
}
