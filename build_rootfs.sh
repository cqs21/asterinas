#!/bin/bash

target="${1:-x86_64}"

if [[ "${target}" == "x86_64" ]]; then
    HOST_PLATFORM="x86_64-linux"
elif [[ "${target}" == "riscv64" ]]; then
    HOST_PLATFORM="riscv64-linux"
else
    echo "Unknown target: ${target}"
    exit 1
fi

export HOST_PLATFORM

nix-build --out-link rootfs rootfs.nix
