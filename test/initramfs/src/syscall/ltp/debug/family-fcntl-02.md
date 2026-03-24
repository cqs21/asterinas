# Fcntl Family Batch 02

## Goal

Re-enable a second small `fcntl` batch that already behaves correctly on the
generic VFS path and only needs filesystem-specific handling for `/exfat`.

Enabled cases in this batch:

- `fcntl07`
- `fcntl07_64`
- `fcntl15`
- `fcntl15_64`

## Problem Cause

This batch split into two classes:

- `fcntl07*` checks `FD_CLOEXEC` on a regular file, pipe, and FIFO.
- `fcntl15*` checks Linux process-associated record-lock release semantics when
  a file descriptor is closed.

On Asterinas, all four cases already pass on `/tmp`, and all four also pass on
`/ext2`. The only remaining issue is `/exfat` for `fcntl07*`.

`fcntl07*` creates a FIFO in the test work directory. On `/exfat`, `mkfifo()`
fails with `EINVAL`, so the test breaks before it can exercise the close-on-exec
path. That is a filesystem capability gap, not a `fcntl()` semantic failure.

## Solution

- Enabled `fcntl07`, `fcntl07_64`, `fcntl15`, and `fcntl15_64` in
  `testcases/all.txt`.
- Added `fcntl07` and `fcntl07_64` to the `/exfat` blocklist with a short
  reason that the test requires FIFO creation.
- Did not change kernel code in this batch, because the underlying `fcntl`
  semantics are already correct for the exercised paths.

Files changed for this batch:

- `test/initramfs/src/syscall/ltp/testcases/all.txt`
- `test/initramfs/src/syscall/ltp/testcases/blocked/exfat.txt`

## Validation Results

Representative commands:

```bash
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=fcntl07,fcntl07_64,fcntl15,fcntl15_64

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=fcntl07,fcntl07_64,fcntl15,fcntl15_64

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=fcntl07,fcntl07_64,fcntl15,fcntl15_64
```

Observed results:

- `/tmp`: `fcntl07`, `fcntl07_64`, `fcntl15`, `fcntl15_64` all `PASS`.
- `/ext2`: `fcntl07`, `fcntl07_64`, `fcntl15`, `fcntl15_64` all `PASS`.
- `/exfat` before blocklist update:
  - `fcntl15`, `fcntl15_64`: `PASS`
  - `fcntl07`, `fcntl07_64`: `TBROK` because `mkfifo(..., 0666)` returns
    `EINVAL`

After the blocklist update, `/exfat` excludes only the FIFO-dependent
`fcntl07*` cases while keeping `fcntl15*` runnable.

## Follow-up

- `fcntl11` and `fcntl11_64` remain disabled and should be debugged as the next
  `fcntl` sub-batch.
- If exFAT later gains Linux-compatible FIFO handling, `fcntl07*` can be
  removed from `blocked/exfat.txt`.
