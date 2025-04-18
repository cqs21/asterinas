#!/bin/bash

target="${1:-x86_64}"

if [[ "${target}" == "x86_64" ]]; then
    host="x86_64-unknown-linux-gnu"
elif [[ "${target}" == "riscv64" ]]; then
    host="riscv64-unknown-linux-gnu"
else
    echo "Unknown target: ${target}"
    exit 1
fi

nix-build --arg crossSystem "{ config = \"${host}\"; }" \
    --out-link initramfs initramfs.nix
