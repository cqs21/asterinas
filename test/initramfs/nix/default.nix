{
  target ? "x86_64",
  enableBenchmarkTest ? false,
  enableConformanceTest ? false,
  enableRegressionTest ? false,
  conformanceTestSuite ? "ltp",
  conformanceTestWorkDir ? "/tmp",
  regressionTestPlatform ? "asterinas",
  dnsServer ? "none",
  smp ? 1,
  initramfsCompressed ? true,
}:
let
  crossSystem.config =
    if target == "x86_64" then
      "x86_64-unknown-linux-gnu"
    else if target == "riscv64" then
      "riscv64-unknown-linux-gnu"
    else
      throw "Target arch ${target} not yet supported.";

  # Pinned nixpkgs (nix version: 2.29.1, channel: nixos-25.05, release date: 2025-07-01)
  nixpkgs = fetchTarball {
    url = "https://github.com/NixOS/nixpkgs/archive/8c50a710ddca43d7a530fb805ad55bde8d0141c5.tar.gz";
    sha256 = "0am8xx09fx5yf2p0wb001v0jx1g5hrfb76h4r37xph378jgk7pcr";
  };
  pkgs = import nixpkgs {
    config = { };
    overlays = [ ];
    inherit crossSystem;
  };
in
rec {
  # Packages needed by initramfs
  busybox = pkgs.busybox;
  benchmark = pkgs.callPackage ./benchmark { };
  conformance = pkgs.callPackage ./conformance {
    inherit smp;
    testSuite = conformanceTestSuite;
    workDir = conformanceTestWorkDir;
  };
  regression = pkgs.callPackage ./regression { testPlatform = regressionTestPlatform; };

  initramfs = pkgs.callPackage ./initramfs.nix {
    inherit busybox;
    benchmark = if enableBenchmarkTest then benchmark else null;
    conformance = if enableConformanceTest then conformance else null;
    regression = if enableRegressionTest then regression else null;
    dnsServer = dnsServer;
  };
  initramfs-image = pkgs.callPackage ./initramfs-image.nix {
    inherit initramfs;
    compressed = initramfsCompressed;
  };

  # Packages needed by host
  apacheHttpd = pkgs.apacheHttpd;
  iperf3 = pkgs.iperf3;
  libmemcached = pkgs.libmemcached.overrideAttrs (_: {
    configureFlags = [ "--enable-memaslap" ];
    LDFLAGS = "-lpthread";
    CPPFLAGS = "-fcommon -fpermissive";
  });
  lmbench = pkgs.callPackage ./benchmark/lmbench.nix { };
  redis =
    (pkgs.redis.overrideAttrs (_: {
      doCheck = false;
    })).override
      {
        withSystemd = false;
      };
}
