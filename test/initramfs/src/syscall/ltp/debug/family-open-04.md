# open04

## Goal

Enable `open04` and make it pass on `/tmp`, `/ext2`, and `/exfat`.

## Failure

`open04` expects `open()` to fail with `EMFILE` once the process reaches `RLIMIT_NOFILE`.
On Asterinas, the final `open()` still succeeded on `/tmp`.

## Root Cause

The file table limit path reused `FileTable::len()`, which returns the slot vector length rather
than the number of occupied descriptors. When the table already contained holes, `insert_with_limit`
and `dup_ceil_with_limit` could skip lower free descriptor numbers and allocate at the current tail
instead. `open04` exposed this because the first new descriptor was placed after a holey prefix,
so the test consumed fewer descriptors than expected before reaching its final assertion.

## Fix

Add a shared helper that scans for the lowest free descriptor within the allowed range by checking
actual slot occupancy. Use that helper for both `insert_with_limit()` and `dup_ceil_with_limit()`.

## Validation

- `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp LTP_CASES=open04`
- `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp SYSCALL_TEST_WORKDIR=/ext2 LTP_CASES=open04`
- `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp SYSCALL_TEST_WORKDIR=/exfat LTP_CASES=open04`
