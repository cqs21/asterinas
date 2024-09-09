#!/bin/bash

# to emulate the aliyun ecs

sudo qemu-system-x86_64 \
	-machine pc,accel=kvm -cpu Skylake-Server -smp 2 -m 4G \
	-bios /usr/share/qemu/OVMF.fd \
	-device piix3-usb-uhci,bus=pci.0,addr=01.2 \
	-device cirrus-vga \
	-chardev stdio,id=mux,mux=on,logfile=qemu.log \
	-device virtio-serial-pci,disable-modern=true \
	-drive file=./asterinas.qcow2,if=none,id=mydrive \
	-device virtio-blk-pci,disable-modern=true,drive=mydrive \
	-netdev user,id=net01,hostfwd=tcp::44624-:22,hostfwd=tcp::51859-:8080 \
	-device virtio-net-pci,netdev=net01,disable-modern=true \
	-device virtio-balloon,disable-modern=true \
	-serial chardev:mux \
	-vnc :42

#sudo qemu-system-x86_64 \
#       -vnc :42 -enable-kvm -smp 2 -cpu host -m 4G \
#       -bios /usr/share/qemu/OVMF.fd \
#       -drive file=./asterinas.qcow2,if=virtio
#      -chardev stdio,id=mux,mux=on,logfile=qemu.log -serial chardev:mux -device virtio-serial-pci,disable-legacy=on,disable-modern=off -device virtconsole,chardev=mux
#      -device isa-debug-exit,iobase=0xf4,iosize=0x04 -netdev user,id=net01,hostfwd=tcp::44624-:22,hostfwd=tcp::51859-:8080 -device virtio-net-pci,netdev=net01,disable-legacy=on,disable-modern=off
