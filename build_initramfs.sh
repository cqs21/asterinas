#!/bin/bash

target=${1:-x86_64}

nix-build --argstr target $target \
    --out-link initramfs initramfs.nix
