# sigwaitinfo01

## Goal

Enable `sigwaitinfo01` on `/tmp`.

## Result

`sigwaitinfo01` already passes on the current kernel without code changes. The testcase validates
expected wakeup behavior, `siginfo_t` contents, original-mask restoration, and `EFAULT` handling.

## Root Cause

There was no kernel defect for this testcase. It remained commented out in `testcases/all.txt`
even though the current implementation already satisfies the expected behavior.

## Fix

- Enabled `sigwaitinfo01` in `testcases/all.txt`.
- Recorded the validation result for future signal-family tracking.

## Validation

- `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp LTP_CASES=sigwaitinfo01`
