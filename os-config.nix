{ config, lib, pkgs, ... }: {
  nixpkgs.buildPlatform.system = builtins.currentSystem;
  nixpkgs.hostPlatform.system = if builtins.getEnv "HOST_PLATFORM" == "" then
    builtins.currentSystem
  else
    builtins.getEnv "HOST_PLATFORM";
  nixpkgs.config.allowUnfree = true;

  imports = [ ./asterinas.nix ];

  boot.loader.grub.enable = false;
  boot.loader.systemd-boot.enable = false;
  boot.initrd.enable = false;
  boot.kernel.enable = false;
  environment.defaultPackages = [ ];

  systemd.enableCgroupAccounting = false;

  system.stateVersion = "24.11";
}
