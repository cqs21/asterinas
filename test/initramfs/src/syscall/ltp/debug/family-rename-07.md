# rename07

## Goal

Enable `rename07` on `/tmp`.

## Failure

No new kernel failure was reproduced on `/tmp`. The testcase already returned the expected
`ENOTDIR` when renaming a directory onto a non-directory path.

## Root Cause

The case had remained disabled in `all.txt`, but current VFS rename semantics are already
compatible with the LTP expectation for this scenario.

## Fix

- Enabled `rename07` in `testcases/all.txt`.
- No kernel code changes were required.

## Validation

- `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp LTP_CASES=rename07`
