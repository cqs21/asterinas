# Fcntl Family Batch 11

## Goal

Re-enable `fcntl26` and `fcntl26_64` in Phase 3, checking `/tmp` first and
then validating `/ext2` and `/exfat` serially.

## Problem Cause

`fcntl26*` stayed disabled while the earlier file-lease support work was still
in progress. With the recent `F_SETLEASE` support in place, these cases are no
longer blocked by a kernel bug.

On `/tmp`, upstream LTP intentionally reports these cases as `TCONF` on ramfs,
so the expected result there is a configuration skip rather than `PASS`.

## Solution

- Re-validate `fcntl26` and `fcntl26_64` on `/tmp`, `/ext2`, and `/exfat`.
- Confirm `/tmp` produces the expected ramfs `TCONF`.
- Confirm `/ext2` and `/exfat` both pass.
- Re-enable `fcntl26` and `fcntl26_64` in `testcases/all.txt`.

No additional kernel code change was required in this batch.

## Validation Results

Representative commands:

```bash
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=fcntl26,fcntl26_64

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=fcntl26,fcntl26_64

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=fcntl26,fcntl26_64
```

Observed results:

- `/tmp`: both cases `TCONF` (expected on ramfs), no failures.
- `/ext2`: both cases `PASS`.
- `/exfat`: both cases `PASS`.
