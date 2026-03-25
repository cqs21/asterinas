# openat203

## Goal

Enable `openat203` on `/tmp` and verify Linux-compatible `openat2(2)` error handling.

## Failure

Before `openat2(2)` support was added, the testcase was skipped with `TCONF` because the syscall
was not implemented.

## Root Cause

`openat203` checks the syscall boundary contract itself: invalid file descriptors, bad userspace
pointers, illegal flag/mode/resolve combinations, and oversized `open_how` layouts. None of these
paths were reachable until `openat2(2)` existed.

## Fix

- Added strict `open_how` size validation:
  zero or undersized layouts return `EINVAL`, inaccessible extension bytes return `EFAULT`, and
  non-zero extension bytes return `E2BIG`.
- Added strict validation for unsupported open flags, invalid modes without `O_CREAT/O_TMPFILE`,
  and unknown resolve bits.
- Preserved Linux-compatible error ordering so `EBADF`, `EFAULT`, `EINVAL`, and `E2BIG` match LTP.

## Validation

- `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp LTP_CASES=openat203`
