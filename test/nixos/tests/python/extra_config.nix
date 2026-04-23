{ pkgs, ... }:

let py = pkgs.python312;
in {
  environment.systemPackages = [ py ];
  # Make the exact matching source tree available without a download.
  environment.etc."python-src.tar.xz".source = py.src;
}
