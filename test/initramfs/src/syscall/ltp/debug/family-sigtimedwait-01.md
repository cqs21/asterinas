# sigtimedwait01

## Goal

Enable `sigtimedwait01` on `/tmp`.

## Result

`sigtimedwait01` already passes on the current kernel without code changes. The testcase covers
normal wakeups, timeout handling, `siginfo_t` contents, original-mask restoration, and `EFAULT`
cases.

## Root Cause

There was no kernel defect for this testcase. It remained commented out in `testcases/all.txt`
even though the current implementation already satisfies the expected behavior.

## Fix

- Enabled `sigtimedwait01` in `testcases/all.txt`.
- Recorded the validation result for future signal-family tracking.

## Validation

- `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp LTP_CASES=sigtimedwait01`
