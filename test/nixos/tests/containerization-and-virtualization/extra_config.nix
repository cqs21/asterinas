{ config, lib, pkgs, ... }:

{
  environment.systemPackages = [ pkgs.skopeo ];
  virtualisation.podman.enable = true;
}
