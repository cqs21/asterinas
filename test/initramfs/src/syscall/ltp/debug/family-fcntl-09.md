# Fcntl Family Batch 09

## Goal

Re-enable `fcntl24` and `fcntl24_64` in Phase 3, with `/tmp` pass-first
validation and cross-checks on `/ext2` and `/exfat`.

## Problem Cause

`fcntl24*` exercises `F_SETLEASE` with `F_WRLCK`. Before the lease support
work, these commands returned `EINVAL`, so the cases were disabled.

On `/tmp` specifically, LTP marks these cases as configuration skip (`TCONF`)
because the test runtime detects `ramfs` and intentionally does not run the
lease assertions there.

## Solution

- Reuse the lease support added for the same fcntl family in kernel fcntl path:
  `F_SETLEASE` and `F_GETLEASE`, with inode-scoped lease state.
- Validate `fcntl24` and `fcntl24_64` on `/tmp`, `/ext2`, and `/exfat`.
- Re-enable `fcntl24` and `fcntl24_64` in `testcases/all.txt`.

## Validation Results

Representative commands:

```bash
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=fcntl24,fcntl24_64

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=fcntl24,fcntl24_64

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=fcntl24,fcntl24_64
```

Observed results:

- `/tmp`: both cases `TCONF` (expected by LTP on ramfs), no failures.
- `/ext2`: both cases `PASS`.
- `/exfat`: both cases `PASS`.
