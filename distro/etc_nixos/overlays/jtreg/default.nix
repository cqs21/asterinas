final: prev: {
  jtreg = let
    # JT Harness
    jtharness = prev.stdenv.mkDerivation rec {
      pname = "jtharness";
      version = "6.0-b26";
      src = prev.fetchzip {
        url =
          "https://github.com/openjdk/jtharness/archive/refs/tags/jt${version}.zip";
        sha256 = "sha256-41PjFHBrtcNN/PgUmZQloE0oXBWEv9l6YqPIdVgpymo";
      };
      buildInputs = with prev.pkgs; [ ant openjdk21 ];
      buildCommand = ''
        mkdir -p $out

        BUILD_DIR=$(mktemp -d)
        ant -DBUILD_DIR=$BUILD_DIR -f $src/build/build.xml dist
        cp -r $BUILD_DIR/binaries/* $out
      '';
    };

    # AsmTools
    asmtools = prev.stdenv.mkDerivation rec {
      pname = "asmtools";
      version = "9.1-b01";
      src = prev.fetchzip {
        url =
          "https://github.com/openjdk/asmtools/archive/refs/tags/${version}.zip";
        sha256 = "sha256-fRlXq+c09MyMfVRoIEdx6egusWVDPiYRDjC3rPmRZTY";
      };
      buildInputs = with prev.pkgs; [ ant openjdk21 ];
      buildCommand = ''
        mkdir -p $out

        BUILD_DIR=$(mktemp -d)
        ant -DBUILD_DIR=$BUILD_DIR -f $src/build/build.xml release
        cp -r $BUILD_DIR/release/* $out
      '';
    };

    # JUnit Platform Console Standalone (includes JUnit Jupiter, JUnit Vintage, and dependencies)
    junit = prev.stdenv.mkDerivation rec {
      pname = "junit";
      version = "6.0.2";

      junit = prev.fetchurl {
        url =
          "https://repo1.maven.org/maven2/org/junit/platform/junit-platform-console-standalone/${version}/junit-platform-console-standalone-${version}.jar";
        sha256 = "sha256-5GibrpfZKCsESHHt+dfwsoRmuslQSKhb9148ZUMLzVA";
      };

      license = prev.fetchurl {
        url =
          "https://github.com/junit-team/junit-framework/raw/refs/tags/r${version}/LICENSE.md";
        sha256 = "sha256-WqTNRMERrdF40cLi/jbVikhAEsgBZ9+SX4Js1k1BG/A";
      };

      buildCommand = ''
        mkdir -p $out/lib

        cp ${license} $out/LICENSE.md
        cp ${junit} $out/lib/junit-platform-console-standalone.jar
      '';
    };

    # TestNG and its dependencies
    testng = prev.stdenv.mkDerivation rec {
      pname = "testng";
      version = "7.9.0";

      testng = prev.fetchurl {
        url =
          "https://repo1.maven.org/maven2/org/testng/testng/${version}/testng-${version}.jar";
        sha256 = "sha256-y5HychyUyBij7mSX4qTPtO1iuQXYjgSlxhUyWs/WWvk";
      };

      license = prev.fetchurl {
        url =
          "https://github.com/testng-team/testng/raw/refs/tags/${version}/LICENSE.txt";
        sha256 = "sha256-wbnfEnXnafPbqwANHkV6LUsPKOtdpsd+SNw37rogLtc";
      };

      slf4j-api = prev.fetchurl {
        url =
          "https://repo1.maven.org/maven2/org/slf4j/slf4j-api/1.7.36/slf4j-api-1.7.36.jar";
        sha256 = "sha256-0+9XXj5JeWeNwBvx3M5RAhSTtNEft/G+itmCh3wWocA";
      };

      jcommander = prev.fetchurl {
        url =
          "https://repo1.maven.org/maven2/com/beust/jcommander/1.82/jcommander-1.82.jar";
        sha256 = "sha256-3urBV8jeaCKHjYXQx7yEZ6GcyEhNN3iPeATwOd3igLE";
      };

      jquery = prev.fetchurl {
        url =
          "https://repo1.maven.org/maven2/org/webjars/jquery/3.7.1/jquery-3.7.1.jar";
        sha256 = "sha256-JiAW3TpVnfh67745KATpv2IHh8kgTAq4Ui1MIx6mUJc";
      };

      guice = prev.fetchurl {
        url =
          "https://repo1.maven.org/maven2/com/google/inject/guice/5.1.0/guice-5.1.0.jar";
        sha256 = "09mw2z48zmgyz12q5wwhkhj1mj3032wh7nghc349k064z85yac21";
      };

      snakeyaml = prev.fetchurl {
        url =
          "https://repo1.maven.org/maven2/org/yaml/snakeyaml/2.2/snakeyaml-2.2.jar";
        sha256 = "sha256-FGeTFEiggXaWrigFt7iyC/sIJlK/nE767VKJMNxJOJs";
      };

      buildCommand = ''
        mkdir -p $out/lib

        cp ${license} $out/LICENSE.txt
        cp ${testng} $out/lib/testng.jar
        cp ${slf4j-api} $out/lib/slf4j-api.jar
        cp ${jcommander} $out/lib/jcommander.jar
        cp ${jquery} $out/lib/jquery.jar
        cp ${guice} $out/lib/guice.jar
        cp ${snakeyaml} $out/lib/snakeyaml.jar
      '';
    };
  in prev.stdenv.mkDerivation rec {
    pname = "jtreg";
    version = "8.2.1+1";
    src = prev.fetchzip {
      url =
        "https://github.com/openjdk/jtreg/archive/refs/tags/jtreg-${version}.zip";
      sha256 = "sha256-psrvWeuYDQ6rUtwvf981057Q6Rd5UsBMSd1uVCp7Y6g";
    };

    patches = [ ./fix-tool-paths-and-build-version.patch ];

    JAVATEST_JAR = "${jtharness}/lib/javatest.jar";
    JTHARNESS_NOTICES = "${jtharness}/legal/copyright.txt ${jtharness}/LICENSE";

    ASMTOOLS_JAR = "${asmtools}/lib/asmtools.jar";
    ASMTOOLS_NOTICES = "${asmtools}/LICENSE";

    JUNIT_JARS = "${junit}/lib/junit-platform-console-standalone.jar";
    JUNIT_NOTICES = "${junit}/LICENSE.md";

    TESTNG_JARS =
      "${testng}/lib/testng.jar ${testng}/lib/slf4j-api.jar ${testng}/lib/jcommander.jar ${testng}/lib/jquery.jar ${testng}/lib/guice.jar ${testng}/lib/snakeyaml.jar";
    TESTNG_NOTICES = "${testng}/LICENSE.txt";

    JDKHOME = "${prev.pkgs.openjdk21}";
    JAVA_SPECIFICATION_VERSION = "21";

    buildInputs = with prev.pkgs; [
      ant
      openjdk21
      hostname
      pandoc
      perl
      html-tidy
      unzip
      zip
    ];
    buildPhase = ''
      make -C make
    '';
    installPhase = ''
      mkdir $out
      cp -r build/images/jtreg/* $out
    '';
  };
}
