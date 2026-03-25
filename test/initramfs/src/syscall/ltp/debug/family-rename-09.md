# rename09

## Goal

Enable `rename09` on `/tmp`.

## Failure

No new kernel failure was reproduced on `/tmp`. The testcase already returned the expected
`EACCES` for rename permission denial.

## Root Cause

The case had remained disabled in `all.txt`, but current VFS permission checks already match the
LTP expectation for this rename scenario.

## Fix

- Enabled `rename09` in `testcases/all.txt`.
- No kernel code changes were required.

## Validation

- `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp LTP_CASES=rename09`
