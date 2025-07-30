{ pkgs, linux_vdso, initramfsCompressed, ... }: rec {
  nixos = import "${pkgs.path}/nixos/lib/eval-config.nix" {
    modules = [ ./configuration.nix ];
  };

  stage-1-image = pkgs.callPackage ./stage-1-initramfs.nix {
    inherit linux_vdso;
    compressed = initramfsCompressed;
  };

  stage-2-image = pkgs.callPackage ./stage-2-rootfs.nix {
    toplevel = nixos.config.system.build.toplevel;
  };
}
