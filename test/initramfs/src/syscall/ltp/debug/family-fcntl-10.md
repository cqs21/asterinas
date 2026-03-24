# Fcntl Family Batch 10

## Goal

Re-enable `fcntl25` and `fcntl25_64` in Phase 3, prioritizing `/tmp` behavior
first and then validating `/ext2` and `/exfat` serially.

## Problem Cause

`fcntl25*` had stayed disabled in `all.txt` while lease-related support was
being brought up in earlier fcntl batches. After the recent lease commits,
these cases are no longer blocked by a kernel defect.

On `/tmp`, this case is intentionally reported as `TCONF` by upstream LTP
because the test environment is ramfs and the case explicitly skips that
filesystem.

## Solution

- Re-validate `fcntl25` and `fcntl25_64` on `/tmp`, `/ext2`, and `/exfat`.
- Confirm `/tmp` result is expected `TCONF` rather than a failure.
- Confirm `/ext2` and `/exfat` both pass.
- Re-enable `fcntl25` and `fcntl25_64` in `testcases/all.txt`.

No kernel code change was required in this batch.

## Validation Results

Representative commands:

```bash
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=fcntl25,fcntl25_64

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=fcntl25,fcntl25_64

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=fcntl25,fcntl25_64
```

Observed results:

- `/tmp`: both cases `TCONF` (expected on ramfs), no failures.
- `/ext2`: both cases `PASS`.
- `/exfat`: both cases `PASS`.
