# renameat201

## Goal

Enable `renameat201` on `/tmp`.

## Failure

`renameat201` exercises `renameat2()` with `RENAME_NOREPLACE` and `RENAME_EXCHANGE`. Asterinas
previously returned `EINVAL` for those valid flag combinations, so the testcase could not reach the
expected `EEXIST`, `ENOENT`, and success cases.

## Root Cause

`sys_renameat2()` parsed the flags but treated every non-zero flag as unsupported. That left the
valid `RENAME_NOREPLACE` and `RENAME_EXCHANGE` modes unimplemented on `/tmp`.

## Fix

- Added `RENAME_NOREPLACE` handling by rejecting existing destinations with `EEXIST`.
- Added `RENAME_EXCHANGE` handling by swapping the two pathnames through a temporary name in the
  source directory.
- Kept unsupported `RENAME_WHITEOUT` and invalid flag combinations returning `EINVAL`.
- Enabled `renameat201` in `testcases/all.txt`.

## Validation

- `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp LTP_CASES=renameat201`
