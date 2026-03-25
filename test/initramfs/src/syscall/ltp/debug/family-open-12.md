# open12

## Goal

Enable `open12` and make the `open()` flag coverage for `O_APPEND`, `O_NOATIME`, `O_CLOEXEC`,
and `O_LARGEFILE` pass in the LTP environment.

## Failure

On `/tmp`, `open12` initially failed in two places:

- `O_NOATIME` still changed `st_atime`
- the `O_CLOEXEC` subtest hit `TBROK` because `execlp("open12_child", ...)` failed

## Root Cause

The kernel-side read paths in `ramfs/tmpfs` and `ext2` updated atime unconditionally and ignored
`O_NOATIME`.

Separately, the initramfs LTP packaging logic only copied the primary testcase binaries listed in
`runtest/syscalls`. Helper binaries such as `open12_child` were omitted, so the exec-based
close-on-exec check could not start.

## Fix

- Respect `StatusFlags::O_NOATIME` in the `ramfs/tmpfs` and `ext2` read paths.
- Extend the LTP initramfs packaging step to copy companion helper binaries matching the selected
  testcase binary prefix, such as `open12_child`.

## Validation

- `make run_kernel AUTO_TEST=syscall SYSCALL_TEST_SUITE=ltp LTP_CASES=open12`
