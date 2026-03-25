# sighold02

## Goal

Enable `sighold02` on `/tmp`.

## Result

`sighold02` already passes on `/tmp` without kernel changes. The testcase verifies that the
legacy `sighold()` interface blocks the expected signals.

## Root Cause

There was no kernel defect for this testcase. It remained commented out in `testcases/all.txt`
even though the current signal-mask implementation already satisfies the expected behavior.

## Fix

- Enabled `sighold02` in `testcases/all.txt`.
- Recorded the validation result for batch tracking.

## Validation

- `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp LTP_CASES=sighold02`
