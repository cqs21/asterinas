# openat201

## Goal

Enable `openat201` on `/tmp` by implementing the baseline `openat2(2)` syscall path.

## Failure

Before the fix, the testcase was skipped with `TCONF` because `__NR_openat2` returned
`ENOSYS`.

## Root Cause

The kernel did not implement `openat2(2)` at all, so LTP could not exercise either the
`open_how` argument validation or the basic resolve-flag semantics.

## Fix

- Added `sys_openat2` and wired syscall number `437` into the x86 and generic syscall tables.
- Parsed `struct open_how` from userspace, including Linux-compatible handling for oversized
  zero-padded structures.
- Enforced strict `openat2` flag, mode, and resolve validation instead of silently truncating
  unknown bits.
- Extended path lookup to honor `RESOLVE_NO_XDEV`, `RESOLVE_NO_MAGICLINKS`,
  `RESOLVE_NO_SYMLINKS`, `RESOLVE_BENEATH`, and `RESOLVE_IN_ROOT`.

## Validation

- `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp LTP_CASES=openat201`
