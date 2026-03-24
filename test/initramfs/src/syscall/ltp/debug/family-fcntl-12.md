# Fcntl Family Batch 12

## Goal

Re-enable `fcntl27` and `fcntl27_64` in Phase 3 after confirming the current
lease behavior on `/tmp`, `/ext2`, and `/exfat`.

## Problem Cause

These cases remained disabled only because the earlier file-lease support work
had not yet been revalidated against them. `fcntl27*` expects
`F_SETLEASE(F_RDLCK)` to fail with `EAGAIN` on `O_RDWR` and `O_WRONLY`
descriptors.

The current lease validation added for the preceding fcntl batches already
matches that expectation, so there was no new kernel bug in this batch.

## Solution

- Re-validate `fcntl27` and `fcntl27_64` on `/tmp`, `/ext2`, and `/exfat`.
- Confirm both cases return the expected `EAGAIN/EWOULDBLOCK` result on all
  three workdirs.
- Re-enable `fcntl27` and `fcntl27_64` in `testcases/all.txt`.

No additional kernel code change was required in this batch.

## Validation Results

Representative commands:

```bash
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=fcntl27,fcntl27_64

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=fcntl27,fcntl27_64

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=fcntl27,fcntl27_64
```

Observed results:

- `/tmp`: both cases `PASS`.
- `/ext2`: both cases `PASS`.
- `/exfat`: both cases `PASS`.
