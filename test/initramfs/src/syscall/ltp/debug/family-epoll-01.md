# Epoll Family Batch 01

## Goal

Re-enable a first `epoll` batch by fixing small control-path compatibility
issues instead of mixing them with the separate timeout-accuracy work in
`epoll_wait*`.

Enabled cases in this batch:

- `epoll_ctl02`

## Problem Cause

`epoll_ctl02` initially exposed two Linux compatibility gaps:

1. Asterinas accepted regular inode-backed files in `EPOLL_CTL_ADD`, while
   Linux requires `EPERM` when the target fd does not support epoll.
2. Asterinas accepted `fd == epfd`, while Linux requires `EINVAL` when an
   epoll instance tries to watch itself through the same file descriptor.

These are both control-path validation issues, not filesystem differences.

`epoll_wait02` was also probed during this family pass, but it failed for a
different reason: timeout measurements were unstable on `/tmp`, with several
early wakeups and one long sleep outlier. That case was intentionally left out
of this batch.

## Solution

- Added `FileLike::supports_epoll()` so file types can explicitly opt out of
  epoll monitoring.
- Marked plain `InodeHandle`s without a specialized `file_io` backend as not
  supporting epoll, which makes regular files return `EPERM` on
  `EPOLL_CTL_ADD`.
- Rejected `fd == epfd` in `sys_epoll_ctl()` with Linux-compatible `EINVAL`.

Files changed for this batch:

- `kernel/src/fs/file/file_handle.rs`
- `kernel/src/fs/file/inode_handle.rs`
- `kernel/src/events/epoll/file.rs`
- `kernel/src/syscall/epoll.rs`

## Validation Results

This batch passed on all three validation targets:

- `/tmp`
- `/ext2`
- `/exfat`

Representative commands:

```bash
make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  LTP_CASES=epoll_ctl02

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/ext2 \
  LTP_CASES=epoll_ctl02

make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp \
  SYSCALL_TEST_WORKDIR=/exfat \
  LTP_CASES=epoll_ctl02
```

Observed LTP result on each target:

- `epoll_ctl(...) if fd does not support epoll : EPERM (1)` -> `TPASS`
- `epoll_ctl(...) if fd is the same as epfd : EINVAL (22)` -> `TPASS`

## Follow-up

- `epoll_wait02` remains disabled. Its `/tmp` run failed due timeout-accuracy
  behavior (`woken up early` at several short intervals and one `slept for too
  long` outlier near the 1s case), so it should be debugged as a timer/wakeup
  batch rather than mixed into `epoll_ctl` work.
- `epoll_ctl04`, `epoll_wait05`, and `epoll_pwait03` should be triaged as
  separate `epoll` sub-batches after the timeout path is understood.
