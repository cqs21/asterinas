# Fcntl Family Batch 07

## Goal

Re-enable `fcntl21` and `fcntl21_64` in Phase 3 after validating the current
kernel behavior on `/tmp`, `/ext2`, and `/exfat`.

## Problem Cause

This batch was still disabled only because it had not yet been revalidated
after the earlier `fcntl` bring-up work. The current lock-handling behavior
already satisfies the eleven `fcntl21*` sub-blocks on all three workdirs.

No new kernel bug or filesystem-specific incompatibility was observed.

## Solution

- Run `fcntl21` and `fcntl21_64` on `/tmp`, `/ext2`, and `/exfat`.
- Confirm every test block passes on every workdir.
- Re-enable both cases in `testcases/all.txt`.

No kernel code changes or filesystem blocklist updates were needed.

## Validation Results

Representative commands:

```bash
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=fcntl21,fcntl21_64

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=fcntl21,fcntl21_64

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=fcntl21,fcntl21_64
```

Observed results:

- `/tmp`: `fcntl21` and `fcntl21_64` both `PASS`.
- `/ext2`: `fcntl21` and `fcntl21_64` both `PASS`.
- `/exfat`: `fcntl21` and `fcntl21_64` both `PASS`.
