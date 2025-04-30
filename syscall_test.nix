{ pkgs ? import <nixpkgs> { }, }:
let ltp = pkgs.callPackage ./ltp.nix { inherit pkgs; };
in pkgs.stdenv.mkDerivation {
  pname = "syscall_test";
  version = "0.1.0";
  src = pkgs.lib.fileset.toSource {
    root = ./.;
    fileset = ./whitelist.txt;
  };

  buildCommand = ''
    mkdir -p $out/opt/ltp/testcases/bin
    mkdir -p $out/opt/ltp/runtest

    while IFS= read -r case; do
      # skip invalid lines
      if [ -z "$case" ] || [[ "$case" =~ ^# ]]; then
        continue
      fi

      matching_line=$(grep -E "^$case\s" ${ltp}/runtest/syscalls)
      if [ -z "$matching_line" ]; then
        echo "Warning: Test case $case not found in ${ltp}/runtest/syscalls" >&2
        continue
      fi

      bin_file=$(echo "$matching_line" | awk '{print $2}')
      if [ -z "$bin_file" ]; then
        echo "Warning: Parsing bin file for $case failed" >&2
        continue
      fi

      src_file="${ltp}/testcases/bin/$bin_file"
      if [ -f "$src_file" ]; then
        cp -u "$src_file" $out/opt/ltp/testcases/bin/
      else
        echo "Warning: Test case $case binary not found in ${ltp}/testcases/bin" >&2
      fi

      echo "$matching_line" >> $out/opt/ltp/runtest/syscalls
    done < $src/whitelist.txt

    cp -r ${ltp}/bin $out/opt/ltp/
    cp -r ${ltp}/runltp $out/opt/ltp/
    cp -r ${ltp}/Version $out/opt/ltp/
    cp -r ${ltp}/ver_linux $out/opt/ltp/
    cp -r ${ltp}/IDcheck.sh $out/opt/ltp/
  '';
}
