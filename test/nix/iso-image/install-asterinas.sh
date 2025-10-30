#!/bin/sh

# SPDX-License-Identifier: MPL-2.0

if [ $# -eq 0 ]; then
    echo "Usage: $0 <device_name>"
    echo "Example: $0 /dev/sda"
    exit 1
fi

INSTALL_DEVICE="$1"
if [ ! -b "$INSTALL_DEVICE" ]; then
    echo "Error: '$INSTALL_DEVICE' does not exist or is not a block device"
    exit 1
fi

parted $INSTALL_DEVICE -- mklabel gpt
parted $INSTALL_DEVICE -- mkpart ESP fat32 1MB 512MB
parted $INSTALL_DEVICE -- mkpart root ext2 512MB 100%
parted $INSTALL_DEVICE -- set 1 esp on

mkfs.fat -F 32 -n boot "${INSTALL_DEVICE}1"
mkfs.ext2 -L nixos "${INSTALL_DEVICE}2"

mount -o sync,dirsync "${INSTALL_DEVICE}2" /mnt
mkdir -p /mnt/boot
mount -o umask=077,sync,dirsync "${INSTALL_DEVICE}1" /mnt/boot

mkdir -p /mnt/etc/nixos
cp /asterinas/configuration.nix /mnt/etc/nixos/configuration.nix

nixos-install --no-root-passwd

umount /mnt/boot
umount /mnt

exit 0
