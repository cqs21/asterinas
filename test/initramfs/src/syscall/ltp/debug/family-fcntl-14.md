# Fcntl Family Batch 14

## Goal

Re-enable Priority A cases `fcntl31` and `fcntl31_64`, with `/tmp` validation
first, then `/ext2` and `/exfat` serially.

## Problem Cause

`fcntl31*` exercises async I/O ownership control and signal selection. Asterinas
only supported the older `F_GETOWN` and `F_SETOWN` path partially, and still
missed several Linux behaviors that the testcase depends on:

- `F_SETOWN` did not preserve Linux's negative-value process-group semantics.
- `F_GETOWN_EX` and `F_SETOWN_EX` were unimplemented.
- `F_GETSIG` and `F_SETSIG` were unimplemented.
- Async notification ownership was tracked as a plain process ID, so the kernel
  could not deliver notifications to a thread ID or a process group.

## Solution

- Extend file-table async ownership tracking to distinguish thread, process, and
  process-group targets.
- Re-register the async poll observer when either the owner or configured signal
  changes.
- Implement `F_GETOWN_EX`, `F_SETOWN_EX`, `F_GETSIG`, and `F_SETSIG`.
- Fix `F_SETOWN` and `F_GETOWN` to follow Linux-compatible process-group
  encoding.
- Re-enable `fcntl31` and `fcntl31_64` in `testcases/all.txt`.

## Validation Results

Commands:

```bash
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=fcntl31,fcntl31_64

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=fcntl31,fcntl31_64

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=fcntl31,fcntl31_64
```

Observed:

- `/tmp`: both cases `PASS`.
- `/ext2`: both cases `PASS`.
- `/exfat`: both cases `PASS`.
