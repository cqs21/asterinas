# `rename05`

## Goal

Re-enable `rename05` in Phase 3 and verify that it passes on `/tmp`, `/ext2`,
and `/exfat`.

## Root Cause

There was no remaining kernel defect in this case. `rename05` checks the
negative path where `rename(file, existing_dir)` must fail with `EISDIR`.

The current VFS path already returns Linux-compatible `EISDIR` on all tested
targets. The testcase had simply remained disabled in
`testcases/all.txt`.

## Solution

- Re-ran `rename05` on `/tmp`, `/ext2`, and `/exfat`.
- Confirmed that the existing implementation already returns the expected
  `EISDIR` errno.
- Re-enabled `rename05` in `testcases/all.txt`.

## Validation

```bash
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=rename05

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=rename05

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=rename05
```

Observed results:

- `/tmp`: `PASS`
- `/ext2`: `PASS`
- `/exfat`: `PASS`

Observed errno on all three runs:

- `rename()` rejected the operation with `EISDIR`

## Impact / Residual Risk

- This extends active rename-family negative-path coverage without a kernel
  code change.
- The remaining Priority A rename todo is `rename06`, which checks rejecting
  renaming a directory into its own subdirectory.
