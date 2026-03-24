# `rename04`

## Goal

Re-enable `rename04` in Phase 3 and verify that it passes on `/tmp`, `/ext2`,
and `/exfat`.

## Root Cause

There was no remaining kernel defect in this case. `rename04` checks the
negative path where a directory is renamed onto a non-empty directory and Linux
must reject the operation with `ENOTEMPTY` or `EEXIST`.

The current VFS path already returns `ENOTEMPTY`, which satisfies LTP across
the tested targets. The testcase had simply remained disabled in
`testcases/all.txt`.

## Solution

- Re-ran `rename04` on `/tmp`, `/ext2`, and `/exfat`.
- Confirmed that the existing implementation already matches the expected
  Linux-compatible errno behavior.
- Re-enabled `rename04` in `testcases/all.txt`.

## Validation

```bash
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=rename04

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=rename04

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=rename04
```

Observed results:

- `/tmp`: `PASS`
- `/ext2`: `PASS`
- `/exfat`: `PASS`

Observed errno on all three runs:

- `rename()` rejected the operation with `ENOTEMPTY`

## Impact / Residual Risk

- This extends active rename-family negative-path coverage without a kernel
  code change.
- Priority A rename work now narrows to `rename05` and `rename06`, which check
  cross-type and self-subdirectory rejection paths.
