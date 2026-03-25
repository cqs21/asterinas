# rename08

## Goal

Enable `rename08` on `/tmp`.

## Failure

No new kernel failure was reproduced on `/tmp`. The testcase already returned the expected
`EFAULT` when either source or destination path pointer was invalid.

## Root Cause

The case had remained disabled in `all.txt`, but current syscall argument validation already
matches the LTP expectation for invalid userspace path pointers.

## Fix

- Enabled `rename08` in `testcases/all.txt`.
- No kernel code changes were required.

## Validation

- `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp LTP_CASES=rename08`
