# renameat01

## Goal

Assess whether `renameat01` needs a `/tmp`-side fix before enabling it in the Priority A set.

## Failure

`renameat01` did not reach its rename-at assertions. The testcase failed while preparing an ext2
test device:

- `mkfs.ext2: lseek(0, 2): Invalid argument`
- `renameat01.c:177: mkfs.ext2 failed with exit code 1`

## Root Cause

This case is blocked in the ext2-device setup path rather than by `/tmp` or `renameat()` lookup
semantics. The failure happens before the testcase exercises the target syscall behavior.

## Resolution

- Left `renameat01` disabled in `testcases/all.txt`.
- Recorded the blocker instead of changing ext2-specific behavior, following the current priority
  to finish `/tmp` coverage first.

## Validation

- `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp LTP_CASES=renameat01`
