# sigwait01

## Goal

Enable `sigwait01` on `/tmp`.

## Result

`sigwait01` already passes on the current kernel without code changes. The testcase verifies that
signal-waiting APIs are interrupted by the expected signal and that the original mask is restored.

## Root Cause

There was no kernel defect for this testcase. It remained commented out in `testcases/all.txt`
even though the current implementation already satisfies the expected behavior.

## Fix

- Enabled `sigwait01` in `testcases/all.txt`.
- Recorded the validation result for future batch tracking.

## Validation

- `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp LTP_CASES=sigwait01`
