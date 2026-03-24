# Fcntl Family Batch 17

## Goal

Re-enable Priority A cases `fcntl34` and `fcntl34_64`, with `/tmp` validation
first, then `/ext2` and `/exfat` serially.

## Problem Cause

`fcntl34*` exercises open-file-description locks with `F_OFD_SETLKW`. Asterinas
still rejected command `38` with `EINVAL`, because the OFD fcntl commands were
not wired into `sys_fcntl`. The existing range-lock implementation was also
process-owned, which is correct for classic POSIX record locks but wrong for OFD
locks: multiple threads in the same process would never conflict with each
other, so the testcase could not use OFD locks to serialize file appends.

## Solution

- Add `F_OFD_GETLK`, `F_OFD_SETLK`, and `F_OFD_SETLKW` to `sys_fcntl`.
- Extend range-lock ownership to distinguish process-owned locks from
  open-file-description-owned locks.
- Route OFD lock requests through the `InodeHandle` owner ID so each open file
  description gets an independent lock owner.
- Release OFD locks automatically when the owning `InodeHandle` is dropped.
- Re-enable `fcntl34` and `fcntl34_64` in `testcases/all.txt`.

## Validation Results

Commands:

```bash
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=fcntl34,fcntl34_64

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=fcntl34,fcntl34_64

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=fcntl34,fcntl34_64
```

Observed:

- `/tmp`: both cases `PASS`.
- `/ext2`: both cases `PASS`.
- `/exfat`: both cases `PASS`.
