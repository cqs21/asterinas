# Fcntl Family Batch 04

## Goal

Re-enable the next small `fcntl` batch in Phase 3 by validating
`fcntl18`, `fcntl18_64`, `fcntl19`, and `fcntl19_64` on `/tmp`, `/ext2`,
and `/exfat`.

## Problem Cause

This batch was not blocked by a new kernel bug. The cases were still commented
out in `all.txt` because they had not been revalidated end to end.

- `fcntl18*` checks `F_GETLK` error paths, including `EFAULT` and `EINVAL`.
- `fcntl19*` checks record-lock consistency across a small set of lock-state
  transitions.

The current `fcntl()` and range-lock implementation already provides the
expected behavior for these paths.

## Solution

- Run the batch on `/tmp`, `/ext2`, and `/exfat`.
- Confirm all four testcases pass on all three workdirs.
- Re-enable the four cases in `testcases/all.txt`.

No kernel code changes or filesystem-specific blocklist updates were needed for
this batch.

## Validation Results

Representative commands:

```bash
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=fcntl18,fcntl18_64,fcntl19,fcntl19_64

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=fcntl18,fcntl18_64,fcntl19,fcntl19_64

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=fcntl18,fcntl18_64,fcntl19,fcntl19_64
```

Observed results:

- `/tmp`: `fcntl18`, `fcntl18_64`, `fcntl19`, and `fcntl19_64` all `PASS`.
- `/ext2`: `fcntl18`, `fcntl18_64`, `fcntl19`, and `fcntl19_64` all `PASS`.
- `/exfat`: `fcntl18`, `fcntl18_64`, `fcntl19`, and `fcntl19_64` all `PASS`.

## Follow-up

- `fcntl17` and `fcntl17_64` remain out of this batch. A quick `/tmp`
  investigation showed they depend on Linux-compatible deadlock detection for
  `F_SETLKW` and currently fail because the expected `EDEADLK` path is missing.
- Later `fcntl` batches should keep treating `fcntl17*` as a separate semantic
  fix instead of mixing it into straight re-enable work.
