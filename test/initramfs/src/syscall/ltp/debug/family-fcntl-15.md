# Fcntl Family Batch 15

## Goal

Re-enable Priority A cases `fcntl32` and `fcntl32_64`, with `/tmp` validation
first, then `/ext2` and `/exfat` serially.

## Problem Cause

`fcntl32*` checks that `fcntl(F_SETLEASE, F_WRLCK)` fails with `EBUSY` or
`EAGAIN` when the same inode is already open through another file descriptor.
Asterinas tracked only the current lease owner, but it did not track other live
open file descriptions on the inode. As a result, the kernel incorrectly granted
the write lease even when a second open descriptor already existed.

## Solution

- Track live open file descriptions in `FsLockContext`.
- Register each `InodeHandle` on open and unregister it on drop.
- Reject `F_SETLEASE` with `EBUSY` when another open file description still
  exists for the same inode.
- Re-enable `fcntl32` and `fcntl32_64` in `testcases/all.txt`.

## Validation Results

Commands:

```bash
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=fcntl32,fcntl32_64

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=fcntl32,fcntl32_64

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=fcntl32,fcntl32_64
```

Observed:

- `/tmp`: both cases `TCONF` on `ramfs`, `Total Failures: 0`.
- `/ext2`: both cases `PASS`.
- `/exfat`: both cases `PASS`.
