#!/bin/bash

VNC_PORT=${1:-42}
SSH_RAND_PORT=$(shuf -i 1024-65535 -n 1)
NGINX_RAND_PORT=$(shuf -i 1024-65535 -n 1)

# Emulate Aliyun ECS
sudo qemu-system-x86_64 \
	-machine pc-i440fx-2.1,accel=kvm -cpu host -smp 2 -m 4G \
	-bios /usr/share/qemu/OVMF.fd \
	-device piix3-usb-uhci,bus=pci.0,addr=01.2 \
	-device cirrus-vga \
	-chardev stdio,id=mux,mux=on,logfile=qemu.log \
	-device virtio-serial-pci,disable-modern=true \
	-drive file=./asterinas.qcow2,if=none,id=mydrive \
	-device virtio-blk-pci,disable-modern=true,drive=mydrive \
	-netdev user,id=net01,hostfwd=tcp::${SSH_RAND_PORT}-:22,hostfwd=tcp::${NGINX_RAND_PORT}-:8080 \
	-device virtio-net-pci,netdev=net01,disable-modern=true \
	-device virtio-balloon,disable-modern=true \
	-serial chardev:mux \
	-vnc :${VNC_PORT}

