# `statx04`

## Goal

Re-enable `statx04` in Phase 3 and make it pass on `/tmp`, then verify the
same fix on `/ext2` and `/exfat`.

## Root Cause

This testcase had two independent blockers:

1. LTP requested a free block device for `mount_device=1`, but the QEMU test
   machine only exposed `/dev/vda` and `/dev/vdb`, both already mounted for
   `/ext2` and `/exfat`. LTP therefore stopped early with `No free devices
   found`.
2. After a spare device was provided, the real kernel gap appeared:
   `FS_IOC_GETFLAGS` returned `ENOTTY` on inode-backed files and directories,
   and `statx()` only advertised `STATX_ATTR_MOUNT_ROOT`. Linux exposes inode
   flag support for tmpfs here, so LTP expected `STATX_ATTR_APPEND`,
   `STATX_ATTR_IMMUTABLE`, and `STATX_ATTR_NODUMP` in
   `stx_attributes_mask`.

## Solution

- Added a third unmounted scratch disk image to the initramfs/QEMU test setup.
- Taught the LTP wrapper to auto-export the first unused `/dev/vd*` as
  `LTP_DEV`, so `mount_device=1` tests can run without loop-device support.
- Implemented `FS_IOC_GETFLAGS` for inode-backed files:
  returning real ext2 inode flags and a zero-valued tmpfs flag set.
- Extended `statx()` attribute reporting:
  tmpfs/ramfs now advertise append/immutable/nodump support, and ext2 maps its
  inode flags to the corresponding `statx` attribute bits.
- Re-enabled `statx04` in `testcases/all.txt`.

## Validation

```bash
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=statx04

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=statx04

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=statx04
```

Observed results:

- `/tmp`: `PASS`
- `/ext2`: `PASS`
- `/exfat`: `PASS`

## Impact / Residual Risk

- This fixes the immediate `statx04` compatibility gap and also unblocks later
  LTP cases that need an unused block device through `LTP_DEV`.
- Tmpfs currently advertises inode-flag support through `FS_IOC_GETFLAGS` and
  `statx()` but still reports all flags as clear. If future tests start setting
  or mutating these flags, Asterinas will need `FS_IOC_SETFLAGS` and persistent
  inode-flag storage for non-ext2 filesystems.
