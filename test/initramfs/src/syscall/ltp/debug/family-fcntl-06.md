# Fcntl Family Batch 06

## Goal

Re-enable `fcntl20` and `fcntl20_64` in Phase 3 after confirming the existing
kernel behavior is already correct on `/tmp`, `/ext2`, and `/exfat`.

## Problem Cause

This batch was not blocked by a kernel bug. The two cases were still commented
out in `all.txt` because they had not yet been revalidated end to end after the
earlier `fcntl` work.

`fcntl20*` exercises a set of record-lock state transitions across seven small
blocks. The current `fcntl()` and range-lock behavior already matches the LTP
expectation on all three workdirs.

## Solution

- Run `fcntl20` and `fcntl20_64` on `/tmp`, `/ext2`, and `/exfat`.
- Confirm every block passes on every workdir.
- Re-enable both cases in `testcases/all.txt`.

No kernel code changes or filesystem-specific blocklists were needed.

## Validation Results

Representative commands:

```bash
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=fcntl20,fcntl20_64

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=fcntl20,fcntl20_64

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=fcntl20,fcntl20_64
```

Observed results:

- `/tmp`: `fcntl20` and `fcntl20_64` both `PASS`.
- `/ext2`: `fcntl20` and `fcntl20_64` both `PASS`.
- `/exfat`: `fcntl20` and `fcntl20_64` both `PASS`.
