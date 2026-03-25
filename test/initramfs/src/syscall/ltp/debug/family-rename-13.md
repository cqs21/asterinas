# rename13

## Goal

Enable `rename13` on `/tmp`.

## Failure

No new kernel failure was reproduced on `/tmp`. The testcase expects `rename(path, path)` to
behave as a no-op, and Asterinas already matches that behavior.

## Root Cause

The case had remained disabled in `all.txt`, but current rename handling already preserves inode
identity and returns success for self-renames.

## Fix

- Enabled `rename13` in `testcases/all.txt`.
- No additional kernel code changes were required.

## Validation

- `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp LTP_CASES=rename13`
