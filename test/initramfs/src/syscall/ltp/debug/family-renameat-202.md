# renameat202

## Goal

Enable `renameat202` on `/tmp`.

## Failure

No new kernel failure remained after adding `renameat2()` support for `RENAME_EXCHANGE`. The
testcase's repeated exchange checks all passed on `/tmp`.

## Root Cause

The case had remained disabled in `all.txt`, but the `renameat2()` support added for
`renameat201` already covers the pathname exchange behavior exercised here.

## Fix

- Enabled `renameat202` in `testcases/all.txt`.
- No additional kernel code changes were required beyond the prior `renameat2()` implementation.

## Validation

- `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp LTP_CASES=renameat202`
