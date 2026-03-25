# openat04

## Goal

Enable `openat04` and make `openat(..., O_TMPFILE, ...)` behave correctly on `/tmp` for a
non-root user inside a setgid directory.

## Failure

On `/tmp`, `openat04` initially failed in two stages:

- `linkat("/proc/self/fd/<n>", ..., AT_SYMLINK_FOLLOW)` returned `EACCES` after switching to
  user `nobody`
- after fixing that, the linked tmpfile still kept `S_ISGID`

## Root Cause

The procfs entries under `/proc/<pid>/fd` were created with owner `root:root` while using modes
such as `u+rx` and `u+rwx`. After the testcase dropped privileges, the process could no longer
traverse its own `/proc/self/fd` directory to link the anonymous tmpfile back into the target
directory.

Separately, `ramfs/tmpfs` inode initialization inherited the parent directory gid for tmpfiles,
but it did not clear `S_ISGID` on non-directory creations when the caller lacked `CAP_FSETID` and
was not a member of the inherited group.

## Fix

- Set procfs `fd` / `fdinfo` entries and `fd` symlinks to the traced task's `fsuid/fsgid`.
- In `ramfs/tmpfs` creation attributes, strip `S_ISGID` from non-directory creations unless the
  caller has `CAP_FSETID` or belongs to the target group.

## Validation

- `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp LTP_CASES=openat04`
