# Fcntl Family Batch 08

## Goal

Re-enable `fcntl23` and `fcntl23_64` in Phase 3 by implementing the missing
`F_SETLEASE` and `F_GETLEASE` behavior needed by the lease tests.

## Problem Cause

`fcntl23*` failed immediately on `/tmp` because Asterinas did not implement file
lease commands yet. `fcntl(F_SETLEASE, F_RDLCK)` returned `EINVAL`, so the test
could not observe the expected lease state transitions.

The missing behavior was kernel-generic rather than specific to `/tmp`,
`/ext2`, or `/exfat`.

## Solution

- Add minimal inode-level lease tracking to the VFS lock context.
- Assign each `InodeHandle` a stable owner id so leases can be owned and
  released with the file handle lifetime.
- Implement `fcntl(F_SETLEASE)` and `fcntl(F_GETLEASE)` for inode-backed files.
- Release the lease automatically when the file handle is dropped.
- Re-enable `fcntl23` and `fcntl23_64` in `testcases/all.txt`.

The implemented semantics are intentionally narrow and match the current Phase 3
needs:

- `F_GETLEASE` reports the current lease type or `F_UNLCK`.
- `F_SETLEASE(F_RDLCK)` succeeds on `O_RDONLY` descriptors.
- `F_SETLEASE(F_RDLCK)` returns `EAGAIN` on non-read-only descriptors.
- Conflicting leases from a different handle owner return `EBUSY`.

## Validation Results

Representative commands:

```bash
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=fcntl23,fcntl23_64

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=fcntl23,fcntl23_64

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=fcntl23,fcntl23_64
```

Observed results:

- `/tmp`: `fcntl23` and `fcntl23_64` both `PASS`.
- `/ext2`: `fcntl23` and `fcntl23_64` both `PASS`.
- `/exfat`: `fcntl23` and `fcntl23_64` both `PASS`.
