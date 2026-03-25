# rename15

## Goal

Enable `rename15` on `/tmp`.

## Failure

No new kernel failure was reproduced on `/tmp`. The testcase validates rename behavior on
symlinks, including symlinks to existing, missing, and later-created targets, and all checks
already passed.

## Root Cause

The case had remained disabled in `all.txt`, but current pathname and symlink rename semantics
already match the LTP expectations on `/tmp`.

## Fix

- Enabled `rename15` in `testcases/all.txt`.
- No additional kernel code changes were required.

## Validation

- `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp LTP_CASES=rename15`
