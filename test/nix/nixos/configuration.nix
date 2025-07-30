{ config, lib, pkgs, ... }: {
  boot.loader.grub.enable = false;
  boot.loader.systemd-boot.enable = false;
  boot.initrd.enable = false;
  boot.kernel.enable = false;
  environment.defaultPackages = [ ];

  systemd.enableCgroupAccounting = false;

  system.stateVersion = "25.05";
}
