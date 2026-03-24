# Fcntl Family Batch 13

## Goal

Re-enable Priority A cases `fcntl30` and `fcntl30_64`, with `/tmp` validation
first, then `/ext2` and `/exfat` serially.

## Problem Cause

`fcntl30*` was blocked by two missing kernel behaviors:

- LTP setup reads `/proc/sys/fs/pipe-max-size`; Asterinas procfs did not expose
  this file, so the testcase failed early with `TBROK`.
- `fcntl(F_GETPIPE_SZ)` was not implemented and returned `EINVAL`, while the
  testcase requires both `F_GETPIPE_SZ` and `F_SETPIPE_SZ`.

Recent fcntl lease-enablement commits were reviewed to avoid regressions in the
same syscall path; this batch is independent of lease semantics.

## Solution

- Add `/proc/sys/fs` and `/proc/sys/fs/pipe-max-size` in procfs (read-only,
  returning `1048576`).
- Implement `F_GETPIPE_SZ` in `sys_fcntl`, and plumb pipe-capacity getters from
  `InodeHandle` to `PipeHandle/PipeObj`.
- Re-enable `fcntl30` and `fcntl30_64` in `testcases/all.txt`.

## Validation Results

Commands:

```bash
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=fcntl30,fcntl30_64

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=fcntl30,fcntl30_64

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=fcntl30,fcntl30_64
```

Observed:

- `/tmp`: both cases `PASS`.
- `/ext2`: both cases `PASS`.
- `/exfat`: both cases `PASS`.
