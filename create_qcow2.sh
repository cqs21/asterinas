#!/bin/bash

sudo rm asterinas.qcow2 -f

sudo qemu-img create -f qcow2 asterinas.qcow2 10G
sudo qemu-nbd -c /dev/nbd0 asterinas.qcow2

sudo parted -s /dev/nbd0 \
	mklabel gpt \
	mkpart primary 1MiB 512MiB \
	set 1 esp on \
	quit

sudo mkfs.fat -F32 -n asterinas /dev/nbd0p1

mnt_dir=$(mktemp -d -t "mnt-XXXXXX")
sudo mount /dev/nbd0p1 $mnt_dir
sudo grub-install --efi-directory $mnt_dir --boot-directory $mnt_dir/boot --removable
sudo cp -r target/osdk/iso_root/boot/* $mnt_dir/boot/
tree $mnt_dir
sudo umount $mnt_dir
sudo rm -rf "$mnt_dir"

sudo qemu-nbd -d /dev/nbd0

