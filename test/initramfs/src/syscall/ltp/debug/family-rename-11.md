# rename11

## Goal

Assess whether `rename11` needs a `/tmp`-side kernel fix before enabling it in the Priority A
set.

## Failure

`rename11` did not reach the rename assertions on `/tmp`. The testcase tried to prepare an ext2
test device and failed during formatting:

- `mkfs.ext2: lseek(0, 2): Invalid argument`
- `rename11.c:107: mkfs.ext2 failed with exit code 1`

## Root Cause

This case is currently blocked in the ext2-device setup path rather than by `/tmp` rename
semantics. The failure happens before the testcase exercises the target rename behavior, so there
is no `/tmp` VFS regression to fix for this item.

## Resolution

- Left `rename11` disabled in `testcases/all.txt`.
- Recorded the blocker instead of changing ext2-specific behavior, per the current priority to
  finish `/tmp` coverage first.

## Validation

- `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp LTP_CASES=rename11`
