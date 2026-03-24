# Fcntl Family Batch 16

## Goal

Re-enable Priority A cases `fcntl33` and `fcntl33_64`, validating `/tmp` first
and then `/ext2` and `/exfat` serially.

## Problem Cause

`fcntl33*` exercises lease break behavior:

- user space writes `/proc/sys/fs/lease-break-time` before testing;
- a conflicting open/truncate must notify the lease holder with `SIGIO`;
- when a write lease is being broken by a conflicting writer, changing that
  lease from write to read must fail with `EAGAIN`.

Asterinas had three gaps:

- `/proc/sys/fs/lease-break-time` was missing, so the test setup failed with
  `EPERM`;
- lease break notifications for open/truncate did not enqueue `SIGIO` to lease
  owner processes;
- write-lease downgrade during a pending write break was accepted instead of
  being rejected.

## Solution

- Implement `/proc/sys/fs/lease-break-time` in procfs as a writable integer
  sysctl node (default `45`).
- Extend inode lease state to track lease owner PID and pending write-break
  status.
- Send `SIGIO` to the lease holder when conflicting open/truncate operations
  occur from another process.
- Reject `F_SETLEASE(F_RDLCK)` with `EAGAIN` when downgrading a write lease
  under a pending write break.
- Re-enable `fcntl33` and `fcntl33_64` in `testcases/all.txt`.

## Validation Results

Commands:

```bash
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=fcntl33,fcntl33_64

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=fcntl33,fcntl33_64

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=fcntl33,fcntl33_64
```

Observed:

- `/tmp`: both cases `CONF` on `ramfs`, `Total Failures: 0`.
- `/ext2`: both cases `PASS`.
- `/exfat`: both cases `PASS`.
