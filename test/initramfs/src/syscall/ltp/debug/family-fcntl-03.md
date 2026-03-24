# Fcntl Family Batch 03

## Goal

Re-enable `fcntl11` and `fcntl11_64` in Phase 3 and verify they pass on
`/tmp`, `/ext2`, and `/exfat`.

Enabled cases in this batch:

- `fcntl11`
- `fcntl11_64`

## Problem Cause

This batch targets POSIX record-lock interactions (`F_GETLK`, `F_SETLK`,
`F_SETLKW`) across overlapping regions and process boundaries.

There was no new kernel blocker in this round. The cases were still commented
out in `testcases/all.txt` because they had not been revalidated recently.

## Solution

- Ran `fcntl11` and `fcntl11_64` on `/tmp`, `/ext2`, and `/exfat`.
- Confirmed all three workdirs pass without kernel code changes.
- Re-enabled `fcntl11` and `fcntl11_64` in `testcases/all.txt`.

Files changed for this batch:

- `test/initramfs/src/syscall/ltp/testcases/all.txt`

## Validation Results

Representative commands:

```bash
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=fcntl11,fcntl11_64

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=fcntl11,fcntl11_64

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=fcntl11,fcntl11_64
```

Observed results:

- `/tmp`: `fcntl11` `PASS`, `fcntl11_64` `PASS`, `Total Failures: 0`
- `/ext2`: `fcntl11` `PASS`, `fcntl11_64` `PASS`, `Total Failures: 0`
- `/exfat`: `fcntl11` `PASS`, `fcntl11_64` `PASS`, `Total Failures: 0`

## Follow-up

- Continue with the next `fcntl` disabled pair (`fcntl17`, `fcntl17_64`) in a
  separate sub-batch.
