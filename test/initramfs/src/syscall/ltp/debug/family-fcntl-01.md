# Fcntl Family Batch 01

## Goal

Re-enable a first `fcntl` batch that should be unlocked by a Linux-compatible
`F_DUPFD` fix instead of filesystem-specific work.

Enabled cases in this batch:

- `fcntl12`
- `fcntl12_64`

## Problem Cause

This batch checks one narrow semantic: `fcntl(fd, F_DUPFD, minfd)` must honor
`RLIMIT_NOFILE`.

Before this change, Asterinas duplicated file descriptors with an unbounded
`dup_ceil()` search. That meant:

- `minfd >= RLIMIT_NOFILE` was not rejected consistently at the syscall layer.
- `minfd < RLIMIT_NOFILE` but no free descriptor existed below the limit did
  not return Linux-compatible `EMFILE`.

As a result, `fcntl12` and `fcntl12_64` could not observe the expected
resource-limit failure path.

## Solution

- Added `RLIMIT_NOFILE` handling to `sys_fcntl(..., F_DUPFD, ...)`.
- Introduced `FileTable::dup_ceil_with_limit()` so descriptor allocation can be
  capped at an exclusive upper bound.
- Returned `EMFILE` when no descriptor is available in `[minfd, RLIMIT_NOFILE)`.
- Kept `EINVAL` for invalid `minfd` values that are negative, overflow `i32`, or
  start at/above `RLIMIT_NOFILE`.

Files changed for this batch:

- `kernel/src/syscall/fcntl.rs`
- `kernel/src/fs/file/file_table.rs`

## Validation Results

This batch passed on all three validation targets:

- `/tmp`
- `/ext2`
- `/exfat`

Representative commands:

```bash
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=fcntl12,fcntl12_64

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=fcntl12,fcntl12_64

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=fcntl12,fcntl12_64
```

Observed LTP result on each target:

- `fcntl12`: `TPASS` with `EMFILE (24)`
- `fcntl12_64`: `TPASS` with `EMFILE (24)`

## Follow-up

- `fcntl07`, `fcntl11`, and `fcntl15` remain disabled and should be debugged as
  separate `fcntl` sub-batches.
- There is still a possible errno-priority edge case when both `fd` and `minfd`
  are invalid; that does not affect `fcntl12*`, but it should be kept in mind
  for later `fcntl` coverage.
