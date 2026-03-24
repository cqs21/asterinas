# Fcntl Family Batch 05

## Goal

Re-enable `fcntl17` and `fcntl17_64` in Phase 3 by making the `/tmp` lock
semantics Linux-compatible first, then validating the same behavior on `/ext2`
and `/exfat`.

## Problem Cause

`fcntl17*` exercises a three-process `F_SETLKW` deadlock scenario.

- One waiter must see `EDEADLK` when adding its blocking lock request would
  close a wait cycle.
- After that process exits, the remaining waiter must acquire the lock because
  the exiting process should release its record locks.

Two kernel gaps broke that sequence:

- `RangeLockList` only queued blocking waiters and never detected wait-for
  cycles, so the test hit its alarm instead of returning `EDEADLK`.
- Process exit could drop `FileTable` directly without walking the normal
  close path, so process-associated record locks stayed behind and the other
  waiter remained blocked.

## Solution

- Add owner-level wait tracking in `RangeLockList` and reject blocking lock
  requests with `EDEADLK` when the new wait edges would create a cycle.
- Reuse `FileTable::close_files()` from `Drop for FileTable` so exit-time file
  table teardown releases record locks through the same path as explicit
  descriptor closes.
- Re-enable `fcntl17` and `fcntl17_64` in `testcases/all.txt` after validating
  the fix on all supported workdirs.

## Validation Results

Representative commands:

```bash
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=fcntl17,fcntl17_64

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=fcntl17,fcntl17_64

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=fcntl17,fcntl17_64
```

Observed results:

- `/tmp`: `fcntl17` and `fcntl17_64` both `PASS`.
- `/ext2`: `fcntl17` and `fcntl17_64` both `PASS`.
- `/exfat`: `fcntl17` and `fcntl17_64` both `PASS`.

Representative success signal after the fix:

- One child reports `lockw err 35` (`EDEADLK`).
- The remaining waiter later reports `lockw locked`.
- LTP reports `Block 1 PASSED`.
